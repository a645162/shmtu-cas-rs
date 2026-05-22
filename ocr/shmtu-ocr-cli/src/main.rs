use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use shmtu_cas::captcha;
use shmtu_cas::cas;
use shmtu_ocr::backend::CasOnnxBackend;

#[derive(Parser)]
#[command(name = "shmtu-ocr", about = "上海海事大学CAS验证码OCR识别工具", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 识别本地验证码图片
    Image {
        /// 图片路径
        path: String,

        /// 模型目录
        #[arg(long, default_value = "./Model")]
        model_dir: String,
    },
    /// 从 CAS 服务器拉取验证码并识别，对比远端 OCR 结果
    Compare {
        /// 模型目录
        #[arg(long, default_value = "./Model")]
        model_dir: String,

        /// 远端 OCR 服务器地址
        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        /// 远端 OCR 服务器端口
        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,

        /// 测试轮数
        #[arg(short, long, default_value_t = 5)]
        rounds: u32,
    },
    /// 从 CAS 服务器拉取验证码并仅用本地 ONNX 识别
    Fetch {
        /// 模型目录
        #[arg(long, default_value = "./Model")]
        model_dir: String,

        /// 测试轮数
        #[arg(short, long, default_value_t = 5)]
        rounds: u32,
    },
    /// 下载 ONNX 模型文件
    DownloadModel {
        /// 目标目录
        #[arg(long, default_value = "./Model")]
        model_dir: String,
    },
}

fn load_backend(model_dir: &str) -> Result<CasOnnxBackend> {
    if !CasOnnxBackend::check_model_exists(model_dir) {
        bail!(
            "模型文件不完整，请先运行 `shmtu-ocr download-model --model-dir {}`",
            model_dir
        );
    }
    CasOnnxBackend::load(model_dir).context("加载 ONNX 模型失败")
}

fn download_model(model_dir: &str) -> Result<()> {
    use shmtu_ocr::const_value;

    let files = [
        const_value::MODEL_ONNX_EQUAL_FP32,
        const_value::MODEL_ONNX_OPERATOR_FP32,
        const_value::MODEL_ONNX_DIGIT_FP32,
    ];

    std::fs::create_dir_all(model_dir)?;

    let client = reqwest::blocking::Client::new();

    for file in &files {
        let dest = format!("{}/{}", model_dir, file);
        if std::path::Path::new(&dest).exists() {
            println!("{} 已存在，跳过", file);
            continue;
        }

        let url = format!("{}/{}", const_value::MODEL_ONNX_BASE_URL, file);
        println!("下载 {} ...", url);

        let mut resp = client.get(&url).send().context("HTTP GET 失败")?;
        if !resp.status().is_success() {
            bail!("下载 {} 失败: HTTP {}", file, resp.status());
        }

        let mut out = std::fs::File::create(&dest)?;
        resp.copy_to(&mut out)?;
        println!("{} 下载完成", file);
    }

    println!("所有模型下载完成 → {}", model_dir);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Image { path, model_dir } => {
            let mut backend = load_backend(&model_dir)?;
            let result = backend.predict_file(&path)?;
            println!("算式: {}", result.expr);
            println!("答案: {}", result.result);
            println!(
                "等号: {:?}  运算符: {:?}  数字: {} {}",
                result.equal_symbol, result.operator, result.digit1, result.digit2
            );
        }

        Commands::Compare {
            model_dir,
            ocr_host,
            ocr_port,
            rounds,
        } => {
            let mut backend = load_backend(&model_dir)?;
            let ocr = captcha::CaptchaOcr::new(&ocr_host, ocr_port);
            let client = cas::create_client()?;

            let mut local_ok = 0u32;
            let mut remote_ok = 0u32;
            let mut match_count = 0u32;

            for i in 1..=rounds {
                println!("\n--- 第 {}/{} 轮 ---", i, rounds);

                let image_data = captcha::fetch_captcha(&client).await?;
                println!("验证码大小: {} bytes", image_data.len());

                // 本地 ONNX
                let local_result = match backend.predict_bytes(&image_data) {
                    Ok(r) => {
                        println!("本地 ONNX: {}", r.expr);
                        local_ok += 1;
                        Some(r.result)
                    }
                    Err(e) => {
                        println!("本地 ONNX 失败: {}", e);
                        None
                    }
                };

                // 远端 TCP OCR
                let remote_result = match ocr.ocr_auto_retry(&image_data, 3) {
                    Ok(expr) => {
                        let answer = captcha::get_expr_result(&expr);
                        println!("远端 OCR: {} → 答案: {}", expr, answer);
                        remote_ok += 1;
                        answer.parse::<i32>().ok()
                    }
                    Err(e) => {
                        println!("远端 OCR 失败: {}", e);
                        None
                    }
                };

                // 对比
                match (local_result, remote_result) {
                    (Some(l), Some(r)) => {
                        if l == r {
                            println!("结果一致 ✓");
                            match_count += 1;
                        } else {
                            println!("结果不一致 ✗ (本地={} 远端={})", l, r);
                        }
                    }
                    _ => println!("无法对比（某侧识别失败）"),
                }
            }

            println!("\n===== 汇总 =====");
            println!("总轮数: {}", rounds);
            println!("本地成功: {}/{}", local_ok, rounds);
            println!("远端成功: {}/{}", remote_ok, rounds);
            println!(
                "结果一致: {}/{} ({:.1}%)",
                match_count,
                rounds,
                match_count as f32 / rounds as f32 * 100.0
            );
        }

        Commands::Fetch { model_dir, rounds } => {
            let mut backend = load_backend(&model_dir)?;
            let client = cas::create_client()?;

            let mut ok = 0u32;

            for i in 1..=rounds {
                let image_data = captcha::fetch_captcha(&client).await?;
                match backend.predict_bytes(&image_data) {
                    Ok(r) => {
                        println!("[{}/{}] {}", i, rounds, r.expr);
                        ok += 1;
                    }
                    Err(e) => {
                        println!("[{}/{}] 识别失败: {}", i, rounds, e);
                    }
                }
            }

            println!("\n成功: {}/{}", ok, rounds);
        }

        Commands::DownloadModel { model_dir } => {
            download_model(&model_dir)?;
        }
    }

    Ok(())
}

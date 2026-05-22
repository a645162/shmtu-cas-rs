use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use shmtu_cas::captcha;
use shmtu_cas::cas;
use shmtu_ocr::backend::CasOnnxBackend;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(
    name = "shmtu-ocr",
    about = "上海海事大学CAS验证码OCR识别工具",
    version
)]
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
        #[arg(short, long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..))]
        rounds: u32,
    },
    /// 从 CAS 服务器拉取验证码并仅用本地 ONNX 识别
    Fetch {
        /// 模型目录
        #[arg(long, default_value = "./Model")]
        model_dir: String,

        /// 测试轮数
        #[arg(short, long, default_value_t = 5, value_parser = clap::value_parser!(u32).range(1..))]
        rounds: u32,
    },
    /// 下载 ONNX 模型文件
    DownloadModel {
        /// 目标目录
        #[arg(long, default_value = "./Model")]
        model_dir: String,
    },
}

fn resolve_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .context("获取当前工作目录失败")?
            .join(path))
    }
}

fn load_backend(model_dir: &Path) -> Result<CasOnnxBackend> {
    let missing = CasOnnxBackend::missing_model_files(model_dir);
    if !missing.is_empty() {
        let missing = missing.join(", ");
        bail!(
            "模型文件不完整，缺少: {}。请先运行 `shmtu-ocr download-model --model-dir {}`",
            missing,
            model_dir.display()
        );
    }
    CasOnnxBackend::load(model_dir).context("加载 ONNX 模型失败")
}

fn download_model(model_dir: &Path) -> Result<()> {
    use shmtu_ocr::const_value;

    let files = [
        const_value::MODEL_ONNX_EQUAL_FP32,
        const_value::MODEL_ONNX_OPERATOR_FP32,
        const_value::MODEL_ONNX_DIGIT_FP32,
    ];

    std::fs::create_dir_all(model_dir)?;

    let client = reqwest::blocking::Client::new();

    for file in &files {
        let dest = model_dir.join(file);
        if dest.exists() {
            println!("{} 已存在，跳过", file);
            continue;
        }

        let url = format!("{}/{}", const_value::MODEL_ONNX_BASE_URL, file);
        println!("下载 {} ...", url);

        let mut resp = client.get(&url).send().context("HTTP GET 失败")?;
        if !resp.status().is_success() {
            bail!("下载 {} 失败: HTTP {}", file, resp.status());
        }

        let mut out = std::fs::File::create(&dest)
            .with_context(|| format!("创建模型文件失败: {}", dest.display()))?;
        resp.copy_to(&mut out)?;
        println!("{} 下载完成", file);
    }

    println!("所有模型下载完成 → {}", model_dir.display());
    Ok(())
}

fn extract_result_i32(expr: &str) -> Result<i32> {
    let answer = captcha::get_expr_result(expr);
    answer
        .parse::<i32>()
        .with_context(|| format!("无法解析答案为整数: {}", answer))
}

fn format_rate(ok: u32, total: u32) -> String {
    format!("{ok}/{total} ({:.1}%)", ok as f32 / total as f32 * 100.0)
}

fn format_avg_ms(total: Duration, count: u32) -> String {
    if count == 0 {
        "n/a".to_string()
    } else {
        format!("{:.1} ms", total.as_secs_f64() * 1000.0 / count as f64)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Image { path, model_dir } => {
            let model_dir = resolve_path(&model_dir)?;
            let mut backend = load_backend(&model_dir)?;
            let start = Instant::now();
            let result = backend.predict_file(&path)?;
            let elapsed = start.elapsed();
            println!("图片: {}", path);
            println!("模型目录: {}", model_dir.display());
            println!("算式: {}", result.expr);
            println!("答案: {}", result.result);
            println!(
                "等号: {:?}  运算符: {:?}  数字: {} {}",
                result.equal_symbol, result.operator, result.digit1, result.digit2
            );
            println!("耗时: {:.1} ms", elapsed.as_secs_f64() * 1000.0);
        }

        Commands::Compare {
            model_dir,
            ocr_host,
            ocr_port,
            rounds,
        } => {
            let model_dir = resolve_path(&model_dir)?;
            let mut backend = load_backend(&model_dir)?;
            let ocr = captcha::CaptchaOcr::new(&ocr_host, ocr_port);
            let client = cas::create_client()?;

            let mut local_ok = 0_u32;
            let mut remote_ok = 0_u32;
            let mut both_ok = 0_u32;
            let mut match_count = 0_u32;
            let mut local_elapsed_total = Duration::ZERO;
            let mut remote_elapsed_total = Duration::ZERO;

            println!("模型目录: {}", model_dir.display());
            println!("远端 OCR: {}:{}", ocr_host, ocr_port);

            for i in 1..=rounds {
                println!("\n--- 第 {}/{} 轮 ---", i, rounds);

                let image_data = captcha::fetch_captcha(&client).await?;
                println!("验证码大小: {} bytes", image_data.len());

                // 本地 ONNX
                let local_started = Instant::now();
                let local_result = match backend.predict_bytes(&image_data) {
                    Ok(r) => {
                        let elapsed = local_started.elapsed();
                        local_ok += 1;
                        local_elapsed_total += elapsed;
                        println!(
                            "LOCAL : {:<24} -> {:<3} ({:.1} ms)",
                            r.expr,
                            r.result,
                            elapsed.as_secs_f64() * 1000.0
                        );
                        Some(r.result)
                    }
                    Err(e) => {
                        println!("LOCAL : 失败 -> {}", e);
                        None
                    }
                };

                // 远端 TCP OCR
                let remote_started = Instant::now();
                let remote_result = match ocr.ocr_auto_retry(&image_data, 3) {
                    Ok(expr) => {
                        let elapsed = remote_started.elapsed();
                        let parsed = extract_result_i32(&expr);
                        match parsed {
                            Ok(answer) => {
                                remote_ok += 1;
                                remote_elapsed_total += elapsed;
                                println!(
                                    "REMOTE: {:<24} -> {:<3} ({:.1} ms)",
                                    expr,
                                    answer,
                                    elapsed.as_secs_f64() * 1000.0
                                );
                                Some(answer)
                            }
                            Err(e) => {
                                println!("REMOTE: {} -> 解析失败 ({})", expr, e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        println!("REMOTE: 失败 -> {}", e);
                        None
                    }
                };

                // 对比
                match (local_result, remote_result) {
                    (Some(l), Some(r)) => {
                        both_ok += 1;
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
            println!("本地成功: {}", format_rate(local_ok, rounds));
            println!("远端成功: {}", format_rate(remote_ok, rounds));
            println!("双方都成功: {}", format_rate(both_ok, rounds));
            println!("结果一致: {}", format_rate(match_count, rounds));
            println!(
                "双方成功后一致率: {}",
                if both_ok == 0 {
                    "n/a".to_string()
                } else {
                    format!(
                        "{}/{} ({:.1}%)",
                        match_count,
                        both_ok,
                        match_count as f32 / both_ok as f32 * 100.0
                    )
                }
            );
            println!(
                "本地平均耗时: {}",
                format_avg_ms(local_elapsed_total, local_ok)
            );
            println!(
                "远端平均耗时: {}",
                format_avg_ms(remote_elapsed_total, remote_ok)
            );
        }

        Commands::Fetch { model_dir, rounds } => {
            let model_dir = resolve_path(&model_dir)?;
            let mut backend = load_backend(&model_dir)?;
            let client = cas::create_client()?;

            let mut ok = 0_u32;
            let mut local_elapsed_total = Duration::ZERO;

            println!("模型目录: {}", model_dir.display());

            for i in 1..=rounds {
                let image_data = captcha::fetch_captcha(&client).await?;
                let started = Instant::now();
                match backend.predict_bytes(&image_data) {
                    Ok(r) => {
                        let elapsed = started.elapsed();
                        local_elapsed_total += elapsed;
                        println!(
                            "[{}/{}] {} -> {} ({:.1} ms)",
                            i,
                            rounds,
                            r.expr,
                            r.result,
                            elapsed.as_secs_f64() * 1000.0
                        );
                        ok += 1;
                    }
                    Err(e) => {
                        println!("[{}/{}] 识别失败: {}", i, rounds, e);
                    }
                }
            }

            println!("\n===== 汇总 =====");
            println!("总轮数: {}", rounds);
            println!("成功: {}", format_rate(ok, rounds));
            println!("失败: {}", rounds - ok);
            println!("平均耗时: {}", format_avg_ms(local_elapsed_total, ok));
        }

        Commands::DownloadModel { model_dir } => {
            let model_dir = resolve_path(&model_dir)?;
            download_model(&model_dir)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_numeric_result_from_expr_or_plain_answer() {
        assert_eq!(extract_result_i32("3+5=8").unwrap(), 8);
        assert_eq!(extract_result_i32("42").unwrap(), 42);
    }

    #[test]
    fn rejects_non_numeric_result() {
        assert!(extract_result_i32("abc").is_err());
    }

    #[test]
    fn formats_average_duration() {
        assert_eq!(format_avg_ms(Duration::from_millis(25), 2), "12.5 ms");
        assert_eq!(format_avg_ms(Duration::ZERO, 0), "n/a");
    }
}

use std::io::{self, Write};

use anyhow::{Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use shmtu_cas::{captcha, cas, parser};
use cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};

#[derive(Clone, ValueEnum)]
enum CaptchaMode {
    /// 自动OCR识别
    Ocr,
    /// 手动输入验证码
    Manual,
}

#[derive(Parser)]
#[command(name = "shmtu-cas", about = "上海海事大学CAS登录与账单查询工具", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 登录CAS并获取账单
    Bill {
        #[arg(short, long, env = "SHMTU_USERNAME")]
        username: String,

        /// 用户名(学号)，优先于 --username
        #[arg(long, env = "SHMTU_USER_ID")]
        user_id: Option<String>,

        #[arg(short, long, env = "SHMTU_PASSWORD")]
        password: String,

        /// 验证码模式: ocr(自动识别) 或 manual(手动输入)
        #[arg(short, long, default_value = "ocr")]
        captcha: CaptchaMode,

        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,

        #[arg(short, long)]
        output: Option<String>,

        #[arg(long, default_value = "all")]
        tab: String,

        #[arg(long, default_value_t = 1)]
        page: u32,

        #[arg(long, default_value_t = false)]
        all_pages: bool,
    },
    /// 测试验证码OCR
    CaptchaTest {
        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,
    },
    /// 解析本地HTML账单文件
    Parse {
        #[arg(short, long)]
        input: String,

        #[arg(short, long)]
        output: Option<String>,
    },
}

fn export_csv(path: &str, bills: &[parser::BillItem]) -> Result<()> {
    parser::export::CsvExporter::new().export(path, bills)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Bill {
            username,
            user_id,
            password,
            captcha: captcha_mode,
            ocr_host,
            ocr_port,
            output,
            tab,
            page,
            all_pages,
        } => {
            let username = user_id.as_deref().unwrap_or(&username);
            let mut epay = EpayAuth::new()?;
            let ocr = captcha::CaptchaOcr::new(&ocr_host, ocr_port);

            println!("正在探测登录状态...");
            match epay.probe_login().await? {
                LoginProbe::AlreadyLoggedIn => {
                    println!("已经登录");
                }
                LoginProbe::NeedLogin { .. } => {
                    let max_retries = 5;
                    for attempt in 1..=max_retries {
                        println!("第{}/{}次登录尝试", attempt, max_retries);

                        let challenge = epay.prepare_challenge().await?;
                        println!("验证码大小: {} bytes", challenge.captcha_image.len());

                        let validate_code = match captcha_mode {
                            CaptchaMode::Ocr => {
                                let ocr_result = ocr.ocr_auto_retry(&challenge.captcha_image, 3)?;
                                println!("OCR识别结果: {}", ocr_result);
                                captcha::get_expr_result(&ocr_result)
                            }
                            CaptchaMode::Manual => {
                                std::fs::write("captcha.png", &challenge.captcha_image)?;
                                println!("验证码已保存到 captcha.png，请查看后输入答案");
                                print!("请输入验证码答案: ");
                                io::stdout().flush()?;
                                let mut input = String::new();
                                io::stdin().read_line(&mut input)?;
                                input.trim().to_string()
                            }
                        };
                        println!("验证码答案: {}", validate_code);

                        match epay
                            .submit_login(&username, &password, &validate_code, &challenge.execution)
                            .await?
                        {
                            LoginSubmitResult::Success => {
                                if epay.test_login_status().await? {
                                    println!("登录验证成功！");
                                    break;
                                }
                                bail!("登录验证失败");
                            }
                            LoginSubmitResult::ValidateCodeError => {
                                println!("验证码错误，重试中...");
                                continue;
                            }
                            LoginSubmitResult::PasswordError => {
                                bail!("用户名或密码错误");
                            }
                            LoginSubmitResult::Failure(msg) => {
                                bail!("登录失败: {}", msg);
                            }
                        }
                    }
                }
            }

            println!("正在获取账单...");

            let tab_no = match tab.as_str() {
                "all" => "1",
                "success" => "2",
                "waitfor" => "3",
                "failure" => "4",
                _ => "1",
            };

            let mut all_bills = Vec::new();
            let mut current_page = page;

            loop {
                let html = epay.get_bill(current_page, tab_no).await?;

                let bill_list = parser::parse_bill_list(&html)?;
                let total_pages = parser::get_total_pages(&html)?;

                if bill_list.is_empty() && current_page == page {
                    println!("没有找到账单记录");
                    return Ok(());
                }

                println!(
                    "第{}/{}页: 找到{}条记录",
                    current_page,
                    total_pages,
                    bill_list.len()
                );

                all_bills.extend(bill_list);

                if !all_pages || current_page >= total_pages {
                    break;
                }
                current_page += 1;
            }

            println!("共{}条账单记录", all_bills.len());

            for bill in &all_bills {
                println!(
                    "{} | {} | {} | {} | {}",
                    bill.date_time_formatted,
                    bill.item_type,
                    bill.target_user,
                    bill.money_str,
                    bill.status
                );
            }

            if let Some(path) = output {
                export_csv(&path, &all_bills)?;
                println!("已导出到 {}", path);
            }
        }

        Commands::CaptchaTest { ocr_host, ocr_port } => {
            let ocr = captcha::CaptchaOcr::new(&ocr_host, ocr_port);

            println!("正在获取验证码...");
            let client = cas::create_client()?;
            let image_data = captcha::fetch_captcha(&client).await?;

            println!("验证码大小: {} bytes", image_data.len());

            std::fs::write("captcha_test.png", &image_data)?;
            println!("已保存验证码图片到 captcha_test.png");

            println!("正在识别验证码...");
            let result = ocr.ocr_auto_retry(&image_data, 3)?;
            println!("OCR结果: {}", result);

            let expr_result = captcha::get_expr_result(&result);
            println!("验证码答案: {}", expr_result);
        }

        Commands::Parse { input, output } => {
            let html = std::fs::read_to_string(&input)?;

            let bill_list = parser::parse_bill_list(&html)?;

            if bill_list.is_empty() {
                println!("没有找到账单记录");
                return Ok(());
            }

            println!("找到{}条账单记录", bill_list.len());
            for bill in &bill_list {
                println!(
                    "{} | {} | {} | {} | {}",
                    bill.date_time_formatted,
                    bill.item_type,
                    bill.target_user,
                    bill.money_str,
                    bill.status
                );
            }

            if let Some(path) = output {
                export_csv(&path, &bill_list)?;
                println!("已导出到 {}", path);
            }
        }
    }

    Ok(())
}

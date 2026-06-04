use std::collections::HashSet;
use std::io::{self, Write};
use std::sync::Arc;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use shmtu_cas::captcha::{
    self, CaptchaAnswer, CaptchaOcr, CaptchaOcrHttp, CaptchaResolver, ManualCaptchaResolver,
    OcrCaptchaResolver, OcrHttpCaptchaResolver,
};
use shmtu_cas::cas::epay::{EpayAuth, LoginProbe, LoginSubmitResult};
use shmtu_cas::cas::{self, wechat};
use shmtu_cas::datatype::bill::{BillItem, BillType};
use shmtu_cas::parser;
use shmtu_cas::sync::{self, BillStore, SyncOptions};

#[derive(Clone, ValueEnum)]
enum CaptchaMode {
    /// 自动OCR识别
    Ocr,
    /// 手动输入验证码
    Manual,
}

#[derive(Clone, ValueEnum, Default)]
enum OcrServerType {
    /// TCP 协议连接 OCR 服务器
    #[default]
    Tcp,
    /// RESTful HTTP 协议连接 OCR 服务器
    Restful,
}

#[derive(Clone, ValueEnum)]
enum BillTabArg {
    All,
    Success,
    WaitFor,
    Failure,
}

impl From<BillTabArg> for BillType {
    fn from(value: BillTabArg) -> Self {
        match value {
            BillTabArg::All => BillType::All,
            BillTabArg::Success => BillType::Success,
            BillTabArg::WaitFor => BillType::NotPaid,
            BillTabArg::Failure => BillType::Failure,
        }
    }
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

        #[arg(short, long, env = "SHMTU_PASSWORD", hide_env_values = true)]
        password: String,

        /// 验证码模式: ocr(自动识别) 或 manual(手动输入)
        #[arg(short, long, default_value = "ocr")]
        captcha: CaptchaMode,

        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,

        /// OCR 服务器协议类型: tcp 或 restful
        #[arg(long, default_value = "tcp")]
        ocr_server_type: OcrServerType,

        /// RESTful OCR 服务器地址
        #[arg(long, env = "SHMTU_OCR_HTTP_URL", default_value = "http://127.0.0.1:5000")]
        ocr_http_url: String,

        #[arg(short, long)]
        output: Option<String>,

        #[arg(long, default_value = "all")]
        tab: BillTabArg,

        #[arg(long, default_value_t = 1)]
        page: u32,

        #[arg(long, default_value_t = false)]
        all_pages: bool,
    },
    /// 增量同步账单（仅拉取新增条目）
    Sync {
        #[arg(short, long, env = "SHMTU_USERNAME")]
        username: String,

        /// 用户名(学号)，优先于 --username
        #[arg(long, env = "SHMTU_USER_ID")]
        user_id: Option<String>,

        #[arg(short, long, env = "SHMTU_PASSWORD", hide_env_values = true)]
        password: String,

        /// 验证码模式: ocr(自动识别) 或 manual(手动输入)
        #[arg(short, long, default_value = "ocr")]
        captcha: CaptchaMode,

        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,

        /// OCR 服务器协议类型: tcp 或 restful
        #[arg(long, default_value = "tcp")]
        ocr_server_type: OcrServerType,

        /// RESTful OCR 服务器地址
        #[arg(long, env = "SHMTU_OCR_HTTP_URL", default_value = "http://127.0.0.1:5000")]
        ocr_http_url: String,

        /// 本地 JSON 存档路径（默认 bills.json）
        #[arg(short, long, default_value = "bills.json")]
        store: String,

        #[arg(long, default_value = "all")]
        tab: BillTabArg,

        /// 连续遇到多少条已知条目后早停
        #[arg(long, default_value_t = 5)]
        early_stop: u32,

        /// 最大翻页数
        #[arg(long, default_value_t = 100)]
        max_pages: u32,
    },
    /// 测试验证码OCR
    CaptchaTest {
        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,

        /// OCR 服务器协议类型: tcp 或 restful
        #[arg(long, default_value = "tcp")]
        ocr_server_type: OcrServerType,

        /// RESTful OCR 服务器地址
        #[arg(long, env = "SHMTU_OCR_HTTP_URL", default_value = "http://127.0.0.1:5000")]
        ocr_http_url: String,
    },
    /// 解析本地HTML账单文件
    Parse {
        #[arg(short, long)]
        input: String,

        #[arg(short, long)]
        output: Option<String>,
    },
    /// 登录微信平台并获取宿舍热水状态
    HotWater {
        #[arg(short, long, env = "SHMTU_USERNAME")]
        username: String,

        /// 用户名(学号)，优先于 --username
        #[arg(long, env = "SHMTU_USER_ID")]
        user_id: Option<String>,

        #[arg(short, long, env = "SHMTU_PASSWORD", hide_env_values = true)]
        password: String,

        /// 验证码模式: ocr(自动识别) 或 manual(手动输入)
        #[arg(short, long, default_value = "ocr")]
        captcha: CaptchaMode,

        #[arg(long, env = "SHMTU_OCR_HOST", default_value = "127.0.0.1")]
        ocr_host: String,

        #[arg(long, env = "SHMTU_OCR_PORT", default_value_t = 21601)]
        ocr_port: u16,

        /// OCR 服务器协议类型: tcp 或 restful
        #[arg(long, default_value = "tcp")]
        ocr_server_type: OcrServerType,

        /// RESTful OCR 服务器地址
        #[arg(long, env = "SHMTU_OCR_HTTP_URL", default_value = "http://127.0.0.1:5000")]
        ocr_http_url: String,
    },
}

fn export_csv(path: &str, bills: &[BillItem]) -> Result<()> {
    parser::export::CsvExporter::new().export(path, bills)
}

/// 解析最终的 HTTP OCR base URL。
///
/// 优先级: 命令行 `--ocr-http-url` > 环境变量 `SHMTU_OCR_HTTP_URL` >
///         环境变量 `SHMTU_OCR_HOST` (拼接端口, HTTP 端口) > 硬编码默认。
///
/// 端口优先级: 命令行 `--ocr-port` (TCP 端口) > `SHMTU_HTTP_PORT` > 21600。
/// 注意 HTTP OCR API 默认端口是 21600 (与 shmtu-ocr-server C++ server.h 一致)。
fn resolve_ocr_http_url(override_url: Option<&str>, host: &str, port: u16) -> String {
    if let Some(url) = override_url {
        if !url.is_empty() {
            return url.to_string();
        }
    }
    if let Ok(env_url) = std::env::var("SHMTU_OCR_HTTP_URL") {
        if !env_url.is_empty() {
            return env_url;
        }
    }
    if !host.is_empty() {
        // 优先使用 SHMTU_HTTP_PORT (HTTP OCR API 端口), 否则用 TCP 端口, 最后兜底 21600
        let http_port: u16 = std::env::var("SHMTU_HTTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(port);
        return format!("http://{}:{}", host, http_port);
    }
    "http://127.0.0.1:5000".to_string()
}

/// 根据 CLI 模式构造对应的 CaptchaResolver。
fn build_resolver(mode: &CaptchaMode, ocr_host: &str, ocr_port: u16, ocr_server_type: &OcrServerType, ocr_http_url: &str) -> Arc<dyn CaptchaResolver> {
    match mode {
        CaptchaMode::Ocr => match ocr_server_type {
            OcrServerType::Tcp => Arc::new(OcrCaptchaResolver::new(ocr_host, ocr_port)),
            OcrServerType::Restful => Arc::new(OcrHttpCaptchaResolver::new(ocr_http_url)),
        },
        CaptchaMode::Manual => Arc::new(ManualCaptchaResolver::new(Box::new(|image: &[u8]| {
            let owned = image.to_vec();
            Box::pin(async move {
                std::fs::write("captcha.png", &owned)?;
                println!("验证码已保存到 captcha.png，请查看后输入答案");
                print!("请输入验证码答案: ");
                io::stdout().flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
                Ok(CaptchaAnswer::answer(input.trim().to_string()))
            })
        }))),
    }
}

/// 基于 JSON 文件的简单 BillStore 实现（CLI 示例用）。
struct JsonBillStore {
    path: String,
    bills: Vec<BillItem>,
    known_numbers: HashSet<String>,
}

impl JsonBillStore {
    fn load(path: &str) -> Result<Self> {
        let bills = if std::path::Path::new(path).exists() {
            let data = std::fs::read_to_string(path)
                .context("读取本地账单文件失败")?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        let known_numbers: HashSet<String> = bills.iter().map(|b: &BillItem| b.number.clone()).collect();

        Ok(Self {
            path: path.to_string(),
            bills,
            known_numbers,
        })
    }

    fn save(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.bills)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}

impl BillStore for JsonBillStore {
    fn contains(&self, number: &str) -> bool {
        self.known_numbers.contains(number)
    }

    fn merge(&mut self, new_bills: Vec<BillItem>) {
        for bill in new_bills {
            if !self.known_numbers.contains(&bill.number) {
                self.known_numbers.insert(bill.number.clone());
                self.bills.push(bill);
            }
        }
    }
}

/// 通用登录流程：探测 → 重试循环。返回已登录的 EpayAuth。
async fn do_login(
    epay: &mut EpayAuth,
    username: &str,
    password: &str,
    resolver: &Arc<dyn CaptchaResolver>,
) -> Result<()> {
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

                let answer = resolver.resolve(&challenge.captcha_image).await?;
                let validate_code = answer.into_final_answer();
                println!("验证码答案: {}", validate_code);

                match epay
                    .submit_login(username, password, &validate_code, &challenge.execution)
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
    Ok(())
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
            ocr_server_type,
            ocr_http_url,
            output,
            tab,
            page,
            all_pages,
        } => {
            let username = user_id.as_deref().unwrap_or(&username);
            let mut epay = EpayAuth::new()?;
            let ocr_http_url = resolve_ocr_http_url(Some(&ocr_http_url), &ocr_host, ocr_port);
            let resolver = build_resolver(&captcha_mode, &ocr_host, ocr_port, &ocr_server_type, &ocr_http_url);
            do_login(&mut epay, username, &password, &resolver).await?;

            println!("正在获取账单...");

            let bill_type: BillType = tab.into();
            let tab_no = bill_type.tab_no();

            let mut all_bills = Vec::new();
            let mut current_page = page;

            loop {
                let html = epay.get_bill(current_page, tab_no).await?;
                let page_result = parser::parse_bill_page(&html)?;

                if page_result.bills.is_empty() && current_page == page {
                    println!("没有找到账单记录");
                    return Ok(());
                }

                println!(
                    "第{}/{}页: 找到{}条记录",
                    current_page,
                    page_result.total_pages,
                    page_result.bills.len()
                );

                let total_pages = page_result.total_pages;
                all_bills.extend(page_result.bills);

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
                    bill.status_str
                );
            }

            if let Some(path) = output {
                export_csv(&path, &all_bills)?;
                println!("已导出到 {}", path);
            }
        }

        Commands::Sync {
            username,
            user_id,
            password,
            captcha: captcha_mode,
            ocr_host,
            ocr_port,
            ocr_server_type,
            ocr_http_url,
            store: store_path,
            tab,
            early_stop,
            max_pages,
        } => {
            let username = user_id.as_deref().unwrap_or(&username);
            let mut epay = EpayAuth::new()?;
            let ocr_http_url = resolve_ocr_http_url(Some(&ocr_http_url), &ocr_host, ocr_port);
            let resolver = build_resolver(&captcha_mode, &ocr_host, ocr_port, &ocr_server_type, &ocr_http_url);
            do_login(&mut epay, username, &password, &resolver).await?;

            println!("正在增量同步账单...");
            let mut store = JsonBillStore::load(&store_path)?;
            println!("本地已有 {} 条记录", store.bills.len());

            let options = SyncOptions {
                bill_type: tab.into(),
                early_stop_threshold: early_stop,
                max_pages,
                ..Default::default()
            };

            let result = sync::incremental_sync(&epay, &mut store, &options).await?;

            println!(
                "同步完成: 新增 {} 条, 翻页 {}, {}",
                result.new_count,
                result.pages_fetched,
                if result.early_stopped { "早停" } else { "正常结束" }
            );

            for bill in &result.new_bills {
                println!(
                    "{} | {} | {} | {} | {}",
                    bill.date_time_formatted,
                    bill.item_type,
                    bill.target_user,
                    bill.money_str,
                    bill.status_str
                );
            }

            if result.new_count > 0 {
                store.save()?;
                println!("已保存到 {}", store_path);
            }
        }

        Commands::CaptchaTest { ocr_host, ocr_port, ocr_server_type, ocr_http_url } => {
            println!("正在获取验证码...");
            let client = cas::create_client()?;
            let image_data = captcha::fetch_captcha(&client).await?;
            let ocr_http_url = resolve_ocr_http_url(Some(&ocr_http_url), &ocr_host, ocr_port);

            println!("验证码大小: {} bytes", image_data.len());

            std::fs::write("captcha_test.png", &image_data)?;
            println!("已保存验证码图片到 captcha_test.png");

            println!("正在识别验证码...");
            let result = match ocr_server_type {
                OcrServerType::Tcp => {
                    let ocr = CaptchaOcr::new(&ocr_host, ocr_port);
                    ocr.ocr_auto_retry(&image_data, 3)?
                }
                OcrServerType::Restful => {
                    let ocr = CaptchaOcrHttp::new(&ocr_http_url);
                    ocr.ocr_auto_retry(&image_data, 3)?
                }
            };
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
                    bill.status_str
                );
            }

            if let Some(path) = output {
                export_csv(&path, &bill_list)?;
                println!("已导出到 {}", path);
            }
        }

        Commands::HotWater {
            username,
            user_id,
            password,
            captcha: captcha_mode,
            ocr_host,
            ocr_port,
            ocr_server_type,
            ocr_http_url,
        } => {
            let username = user_id.as_deref().unwrap_or(&username);
            let mut wx = wechat::WechatAuth::new()?;
            let ocr_http_url = resolve_ocr_http_url(Some(&ocr_http_url), &ocr_host, ocr_port);
            let resolver = build_resolver(&captcha_mode, &ocr_host, ocr_port, &ocr_server_type, &ocr_http_url);

            println!("正在探测登录状态...");
            match wx.probe_login().await? {
                wechat::LoginProbe::AlreadyLoggedIn => {
                    println!("已经登录");
                }
                wechat::LoginProbe::NeedLogin { ticket_url } => {
                    let max_retries = 5;
                    for attempt in 1..=max_retries {
                        println!("第{}/{}次登录尝试", attempt, max_retries);

                        let challenge = wx.prepare_challenge(&ticket_url).await?;
                        println!("验证码大小: {} bytes", challenge.captcha_image.len());

                        let answer = resolver.resolve(&challenge.captcha_image).await?;
                        let validate_code = answer.into_final_answer();
                        println!("验证码答案: {}", validate_code);

                        match wx
                            .submit_login(username, &password, &validate_code, &challenge.execution)
                            .await?
                        {
                            wechat::LoginSubmitResult::Success => {
                                if wx.test_login_status().await? {
                                    println!("登录验证成功！");
                                    break;
                                }
                                bail!("登录验证失败");
                            }
                            wechat::LoginSubmitResult::ValidateCodeError => {
                                println!("验证码错误，重试中...");
                                continue;
                            }
                            wechat::LoginSubmitResult::PasswordError => {
                                bail!("用户名或密码错误");
                            }
                            wechat::LoginSubmitResult::Failure(msg) => {
                                bail!("登录失败: {}", msg);
                            }
                        }
                    }
                }
            }

            println!("正在获取热水信息...");
            let html = wx.get_hot_water().await?;
            let list = parser::hot_water::parse_hot_water_list(&html)?;

            if list.is_empty() {
                if html.contains("Fatal error")
                    || html.contains("404 Not Found")
                    || html.contains("Exception")
                {
                    eprintln!("\n[服务器错误] SHMTU 热水接口返回错误页（与本程序无关）:");
                    eprintln!("{}", html.chars().take(500).collect::<String>());
                } else {
                    println!("没有找到热水信息（可能 HTML 结构变化或宿舍楼信息为空）");
                }
                return Ok(());
            }

            println!("共{}栋楼", list.len());
            for info in &list {
                println!(
                    "{}号楼: 温度 {:.1}℃, 水位 {:.0}%",
                    info.building, info.temperature, info.water_level
                );
            }
        }
    }

    Ok(())
}

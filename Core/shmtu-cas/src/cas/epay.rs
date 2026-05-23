use anyhow::{Context, Result, bail};
use reqwest::StatusCode;

use super::{self as cas, CasAuthResult};
use crate::captcha;

/// Cookie 条目（用于序列化/反序列化）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CookieEntry {
    value: String,
    #[serde(default)]
    domain: Option<String>,
}

const EPAY_BILL_URL: &str = "https://ecard.shmtu.edu.cn/epay/consume/query";

/// Cookie 管理器：将 cookies 注入到 HTTP 请求的 Cookie header
struct CookieJar {
    /// 格式："key1=val1; key2=val2"
    cookies: String,
}

impl CookieJar {
    fn new() -> Self {
        Self {
            cookies: String::new(),
        }
    }

    /// 从 JSON 字符串恢复 cookies
    fn restore(&mut self, json: &str) -> Result<()> {
        let parsed: std::collections::HashMap<String, CookieEntry> =
            serde_json::from_str(json).context("解析 cookies JSON 失败")?;
        self.cookies = parsed
            .iter()
            .map(|(k, v)| format!("{}={}", k, v.value))
            .collect::<Vec<_>>()
            .join("; ");
        Ok(())
    }

    /// 提取当前 cookies 为 JSON 字符串
    fn extract(&self) -> Result<String> {
        let mut map = std::collections::HashMap::new();
        for pair in self.cookies.split(';') {
            let pair = pair.trim();
            if let Some((k, v)) = pair.split_once('=') {
                let k = k.trim().to_string();
                let v = v.trim().to_string();
                if !k.is_empty() {
                    map.insert(
                        k,
                        CookieEntry {
                            value: v,
                            domain: None,
                        },
                    );
                }
            }
        }
        serde_json::to_string(&map).context("序列化 cookies 失败")
    }

    /// 追加 Set-Cookie header 值（格式："name=value; path=...; domain=...; ..."）
    fn add_from_set_cookie(&mut self, header_val: &str) {
        if let Some((name, value)) = header_val.split_once(';').and_then(|(k, _)| k.split_once('=')) {
            let name = name.trim();
            let value = value.trim();
            if name.is_empty() || value.is_empty() {
                return;
            }
            // 移除同名 cookie，追加新值
            let existing: String = self
                .cookies
                .split(';')
                .map(|p| p.trim())
                .filter(|p| {
                    !p.starts_with(&format!("{}=", name))
                })
                .collect::<Vec<_>>()
                .join("; ");
            if existing.is_empty() {
                self.cookies = format!("{}={}", name, value);
            } else {
                self.cookies = format!("{}; {}={}", existing, name, value);
            }
        }
    }

    /// 取出当前 cookie 字符串
    fn get(&self) -> &str {
        &self.cookies
    }
}

impl Default for CookieJar {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EpayAuth {
    client: reqwest::Client,
    cookies: CookieJar,
    login_url: Option<String>,
}

/// 探测登录状态的结果
pub enum LoginProbe {
    AlreadyLoggedIn,
    NeedLogin { login_url: String },
}

/// 提交登录后的结果
pub enum LoginSubmitResult {
    Success,
    ValidateCodeError,
    PasswordError,
    Failure(String),
}

/// 一次登录尝试所需的材料
pub struct LoginChallenge {
    pub execution: String,
    pub captcha_image: Vec<u8>,
}

impl EpayAuth {
    pub fn new() -> Result<Self> {
        let client = cas::create_client()?;
        Ok(Self {
            client,
            cookies: CookieJar::new(),
            login_url: None,
        })
    }

    /// 从外部恢复会话 cookies（供调用方在 login_epay 前调用）
    pub fn restore_session(&mut self, cookies_json: &str) -> Result<()> {
        self.cookies.restore(cookies_json)
    }

    /// 提取当前 cookies 为 JSON（供登录成功后保存会话）
    pub fn extract_session(&self) -> Result<String> {
        self.cookies.extract()
    }

    /// 在请求前后自动处理 Set-Cookie 头的辅助方法
    fn get_with_cookies(&self, url: &str) -> impl std::future::Future<Output = Result<reqwest::Response, reqwest::Error>> + Send + '_ {
        let jar = &self.cookies;
        let req = self.client.get(url);
        let req = if jar.get().is_empty() {
            req
        } else {
            req.header(reqwest::header::COOKIE, jar.get())
        };
        req.send()
    }

    /// 探测登录状态
    pub async fn probe_login(&mut self) -> Result<LoginProbe> {
        let url = format!("{}?pageNo=1&tabNo=1", EPAY_BILL_URL);
        let resp = self.get_with_cookies(&url).await
            .context("探测登录状态失败")?;

        // 提取 Set-Cookie 响应头
        for header_val in resp.headers().get_all(reqwest::header::SET_COOKIE) {
            if let Ok(s) = header_val.to_str() {
                self.cookies.add_from_set_cookie(s);
            }
        }

        if resp.status() == StatusCode::OK {
            return Ok(LoginProbe::AlreadyLoggedIn);
        }

        if resp.status() == StatusCode::FOUND {
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("")
                .to_string();

            if location.is_empty() {
                bail!("重定向URL为空");
            }

            self.login_url = Some(location.clone());
            Ok(LoginProbe::NeedLogin { login_url: location })
        } else {
            bail!("探测登录状态失败，状态码: {}", resp.status())
        }
    }

    /// 获取execution令牌 + 验证码图片，交给调用方解验证码
    pub async fn prepare_challenge(&self) -> Result<LoginChallenge> {
        let login_url = self
            .login_url
            .as_ref()
            .context("尚未探测登录状态，请先调用 probe_login")?;

        let execution = cas::get_execution(&self.client, login_url).await?;
        let captcha_image = captcha::fetch_captcha(&self.client).await?;

        Ok(LoginChallenge {
            execution,
            captcha_image,
        })
    }

    /// 提交验证码答案完成登录
    pub async fn submit_login(
        &self,
        username: &str,
        password: &str,
        validate_code: &str,
        execution: &str,
    ) -> Result<LoginSubmitResult> {
        let login_url = self
            .login_url
            .as_ref()
            .context("尚未探测登录状态，请先调用 probe_login")?;

        let result = cas::cas_login(
            &self.client,
            login_url,
            username,
            password,
            validate_code,
            execution,
        )
        .await?;

        match result {
            CasAuthResult::Success { location } => {
                cas::cas_redirect(&self.client, &location).await?;
                Ok(LoginSubmitResult::Success)
            }
            CasAuthResult::ValidateCodeError => Ok(LoginSubmitResult::ValidateCodeError),
            CasAuthResult::PasswordError => Ok(LoginSubmitResult::PasswordError),
            CasAuthResult::Failure(msg) => Ok(LoginSubmitResult::Failure(msg)),
        }
    }

    /// 测试是否已登录
    pub async fn test_login_status(&self) -> Result<bool> {
        let url = format!("{}?pageNo=1&tabNo=1", EPAY_BILL_URL);
        let resp = self.get_with_cookies(&url).await
            .context("测试登录状态失败")?;

        Ok(resp.status() == StatusCode::OK)
    }

    /// 获取账单页面HTML
    pub async fn get_bill(&self, page_no: u32, tab_no: &str) -> Result<String> {
        let url = format!("{}?pageNo={}&tabNo={}", EPAY_BILL_URL, page_no, tab_no);
        let resp = self.get_with_cookies(&url).await
            .context("获取账单失败")?;

        if resp.status() == StatusCode::OK {
            Ok(resp.text().await?)
        } else if resp.status() == StatusCode::FOUND {
            bail!("未登录，需要重新登录");
        } else {
            bail!("获取账单失败，状态码: {}", resp.status());
        }
    }
}

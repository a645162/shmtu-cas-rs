use anyhow::{Context, Result, bail};
use reqwest::StatusCode;

use super::{self as cas, CasAuthResult};
use crate::captcha;

const EPAY_BILL_URL: &str = "https://ecard.shmtu.edu.cn/epay/consume/query";

pub struct EpayAuth {
    client: reqwest::Client,
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
            login_url: None,
        })
    }

    /// 探测登录状态
    pub async fn probe_login(&mut self) -> Result<LoginProbe> {
        let url = format!("{}?pageNo=1&tabNo=1", EPAY_BILL_URL);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("探测登录状态失败")?;

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
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .context("测试登录状态失败")?;

        Ok(resp.status() == StatusCode::OK)
    }

    /// 获取账单页面HTML
    pub async fn get_bill(&self, page_no: u32, tab_no: &str) -> Result<String> {
        let url = format!("{}?pageNo={}&tabNo={}", EPAY_BILL_URL, page_no, tab_no);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
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

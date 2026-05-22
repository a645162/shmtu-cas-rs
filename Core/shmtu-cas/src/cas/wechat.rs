use anyhow::{Context, Result, bail};
use reqwest::StatusCode;

use super::{self as cas, CasAuthResult};
use crate::captcha;

const HOT_WATER_URL: &str = "http://hqzx.shmtu.edu.cn/cellphone/getHotWater";

pub struct WechatAuth {
    client: reqwest::Client,
    login_url: Option<String>,
}

pub enum LoginProbe {
    AlreadyLoggedIn,
    NeedLogin { ticket_url: String },
}

pub enum LoginSubmitResult {
    Success,
    ValidateCodeError,
    PasswordError,
    Failure(String),
}

pub struct LoginChallenge {
    pub execution: String,
    pub captcha_image: Vec<u8>,
    pub login_url: String,
}

impl WechatAuth {
    pub fn new() -> Result<Self> {
        let client = cas::create_client()?;
        Ok(Self {
            client,
            login_url: None,
        })
    }

    /// 探测登录状态，未登录返回 wengine_new_ticket 跳转 URL
    pub async fn probe_login(&mut self) -> Result<LoginProbe> {
        let resp = self
            .client
            .get(HOT_WATER_URL)
            .send()
            .await
            .context("探测热水登录状态失败")?;

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

            Ok(LoginProbe::NeedLogin { ticket_url: location })
        } else {
            bail!("探测热水登录状态失败，状态码: {}", resp.status())
        }
    }

    /// 跟随 wengine_new_ticket 一次跳转，拿到真正的 CAS 登录 URL
    pub async fn prepare_challenge(&mut self, ticket_url: &str) -> Result<LoginChallenge> {
        let resp = self
            .client
            .get(ticket_url)
            .send()
            .await
            .context("获取wengine_new_ticket失败")?;

        if resp.status() != StatusCode::FOUND {
            bail!("wengine_new_ticket未返回重定向，状态码: {}", resp.status());
        }

        let login_url = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if login_url.is_empty() {
            bail!("CAS登录URL为空");
        }

        self.login_url = Some(login_url.clone());

        let execution = cas::get_execution(&self.client, &login_url).await?;
        let captcha_image = captcha::fetch_captcha(&self.client).await?;

        Ok(LoginChallenge {
            execution,
            captcha_image,
            login_url,
        })
    }

    /// 提交验证码答案完成登录，并跳转到 getHotWater 服务
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
            .context("尚未准备challenge，请先调用 prepare_challenge")?;

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
                let final_url = format!("{}&from={}", location, HOT_WATER_URL);
                cas::cas_redirect(&self.client, &final_url).await?;
                Ok(LoginSubmitResult::Success)
            }
            CasAuthResult::ValidateCodeError => Ok(LoginSubmitResult::ValidateCodeError),
            CasAuthResult::PasswordError => Ok(LoginSubmitResult::PasswordError),
            CasAuthResult::Failure(msg) => Ok(LoginSubmitResult::Failure(msg)),
        }
    }

    pub async fn test_login_status(&self) -> Result<bool> {
        let resp = self
            .client
            .get(HOT_WATER_URL)
            .send()
            .await
            .context("测试热水登录状态失败")?;

        Ok(resp.status() == StatusCode::OK)
    }

    /// 获取热水HTML
    pub async fn get_hot_water(&self) -> Result<String> {
        let resp = self
            .client
            .get(HOT_WATER_URL)
            .send()
            .await
            .context("获取热水信息失败")?;

        if resp.status() == StatusCode::OK {
            Ok(resp.text().await?)
        } else {
            bail!("获取热水信息失败，状态码: {}", resp.status())
        }
    }
}

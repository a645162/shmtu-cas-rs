pub mod epay;

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode, redirect};
use scraper::{Html, Selector};

#[derive(Debug, PartialEq)]
pub enum CasAuthResult {
    Success { location: String },
    ValidateCodeError,
    PasswordError,
    Failure(String),
}

pub fn create_client() -> Result<Client> {
    Client::builder()
        .redirect(redirect::Policy::none())
        .cookie_store(true)
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .context("创建HTTP客户端失败")
}

pub async fn get_execution(client: &Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .context("获取登录页面失败")?;

    if resp.status() != StatusCode::OK {
        anyhow::bail!("获取登录页面失败，状态码: {}", resp.status());
    }

    let html_text = resp.text().await?;
    let document = Html::parse_document(&html_text);
    let selector = Selector::parse("input[name='execution']").unwrap();

    document
        .select(&selector)
        .next()
        .and_then(|el| el.value().attr("value"))
        .map(|v| v.to_string())
        .context("未找到execution元素")
}

pub async fn cas_login(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    validate_code: &str,
    execution: &str,
) -> Result<CasAuthResult> {
    let resp = client
        .post(url)
        .form(&[
            ("username", username.trim()),
            ("password", password.trim()),
            ("validateCode", validate_code.trim()),
            ("execution", execution.trim()),
            ("_eventId", "submit"),
            ("geolocation", ""),
        ])
        .send()
        .await
        .context("提交登录表单失败")?;

    let status = resp.status();

    if status == StatusCode::FOUND {
        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        Ok(CasAuthResult::Success { location })
    } else {
        let html = resp.text().await?;
        let document = Html::parse_document(&html);
        let selector = Selector::parse("#loginErrorsPanel").unwrap();
        let error_text = document
            .select(&selector)
            .next()
            .map(|el| el.text().collect::<String>())
            .unwrap_or_default();

        if error_text.contains("account is not recognized") || error_text.contains("用户名或密码") {
            Ok(CasAuthResult::PasswordError)
        } else if error_text.contains("reCAPTCHA") || error_text.contains("验证码") {
            Ok(CasAuthResult::ValidateCodeError)
        } else {
            Ok(CasAuthResult::Failure(error_text))
        }
    }
}

pub async fn cas_redirect(client: &Client, url: &str) -> Result<()> {
    let mut current_url = url.to_string();

    for _ in 0..10 {
        let resp = client
            .get(&current_url)
            .send()
            .await
            .context("跟随重定向失败")?;

        let status = resp.status();
        if status == StatusCode::FOUND || status == StatusCode::MOVED_PERMANENTLY {
            if let Some(location) = resp.headers().get("location").and_then(|v| v.to_str().ok()) {
                current_url = location.to_string();
            } else {
                break;
            }
        } else {
            break;
        }
    }

    Ok(())
}

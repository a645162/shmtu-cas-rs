pub mod expr_resolver;
pub mod manual_resolver;
pub mod ocr;
pub mod ocr_resolver;
pub mod resolver;

pub use expr_resolver::ExprCaptchaResolver;
pub use manual_resolver::ManualCaptchaResolver;
pub use ocr::CaptchaOcr;
pub use ocr_resolver::OcrCaptchaResolver;
pub use resolver::{CaptchaAnswer, CaptchaAnswerKind, CaptchaResolver, ResolveFuture};

use anyhow::{Context, Result};
use reqwest::Client;

const CAPTCHA_URL: &str = "https://cas.shmtu.edu.cn/cas/captcha";

/// 拉取验证码图片字节。对齐 C# 的 `Captcha.GetImageDataFromUrlUsingGet`。
pub async fn fetch_captcha(client: &Client) -> Result<Vec<u8>> {
    let resp = client
        .get(CAPTCHA_URL)
        .send()
        .await
        .context("获取验证码失败")?;

    if resp.status() != reqwest::StatusCode::OK {
        anyhow::bail!("获取验证码失败，状态码: {}", resp.status());
    }

    let image_data = resp.bytes().await?.to_vec();
    Ok(image_data)
}

/// 把 `12+34=46` 这样的算式取右侧答案 `46`；找不到 `=` 则按原样 trim 返回。
pub fn get_expr_result(expr: &str) -> String {
    if let Some(pos) = expr.rfind('=') {
        expr[pos + 1..].trim().to_string()
    } else {
        expr.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_expr_result() {
        assert_eq!(get_expr_result("12+34=46"), "46");
        assert_eq!(get_expr_result("3+5=8"), "8");
        assert_eq!(get_expr_result("10-3=7"), "7");
        assert_eq!(get_expr_result("6*9=54"), "54");
        assert_eq!(get_expr_result("42"), "42");
    }
}

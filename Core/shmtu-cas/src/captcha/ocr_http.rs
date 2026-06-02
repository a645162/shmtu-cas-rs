use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::time::Duration;

/// 通过 RESTful HTTP OCR 服务识别验证码。
#[derive(Debug, Clone)]
pub struct CaptchaOcrHttp {
    base_url: String,
    async_client: reqwest::Client,
}

/// RESTful OCR 响应体
#[derive(Debug, serde::Deserialize)]
struct OcrHttpResponse {
    success: bool,
    expression: Option<String>,
    #[allow(dead_code)]
    result: Option<i32>,
    error: Option<String>,
}

impl CaptchaOcrHttp {
    fn build_blocking_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default()
    }

    pub fn new(base_url: &str) -> Self {
        let async_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            async_client,
        }
    }

    fn decode_response(response: OcrHttpResponse) -> Result<String> {
        if response.success {
            response
                .expression
                .ok_or_else(|| anyhow::anyhow!("OCR成功但未返回表达式"))
        } else {
            let err_msg = response.error.unwrap_or_else(|| "未知错误".to_string());
            Err(anyhow::anyhow!("RESTful OCR识别失败: {}", err_msg))
        }
    }

    /// 通过 HTTP POST 识别验证码图片。
    pub fn ocr_by_http(&self, image_data: &[u8]) -> Result<String> {
        let base64_image = BASE64.encode(image_data);
        let body = serde_json::json!({ "imageBase64": base64_image });
        let blocking_client = Self::build_blocking_client();

        let response: OcrHttpResponse = blocking_client
            .post(format!("{}/api/ocr", self.base_url))
            .json(&body)
            .send()
            .context("连接RESTful OCR服务器失败")?
            .json()
            .context("解析OCR响应失败")?;

        Self::decode_response(response)
    }

    pub async fn ocr_by_http_async(&self, image_data: &[u8]) -> Result<String> {
        let base64_image = BASE64.encode(image_data);
        let body = serde_json::json!({ "imageBase64": base64_image });

        let response: OcrHttpResponse = self
            .async_client
            .post(format!("{}/api/ocr", self.base_url))
            .json(&body)
            .send()
            .await
            .context("连接RESTful OCR服务器失败")?
            .json()
            .await
            .context("解析OCR响应失败")?;

        Self::decode_response(response)
    }

    /// 带重试的 HTTP OCR 识别。
    pub fn ocr_auto_retry(&self, image_data: &[u8], max_retries: usize) -> Result<String> {
        let mut last_error = None;
        for i in 0..max_retries {
            match self.ocr_by_http(image_data) {
                Ok(result) if !result.is_empty() => return Ok(result),
                Ok(_) => {
                    last_error = Some(anyhow::anyhow!("OCR返回空结果"));
                }
                Err(e) => {
                    eprintln!("第{}次RESTful OCR尝试失败: {}", i + 1, e);
                    last_error = Some(e);
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("RESTful OCR在{}次重试后失败", max_retries)))
    }

    pub async fn ocr_auto_retry_async(&self, image_data: &[u8], max_retries: usize) -> Result<String> {
        let mut last_error = None;
        for i in 0..max_retries {
            match self.ocr_by_http_async(image_data).await {
                Ok(result) if !result.is_empty() => return Ok(result),
                Ok(_) => {
                    last_error = Some(anyhow::anyhow!("OCR返回空结果"));
                }
                Err(e) => {
                    eprintln!("第{}次RESTful OCR尝试失败: {}", i + 1, e);
                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("RESTful OCR在{}次重试后失败", max_retries)))
    }

    /// 检查 RESTful OCR 服务健康状态。
    pub fn health_check(&self) -> Result<bool> {
        let blocking_client = Self::build_blocking_client();
        let resp = blocking_client
            .get(format!("{}/api/health", self.base_url))
            .send()
            .context("连接RESTful OCR健康检查失败")?;

        Ok(resp.status().is_success())
    }

    pub async fn health_check_async(&self) -> Result<bool> {
        let resp = self
            .async_client
            .get(format!("{}/api/health", self.base_url))
            .send()
            .await
            .context("连接RESTful OCR健康检查失败")?;

        Ok(resp.status().is_success())
    }
}

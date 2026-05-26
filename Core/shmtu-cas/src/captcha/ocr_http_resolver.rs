use anyhow::Result;

use super::ocr_http::CaptchaOcrHttp;
use super::resolver::{CaptchaAnswer, CaptchaResolver, ResolveFuture};

/// 通过 RESTful HTTP OCR 服务识别验证码。
pub struct OcrHttpCaptchaResolver {
    ocr: CaptchaOcrHttp,
    max_retries: usize,
}

impl OcrHttpCaptchaResolver {
    pub fn new(base_url: &str) -> Self {
        Self {
            ocr: CaptchaOcrHttp::new(base_url),
            max_retries: 3,
        }
    }

    pub fn with_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn from_ocr(ocr: CaptchaOcrHttp) -> Self {
        Self { ocr, max_retries: 3 }
    }
}

impl CaptchaResolver for OcrHttpCaptchaResolver {
    fn resolve<'a>(&'a self, image_data: &'a [u8]) -> ResolveFuture<'a> {
        let max_retries = self.max_retries;
        let owned = image_data.to_vec();
        Box::pin(async move {
            let expr = tokio::task::spawn_blocking({
                let ocr = self.ocr.clone();
                move || -> Result<String> { ocr.ocr_auto_retry(&owned, max_retries) }
            })
            .await??;
            Ok(CaptchaAnswer::expression(expr))
        })
    }
}

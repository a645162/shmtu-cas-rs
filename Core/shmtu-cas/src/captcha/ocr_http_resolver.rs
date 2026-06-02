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
        Box::pin(async move {
            let expr = self.ocr.ocr_auto_retry_async(image_data, max_retries).await?;
            Ok(CaptchaAnswer::expression(expr))
        })
    }
}

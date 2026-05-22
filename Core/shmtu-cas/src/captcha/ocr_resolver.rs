use anyhow::Result;

use super::ocr::CaptchaOcr;
use super::resolver::{CaptchaAnswer, CaptchaResolver, ResolveFuture};

/// 通过远端 TCP OCR 服务识别验证码。对齐 C# 的 `RemoteOcrCaptchaResolver`。
pub struct OcrCaptchaResolver {
    ocr: CaptchaOcr,
    max_retries: usize,
}

impl OcrCaptchaResolver {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            ocr: CaptchaOcr::new(&host.into(), port),
            max_retries: 3,
        }
    }

    pub fn with_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn from_ocr(ocr: CaptchaOcr) -> Self {
        Self { ocr, max_retries: 3 }
    }
}

impl CaptchaResolver for OcrCaptchaResolver {
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

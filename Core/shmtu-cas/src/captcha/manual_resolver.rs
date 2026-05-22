use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

use super::resolver::{CaptchaAnswer, CaptchaResolver, ResolveFuture};

type ManualHandler =
    Box<dyn for<'a> Fn(&'a [u8]) -> Pin<Box<dyn Future<Output = Result<CaptchaAnswer>> + Send + 'a>>
        + Send
        + Sync>;

/// 把验证码图片交给用户/外部回调拿到答案。对齐 C# 的 `ManualCaptchaResolver`。
pub struct ManualCaptchaResolver {
    handler: ManualHandler,
}

impl ManualCaptchaResolver {
    pub fn new(handler: ManualHandler) -> Self {
        Self { handler }
    }
}

impl CaptchaResolver for ManualCaptchaResolver {
    fn resolve<'a>(&'a self, image_data: &'a [u8]) -> ResolveFuture<'a> {
        (self.handler)(image_data)
    }
}

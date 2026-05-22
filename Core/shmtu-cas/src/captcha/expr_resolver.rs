use super::get_expr_result;
use super::resolver::{CaptchaAnswer, CaptchaResolver, ResolveFuture};

/// 调用方传入的算式字符串解析为答案。一般用于"已经在外部识别好算式"的场景。
pub struct ExprCaptchaResolver<F>
where
    F: Fn(&[u8]) -> String + Send + Sync,
{
    expr_provider: F,
}

impl<F> ExprCaptchaResolver<F>
where
    F: Fn(&[u8]) -> String + Send + Sync,
{
    pub fn new(expr_provider: F) -> Self {
        Self { expr_provider }
    }
}

impl<F> CaptchaResolver for ExprCaptchaResolver<F>
where
    F: Fn(&[u8]) -> String + Send + Sync,
{
    fn resolve<'a>(&'a self, image_data: &'a [u8]) -> ResolveFuture<'a> {
        Box::pin(async move {
            let expr = (self.expr_provider)(image_data);
            let answer = get_expr_result(&expr);
            Ok(CaptchaAnswer::answer(answer))
        })
    }
}

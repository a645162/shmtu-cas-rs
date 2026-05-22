use anyhow::Result;
use std::future::Future;
use std::pin::Pin;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptchaAnswerKind {
    /// 一个完整算式（如 "12+34="），调用方还需要计算
    Expression,
    /// 已经是最终答案
    Answer,
}

#[derive(Debug, Clone)]
pub struct CaptchaAnswer {
    pub value: String,
    pub kind: CaptchaAnswerKind,
}

impl CaptchaAnswer {
    pub fn new(value: impl Into<String>, kind: CaptchaAnswerKind) -> Self {
        Self {
            value: value.into(),
            kind,
        }
    }

    pub fn answer(value: impl Into<String>) -> Self {
        Self::new(value, CaptchaAnswerKind::Answer)
    }

    pub fn expression(value: impl Into<String>) -> Self {
        Self::new(value, CaptchaAnswerKind::Expression)
    }

    /// 不论是答案还是算式，都规约为最终答案字符串
    pub fn into_final_answer(self) -> String {
        match self.kind {
            CaptchaAnswerKind::Answer => self.value,
            CaptchaAnswerKind::Expression => super::get_expr_result(&self.value),
        }
    }
}

pub type ResolveFuture<'a> = Pin<Box<dyn Future<Output = Result<CaptchaAnswer>> + Send + 'a>>;

/// 验证码解析器抽象。与 C# 的 `ICaptchaResolver` 对齐。
pub trait CaptchaResolver: Send + Sync {
    fn resolve<'a>(&'a self, image_data: &'a [u8]) -> ResolveFuture<'a>;
}

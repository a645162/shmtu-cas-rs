//! 统一 ONNX 推理后端入口
//!
//! 提供 `OcrBackend` 枚举，可以根据 `crate::ModelVersion` 选择加载 v1 或 v2 实现。
//! 旧名 `CasOnnxBackend` 保留为 type alias（指向 v1），保持现有 CLI/工具链可继续工作。

use anyhow::{Context, Result};
use std::path::Path;

use crate::ModelVersion;

pub mod v1;
pub mod v2;

pub use v1::V1Backend;
pub use v2::V2Backend;

/// 旧名字：等价于 v1 后端（保持向后兼容）。
pub type CasOnnxBackend = V1Backend;

/// 统一 OCR 推理后端。
///
/// 统一提供 `predict_bytes` / `predict_file` / `predict_validate_code` 三个对外方法。
/// `load(version, dir)` 根据版本自动选 v1/v2 实现。
pub enum OcrBackend {
    V1(V1Backend),
    V2(V2Backend),
}

impl OcrBackend {
    /// 根据版本从目录加载对应 ONNX 后端。
    pub fn load(version: ModelVersion, dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        match version {
            ModelVersion::V1 => Ok(Self::V1(V1Backend::load(dir).context("加载 v1 ONNX 后端失败")?)),
            ModelVersion::V2 => Ok(Self::V2(V2Backend::load(dir).context("加载 v2 ONNX 后端失败")?)),
        }
    }

    /// 当前后端对应的模型版本。
    pub fn version(&self) -> ModelVersion {
        match self {
            Self::V1(_) => ModelVersion::V1,
            Self::V2(_) => ModelVersion::V2,
        }
    }

    /// 检查指定版本对应的所有模型文件是否齐全。
    pub fn check_model_exists(version: ModelVersion, dir: impl AsRef<Path>) -> bool {
        match version {
            ModelVersion::V1 => V1Backend::check_model_exists(dir),
            ModelVersion::V2 => V2Backend::check_model_exists(dir),
        }
    }

    /// 列出指定版本缺失的模型文件（v1 返回 `&'static str` 列表，v2 返回 String 列表，统一为 String）。
    pub fn missing_model_files(version: ModelVersion, dir: impl AsRef<Path>) -> Vec<String> {
        match version {
            ModelVersion::V1 => V1Backend::missing_model_files(dir)
                .into_iter()
                .map(str::to_string)
                .collect(),
            ModelVersion::V2 => V2Backend::missing_model_files(dir),
        }
    }

    pub fn predict_validate_code(&mut self, img: &image::DynamicImage) -> Result<crate::OcrResult> {
        match self {
            Self::V1(b) => b.predict_validate_code(img),
            Self::V2(b) => b.predict_validate_code(img),
        }
    }

    pub fn predict_file(&mut self, path: impl AsRef<Path>) -> Result<crate::OcrResult> {
        match self {
            Self::V1(b) => b.predict_file(path),
            Self::V2(b) => b.predict_file(path),
        }
    }

    pub fn predict_bytes(&mut self, data: &[u8]) -> Result<crate::OcrResult> {
        match self {
            Self::V1(b) => b.predict_bytes(data),
            Self::V2(b) => b.predict_bytes(data),
        }
    }
}

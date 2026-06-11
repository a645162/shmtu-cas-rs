//! `model-assets.json` v2 schema 解析。
//!
//! 新版 (schema_version >= 2) manifest 是 **多模型** 结构：
//!
//! ```jsonc
//! {
//!   "schema_version": 2,
//!   "modellist": ["mobilenet_v3_small.trislot_decoder.v2_0", ...],
//!   "models": [
//!     {
//!       "asset_stem": "mobilenet_v3_small.trislot_decoder.v2_0",
//!       "version": "2.0",
//!       "family": "trislot_decoder",
//!       "display_name": "CAS OCR TriSlot Decoder",
//!       "backbone": "mobilenet_v3_small",
//!       "supported_backbones": ["mobilenet_v3_small", "mobilenetv4_conv_small"],
//!       "model_size_m": 2.5,
//!       "metrics": { "val_acc_expression": 0.9512, ... },
//!       "artifacts": {
//!         "pytorch": { "fp32": { "engine": "pytorch", "files": [...] } },
//!         "onnx":    { "fp16": {...}, "fp32": {...} },
//!         "ncnn":    { "fp32": {...} }
//!       }
//!     }
//!   ],
//!   // 旧版平铺数组,新 manifest 也会保留,便于老 client 兼容
//!   "artifacts": [...],
//!   "digests": [...]
//! }
//! ```
//!
//! 本模块提供：
//! - [`V2Manifest`]  顶层 manifest 结构
//! - [`V2ModelEntry`] 单个模型分组条目
//! - [`V2FlatArtifact`] 旧版平铺条目 (fallback)
//! - [`ModelInfo`] / [`ModelMetrics`] 公共 API,供 Tauri command 序列化给前端
//! - [`list_models_from_manifest`] 从 manifest 抽取 `ModelInfo` 列表
//! - [`find_artifact_in_model`] 在 `V2ModelEntry` 中按 engine/precision 查找文件

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

/// 顶层 v2 manifest 解析。所有字段都用 `#[serde(default)]` 以兼容旧版 schema。
#[derive(Debug, Clone, Deserialize)]
pub struct V2Manifest {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub modellist: Vec<String>,
    #[serde(default)]
    pub models: Vec<V2ModelEntry>,
    /// 旧版平铺数组(没有 `models` 字段时作为 fallback)。
    #[serde(default)]
    pub artifacts: Vec<V2FlatArtifact>,
    /// 顶层 digest 列表(可选,本模块不解析内容,仅保留兼容)。
    #[serde(default)]
    pub digests: Vec<serde_json::Value>,
}

impl V2Manifest {
    /// 是否为新格式 (有 `models` 字段且非空)。
    pub fn has_grouped_models(&self) -> bool {
        !self.models.is_empty()
    }
}

/// 旧版/平铺的 artifact 条目(`models` 字段缺失时使用)。
#[derive(Debug, Clone, Deserialize)]
pub struct V2FlatArtifact {
    #[serde(default)]
    pub version: String,
    pub family: String,
    pub backbone: String,
    #[serde(default)]
    pub engine: String,
    pub precision: String,
    #[serde(default)]
    pub format: String,
    pub files: Vec<V2ArtifactFile>,
}

/// 单个文件条目。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct V2ArtifactFile {
    #[allow(dead_code)]
    pub path: String,
    pub sha256: String,
    pub release_asset_name: String,
}

/// 单个模型分组条目(`models[].artifacts` 形式,按 engine -> precision 嵌套)。
#[derive(Debug, Clone, Deserialize)]
pub struct V2ModelEntry {
    pub asset_stem: String,
    #[serde(default)]
    pub display_name: String,
    pub backbone: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub model_size_m: Option<f64>,
    #[serde(default)]
    pub metrics: Option<RawModelMetrics>,
    #[serde(default)]
    pub supported_backbones: Vec<String>,
    /// `engine -> precision -> V2ArtifactEntry` 的 JSON 结构,故意用 `serde_json::Value`
    /// 以避免对所有 engine/precision 组合做穷举式建模。
    pub artifacts: serde_json::Value,
}

/// 内部 metrics 解析(所有字段可选)。
#[derive(Debug, Clone, Deserialize)]
pub struct RawModelMetrics {
    #[serde(default)]
    pub val_acc_expression: Option<f64>,
    #[serde(default)]
    pub val_loss: Option<f64>,
    #[serde(default)]
    pub test_acc_expression: Option<f64>,
    #[serde(default)]
    pub test_loss: Option<f64>,
}

/// 内部单个 artifact 解析(对应 `models[].artifacts[engine][precision]` 叶子)。
#[derive(Debug, Clone, Deserialize)]
pub struct V2ArtifactEntry {
    #[allow(dead_code)]
    pub engine: String,
    #[serde(default)]
    pub precision: String,
    #[serde(default)]
    pub format: String,
    pub files: Vec<V2ArtifactFile>,
}

// ---- 公共 API 类型(给前端用)----

/// 模型展示信息(供 Tauri command 序列化给前端)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub asset_stem: String,
    pub display_name: String,
    pub backbone: String,
    pub version: String,
    pub family: String,
    #[serde(default)]
    pub model_size_m: Option<f64>,
    #[serde(default)]
    pub metrics: Option<ModelMetrics>,
    #[serde(default)]
    pub supported_backbones: Vec<String>,
}

/// 公开 metrics(所有字段可选)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetrics {
    #[serde(default)]
    pub val_acc_expression: Option<f64>,
    #[serde(default)]
    pub val_loss: Option<f64>,
    #[serde(default)]
    pub test_acc_expression: Option<f64>,
    #[serde(default)]
    pub test_loss: Option<f64>,
}

impl From<RawModelMetrics> for ModelMetrics {
    fn from(r: RawModelMetrics) -> Self {
        Self {
            val_acc_expression: r.val_acc_expression,
            val_loss: r.val_loss,
            test_acc_expression: r.test_acc_expression,
            test_loss: r.test_loss,
        }
    }
}

// ---- 函数 ----

/// 从 manifest 抽取 `ModelInfo` 列表。
///
/// - 优先从 `models[]` 解析;
/// - 若 `models` 为空,尝试从平铺 `artifacts[]` 推断(每个 backbone 视为一个模型)。
pub fn list_models_from_manifest(manifest: &V2Manifest) -> Vec<ModelInfo> {
    if !manifest.models.is_empty() {
        return manifest.models.iter().map(model_entry_to_info).collect();
    }

    // Fallback: 从平铺 artifacts 推断。
    // 同一 (family, backbone) 只保留一个 ModelInfo,artifacts 列表累积为内部表达。
    use std::collections::BTreeMap;
    let mut by_key: BTreeMap<(String, String), ModelInfo> = BTreeMap::new();
    for art in &manifest.artifacts {
        let key = (art.family.clone(), art.backbone.clone());
        by_key
            .entry(key)
            .or_insert_with(|| ModelInfo {
                asset_stem: format!("{}.{}", art.backbone, art.family),
                display_name: art.backbone.clone(),
                backbone: art.backbone.clone(),
                version: art.version.clone(),
                family: art.family.clone(),
                model_size_m: None,
                metrics: None,
                supported_backbones: vec![art.backbone.clone()],
            });
    }
    by_key.into_values().collect()
}

fn model_entry_to_info(entry: &V2ModelEntry) -> ModelInfo {
    ModelInfo {
        asset_stem: entry.asset_stem.clone(),
        display_name: if entry.display_name.is_empty() {
            entry.backbone.clone()
        } else {
            entry.display_name.clone()
        },
        backbone: entry.backbone.clone(),
        version: entry.version.clone(),
        family: entry.family.clone(),
        model_size_m: entry.model_size_m,
        metrics: entry.metrics.clone().map(ModelMetrics::from),
        supported_backbones: if entry.supported_backbones.is_empty() {
            vec![entry.backbone.clone()]
        } else {
            entry.supported_backbones.clone()
        },
    }
}

/// 在 `V2ModelEntry` 的分组 artifacts 中按 `(engine, precision)` 查找文件。
///
/// 命中后返回第一个文件 + 解析得到的 entry 引用。
pub fn find_artifact_in_model(
    model: &V2ModelEntry,
    engine: &str,
    precision: &str,
) -> Result<(V2ArtifactFile, V2ArtifactEntry)> {
    let engines = model
        .artifacts
        .as_object()
        .ok_or_else(|| anyhow!("model.artifacts 不是 object: {}", model.asset_stem))?;
    let engine_val = engines
        .get(engine)
        .ok_or_else(|| anyhow!("model {} 缺少 engine={}", model.asset_stem, engine))?;
    let precisions = engine_val
        .as_object()
        .ok_or_else(|| anyhow!("model {} engine={} 不是 object", model.asset_stem, engine))?;
    let prec_val = precisions.get(precision).ok_or_else(|| {
        anyhow!(
            "model {} engine={} 缺少 precision={}",
            model.asset_stem,
            engine,
            precision
        )
    })?;
    let entry: V2ArtifactEntry = serde_json::from_value(prec_val.clone()).with_context(|| {
        format!(
            "解析 model={} engine={} precision={} 失败",
            model.asset_stem, engine, precision
        )
    })?;
    let file = entry
        .files
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("model {} 命中条目无 files", model.asset_stem))?;
    Ok((file, entry))
}

/// 在 `V2Manifest` 中按 `asset_stem` 查找模型条目。
pub fn find_model_by_stem<'a>(
    manifest: &'a V2Manifest,
    asset_stem: &str,
) -> Option<&'a V2ModelEntry> {
    manifest.models.iter().find(|m| m.asset_stem == asset_stem)
}

/// 在 `V2Manifest` 中按 `(family, backbone)` 查找模型条目。
pub fn find_model_by_backbone<'a>(
    manifest: &'a V2Manifest,
    family: &str,
    backbone: &str,
) -> Option<&'a V2ModelEntry> {
    manifest
        .models
        .iter()
        .find(|m| m.family == family && m.backbone == backbone)
}

// ---- tests ----
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_manifest() -> V2Manifest {
        let raw = json!({
            "schema_version": 2,
            "modellist": ["mobilenet_v3_small.trislot_decoder.v2_0"],
            "models": [{
                "asset_stem": "mobilenet_v3_small.trislot_decoder.v2_0",
                "display_name": "CAS OCR TriSlot Decoder",
                "version": "2.0",
                "family": "trislot_decoder",
                "backbone": "mobilenet_v3_small",
                "supported_backbones": ["mobilenet_v3_small", "mobilenetv4_conv_small"],
                "model_size_m": 2.5,
                "metrics": {
                    "val_acc_expression": 0.9512,
                    "val_loss": 0.0234,
                    "test_acc_expression": 0.9489,
                    "test_loss": 0.0251
                },
                "artifacts": {
                    "onnx": {
                        "fp16": {
                            "engine": "onnx",
                            "precision": "fp16",
                            "format": "onnx",
                            "files": [{
                                "path": "onnx/mobilenet_v3_small.trislot_decoder.v2_0.fp16.onnx",
                                "release_asset_name": "mobilenet_v3_small.trislot_decoder.v2_0.fp16.onnx",
                                "sha256": "deadbeef"
                            }]
                        }
                    }
                }
            }],
            "artifacts": [],
            "digests": []
        });
        serde_json::from_value(raw).expect("sample manifest parses")
    }

    #[test]
    fn parses_grouped_manifest() {
        let m = sample_manifest();
        assert_eq!(m.schema_version, 2);
        assert!(m.has_grouped_models());
        assert_eq!(m.models.len(), 1);
        assert_eq!(m.models[0].backbone, "mobilenet_v3_small");
        assert_eq!(m.models[0].metrics.as_ref().unwrap().val_acc_expression, Some(0.9512));
    }

    #[test]
    fn list_models_returns_info() {
        let m = sample_manifest();
        let infos = list_models_from_manifest(&m);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].asset_stem, "mobilenet_v3_small.trislot_decoder.v2_0");
        assert_eq!(infos[0].supported_backbones.len(), 2);
        assert_eq!(infos[0].model_size_m, Some(2.5));
    }

    #[test]
    fn find_artifact_in_model_ok() {
        let m = sample_manifest();
        let model = &m.models[0];
        let (file, entry) = find_artifact_in_model(model, "onnx", "fp16").unwrap();
        assert_eq!(file.sha256, "deadbeef");
        assert_eq!(entry.engine, "onnx");
    }

    #[test]
    fn find_artifact_in_model_missing_engine() {
        let m = sample_manifest();
        let model = &m.models[0];
        assert!(find_artifact_in_model(model, "pytorch", "fp32").is_err());
    }

    #[test]
    fn find_artifact_in_model_missing_precision() {
        let m = sample_manifest();
        let model = &m.models[0];
        assert!(find_artifact_in_model(model, "onnx", "fp32").is_err());
    }

    #[test]
    fn fallback_to_flat_artifacts() {
        let raw = json!({
            "schema_version": 1,
            "artifacts": [
                {
                    "version": "2.0",
                    "family": "trislot_decoder",
                    "backbone": "mobilenet_v3_small",
                    "engine": "onnx",
                    "precision": "fp16",
                    "format": "onnx",
                    "files": [{
                        "path": "x.onnx",
                        "release_asset_name": "x.onnx",
                        "sha256": "abc"
                    }]
                }
            ]
        });
        let m: V2Manifest = serde_json::from_value(raw).unwrap();
        assert!(!m.has_grouped_models());
        let infos = list_models_from_manifest(&m);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].backbone, "mobilenet_v3_small");
        assert_eq!(infos[0].family, "trislot_decoder");
        assert!(infos[0].metrics.is_none());
    }

    #[test]
    fn missing_optional_fields_default() {
        let raw = json!({
            "schema_version": 2,
            "models": [{
                "asset_stem": "x.y.v2_0",
                "backbone": "mobilenet_v3_small",
                "artifacts": {}
            }]
        });
        let m: V2Manifest = serde_json::from_value(raw).unwrap();
        let entry = &m.models[0];
        assert_eq!(entry.display_name, "");
        assert_eq!(entry.version, "");
        assert_eq!(entry.family, "");
        assert!(entry.model_size_m.is_none());
        assert!(entry.metrics.is_none());
        assert!(entry.supported_backbones.is_empty());
    }
}

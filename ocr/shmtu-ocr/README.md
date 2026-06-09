# shmtu-ocr (Rust / Tauri)

Rust 端 ONNX 推理库，封装上海海事大学 CAS 验证码 OCR 识别，供 Tauri 桌面端本地推理使用。支持 v1（legacy）与 v2（**默认**）两套模型。

## 模型版本

| 版本 | 模型数量 | Backbone | 引擎 | 标签 | 默认 |
|---|---|---|---|---|---|
| v1 | 3 | resnet18 / resnet34 | ONNX Runtime | `v1.0-ONNX` | 否 |
| **v2** | **1** | `mobilenet_v3_small` | ONNX Runtime | `v2.0.x` | **是** |

详细对比与下载策略见根仓库 [Documents/docs/ocr-model-versions.md](../../../Documents/docs/ocr-model-versions.md)。

## 快速开始

```rust
use shmtu_ocr::{CasOcr, ModelVersion};

let ocr = CasOcr::builder()
    .model_version(ModelVersion::V2)        // 默认就是 V2，可省略
    .model_dir("./models")
    .use_gpu(cfg!(feature = "vulkan"))
    .build()?;

// 缺失则自动下载
ocr.ensure_models_async(None).await?;

// 加载到 ONNX Runtime
ocr.load_model()?;

// 推理
let result = ocr.predict(bitmap)?;
println!("{} = {}", result.expr, result.result);
```

## 切换到 v1

```rust
let ocr = CasOcr::builder()
    .model_version(ModelVersion::V1)        // 走老的 3 模型 ResNet 路径
    .model_dir("./models")
    .use_gpu(false)
    .build()?;
```

## 配置项

`shmtu-cas-rs/ocr/shmtu-ocr` 的 `const_value` 模块提供：

- `const_value::v1::*` -- v1 模型文件名、URL、SHA256SUMS
- `const_value::v2::*` -- v2 默认 tag (`v2.0.2`)、backbone、precision、清单文件名 (`model-assets.json`)

旧顶层常量（如 `MODEL_ONNX_EQUAL_FP32`）已 `#[deprecated]`，请迁移到 `v1` / `v2` 子模块。

## 下载策略

- **v1**：下载 3 个 ONNX 模型 + `SHA256SUMS.txt` 校验。
- **v2**：通过 release 根目录的 `model-assets.json` 清单按 `{tag, backbone, precision, engine}` 维度匹配资产并下载，使用清单内嵌的 `sha256` 校验。

GitHub 与 Gitee 互为 fallback。

## GPU 加速

通过 `vulkan` / `cuda` feature 启用 ONNX Runtime 的 GPU EP。设置面板可选择是否启用，并持久化到 `config.json`（`ocr.model_version` 与 `ocr.use_gpu`）。

## 返回值

`OcrResult`：

```rust
pub struct OcrResult {
    pub result: i32,         // 算式结果
    pub expr: String,        // 完整算式，如 "3 + 5 = 8"
    pub equal_symbol: EqualSymbol,  // v2 时为 EqualSymbol::NotApplicable
    pub operator: ExprOperator,
    pub digit1: i32,
    pub digit2: i32,
}
```

## 相关链接

- 根仓库 OCR 总览：[Documents/docs/ocr-model-versions.md](../../../Documents/docs/ocr-model-versions.md)
- 模型训练与导出：[shmtu-cas-ocr-model V2 文档](https://a645162.github.io/shmtu-cas-ocr-model/usage/v2-quickstart)

pub mod backend;
pub mod downloader;
pub mod image;

/// 模型文件名常量。对齐 C# 的 ConstValue。
///
/// 旧顶层常量（`MODEL_ONNX_EQUAL_FP32` 等）保留为 deprecated re-export，
/// 新代码请使用 `v1`/`v2` 子模块。
pub mod const_value {
    /// v1 模型: 3 个独立 ResNet ONNX。
    pub mod v1 {
        pub const MODEL_ONNX_EQUAL: &str = "resnet18_equal_symbol_latest.onnx";
        pub const MODEL_ONNX_OPERATOR: &str = "resnet18_operator_latest.onnx";
        pub const MODEL_ONNX_DIGIT: &str = "resnet34_digit_latest.onnx";
        pub const SHA256SUMS_FILE: &str = "SHA256SUMS.txt";
        pub const BASE_URL_GITEE: &str =
            "https://gitee.com/a645162/shmtu-cas-ocr-model/releases/download/v1.0-ONNX";
        pub const BASE_URL_GITHUB: &str =
            "https://github.com/a645162/shmtu-cas-ocr-model/releases/download/v1.0-ONNX";
        pub const CHECKSUM_URL: &str =
            "https://gitee.com/a645162/shmtu-cas-ocr-model/releases/download/v1.0-ONNX/SHA256SUMS.txt";
    }

    /// v2 模型: 单个 MobileNetV3 Tri-Slot Decoder（默认）。
    pub mod v2 {
        pub const DEFAULT_TAG: &str = "v2.0.2";
        pub const DEFAULT_BACKBONE: &str = "mobilenet_v3_small";
        pub const DEFAULT_PRECISION: &str = "fp16";
        pub const MODEL_FAMILY: &str = "trislot_decoder";
        pub const BASE_URL_GITHUB: &str =
            "https://github.com/a645162/shmtu-cas-ocr-model/releases/download";
        pub const BASE_URL_GITEE: &str =
            "https://gitee.com/a645162/shmtu-cas-ocr-model/releases/download";
        pub const MANIFEST_NAME: &str = "model-assets.json";

        /// 拼出模型文件名: `{backbone}.trislot_decoder.v2_0.{precision}.onnx`
        pub fn build_model_name(backbone: &str, precision: &str) -> String {
            format!("{}.{}.v2_0.{}.onnx", backbone, MODEL_FAMILY, precision)
        }
    }

    // ---- 旧顶层常量（deprecated, 仅作 re-export 以保持旧代码可编译）----
    #[deprecated(note = "请使用 const_value::v1::MODEL_ONNX_EQUAL")]
    pub const MODEL_ONNX_EQUAL_FP32: &str = v1::MODEL_ONNX_EQUAL;
    #[deprecated(note = "请使用 const_value::v1::MODEL_ONNX_OPERATOR")]
    pub const MODEL_ONNX_OPERATOR_FP32: &str = v1::MODEL_ONNX_OPERATOR;
    #[deprecated(note = "请使用 const_value::v1::MODEL_ONNX_DIGIT")]
    pub const MODEL_ONNX_DIGIT_FP32: &str = v1::MODEL_ONNX_DIGIT;
    #[deprecated(note = "请使用 const_value::v1::BASE_URL_GITEE")]
    pub const MODEL_ONNX_BASE_URL: &str = v1::BASE_URL_GITEE;
    #[deprecated(note = "请使用 const_value::v1::CHECKSUM_URL")]
    pub const MODEL_ONNX_CHECKSUM_URL: &str = v1::CHECKSUM_URL;
}

/// 模型版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ModelVersion {
    /// v1: 3 个独立 ResNet 模型（保留以兼容老用户）。
    V1,
    /// v2: 单个 MobileNetV3 Tri-Slot Decoder 模型（默认）。
    #[default]
    V2,
}

impl ModelVersion {
    /// 字符串表示 ("v1" / "v2")，用于配置/前端序列化。
    pub fn as_str(self) -> &'static str {
        match self {
            ModelVersion::V1 => "v1",
            ModelVersion::V2 => "v2",
        }
    }

    /// UI 展示用中文名。
    pub fn display_name(self) -> &'static str {
        match self {
            ModelVersion::V1 => "v1 (旧版, 3 模型 ResNet)",
            ModelVersion::V2 => "v2 (新版, 单模型 MobileNetV3)",
        }
    }

    /// 解析失败时回退到默认值（V2）。
    pub fn parse_or_default(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "v1" => ModelVersion::V1,
            "v2" => ModelVersion::V2,
            _ => ModelVersion::default(),
        }
    }
}

/// 等号类型。对齐 C# 的 CasExprEqualSymbol。
///
/// `NotApplicable` 用于 v2 模型（v2 不预测等号类型，模型本身在内部隐式处理）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualSymbol {
    Chs = 0,
    Symbol = 1,
    /// v2 模型专用：v2 不预测等号（算式表达由模型直接给出 digit/op/digit）。
    NotApplicable = -1,
}

/// 运算符类型。对齐 C# 的 CasExprOperator。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExprOperator {
    Add = 0,
    AddChs = 1,
    Sub = 2,
    SubChs = 3,
    Mul = 4,
    MulChs = 5,
}

impl ExprOperator {
    pub fn as_str(self) -> &'static str {
        match self {
            ExprOperator::Add | ExprOperator::AddChs => "+",
            ExprOperator::Sub | ExprOperator::SubChs => "-",
            ExprOperator::Mul | ExprOperator::MulChs => "×",
        }
    }

    pub fn calculate(self, digit1: i32, digit2: i32) -> i32 {
        match self {
            ExprOperator::Add | ExprOperator::AddChs => digit1 + digit2,
            ExprOperator::Sub | ExprOperator::SubChs => digit1 - digit2,
            ExprOperator::Mul | ExprOperator::MulChs => digit1 * digit2,
        }
    }
}

/// 验证码识别结果。对齐 C# 的 PredictValidateCode 返回元组。
#[derive(Debug, Clone)]
pub struct OcrResult {
    /// 最终答案（纯数字）
    pub result: i32,
    /// 完整算式，如 "3 + 5 = 8"
    pub expr: String,
    /// 等号类型（v2 时为 `EqualSymbol::NotApplicable`）
    pub equal_symbol: EqualSymbol,
    /// 运算符类型
    pub operator: ExprOperator,
    /// 第一个数字
    pub digit1: i32,
    /// 第二个数字
    pub digit2: i32,
}

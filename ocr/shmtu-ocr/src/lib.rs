pub mod backend;
pub mod image;

/// 模型文件名常量。对齐 C# 的 ConstValue。
pub mod const_value {
    pub const MODEL_ONNX_EQUAL_FP32: &str = "resnet18_equal_symbol_latest.onnx";
    pub const MODEL_ONNX_OPERATOR_FP32: &str = "resnet18_operator_latest.onnx";
    pub const MODEL_ONNX_DIGIT_FP32: &str = "resnet34_digit_latest.onnx";

    pub const MODEL_ONNX_BASE_URL: &str =
        "https://gitee.com/a645162/shmtu-cas-ocr-model/releases/download/v1.0-ONNX";
}

/// 等号类型。对齐 C# 的 CasExprEqualSymbol。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqualSymbol {
    Chs = 0,
    Symbol = 1,
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
    /// 等号类型
    pub equal_symbol: EqualSymbol,
    /// 运算符类型
    pub operator: ExprOperator,
    /// 第一个数字
    pub digit1: i32,
    /// 第二个数字
    pub digit2: i32,
}

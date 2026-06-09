use anyhow::{bail, Context, Result};
use image::DynamicImage;
use ort::session::Session;
use ort::value::Tensor;
use std::path::{Path, PathBuf};

use crate::const_value;

/// v1 ONNX 推理后端（3 个独立 ResNet 模型）。
///
/// 对齐 C# 的 CasOnnxBackend。文件名常量、URL 全部从 `crate::const_value::v1` 取，
/// 不再使用根级 v1 常量（已 deprecated，但仍 re-export 以保持旧代码可编译）。
pub struct V1Backend {
    session_equal_symbol: Session,
    session_operator: Session,
    session_digit: Session,
}

impl V1Backend {
    fn model_paths(dir: &Path) -> [(&'static str, PathBuf); 3] {
        [
            (
                const_value::v1::MODEL_ONNX_EQUAL,
                dir.join(const_value::v1::MODEL_ONNX_EQUAL),
            ),
            (
                const_value::v1::MODEL_ONNX_OPERATOR,
                dir.join(const_value::v1::MODEL_ONNX_OPERATOR),
            ),
            (
                const_value::v1::MODEL_ONNX_DIGIT,
                dir.join(const_value::v1::MODEL_ONNX_DIGIT),
            ),
        ]
    }

    /// 检查 v1 模型文件是否都存在。
    pub fn check_model_exists(dir: impl AsRef<Path>) -> bool {
        Self::missing_model_files(dir).is_empty()
    }

    /// 列出缺失的 v1 模型文件名。
    pub fn missing_model_files(dir: impl AsRef<Path>) -> Vec<&'static str> {
        Self::model_paths(dir.as_ref())
            .into_iter()
            .filter_map(|(name, path)| (!path.exists()).then_some(name))
            .collect()
    }

    /// 从目录加载三个 v1 ONNX 模型。
    pub fn load(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let [(_, equal_path), (_, operator_path), (_, digit_path)] = Self::model_paths(dir);

        let session_equal_symbol = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&equal_path)
            .with_context(|| format!("加载等号模型失败: {}", equal_path.display()))?;

        let session_operator = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&operator_path)
            .with_context(|| format!("加载运算符模型失败: {}", operator_path.display()))?;

        let session_digit = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&digit_path)
            .with_context(|| format!("加载数字模型失败: {}", digit_path.display()))?;

        Ok(Self {
            session_equal_symbol,
            session_operator,
            session_digit,
        })
    }

    /// 对单张裁切后的图片运行 ResNet 推理，返回 argmax 类别索引。
    fn predict_resnet(session: &mut Session, sub_image: &DynamicImage) -> Result<i32> {
        let input_array = crate::image::resnet_process::convert_image_to_tensor(sub_image);
        let input_tensor = Tensor::from_array(input_array).context("构造输入 tensor 失败")?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .context("ONNX 推理失败")?;

        let output = outputs[0]
            .try_extract_tensor::<f32>()
            .context("提取输出 tensor 失败")?;

        let (_shape, data) = output;

        let max_idx = data
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i as i32)
            .unwrap_or(-1);

        Ok(max_idx)
    }

    /// 对验证码图片执行完整识别流程。对齐 C# 的 PredictValidateCode。
    pub fn predict_validate_code(&mut self, img: &DynamicImage) -> Result<crate::OcrResult> {
        if img.width() == 0 || img.height() == 0 {
            bail!("输入图片为空");
        }

        // 1. 二值化 + 通道重映射
        let binary = crate::image::image_utils::convert_to_binary(img, 200);
        let remapped = crate::image::image_utils::revert_color(&binary);

        // 2. 裁切等号区域并推理
        let equal_img = crate::image::captcha_image::split_by_ratio(&remapped, 0.7, 1.0);
        let equal_cls = Self::predict_resnet(&mut self.session_equal_symbol, &equal_img)?;
        let equal_symbol = if equal_cls == 1 {
            crate::EqualSymbol::Symbol
        } else {
            crate::EqualSymbol::Chs
        };

        // 3. 根据等号类型选择裁切关键点（v1 不会产生 NotApplicable，给默认值兜底）
        let key_points = match equal_symbol {
            crate::EqualSymbol::Symbol => [0.25f32, 0.58, 0.75],
            crate::EqualSymbol::Chs | crate::EqualSymbol::NotApplicable => [0.15f32, 0.33, 0.46],
        };

        // 4. 裁切 digit1 / operator / digit2
        let digit1_img = crate::image::captcha_image::split_by_ratio(&remapped, 0.0, key_points[0]);
        let operator_img =
            crate::image::captcha_image::split_by_ratio(&remapped, key_points[0], key_points[1]);
        let digit2_img =
            crate::image::captcha_image::split_by_ratio(&remapped, key_points[1], key_points[2]);

        // 5. 推理
        let digit1 = Self::predict_resnet(&mut self.session_digit, &digit1_img)?;
        let operator_cls = Self::predict_resnet(&mut self.session_operator, &operator_img)?;
        let digit2 = Self::predict_resnet(&mut self.session_digit, &digit2_img)?;

        // 6. 映射运算符
        let operator = match operator_cls {
            0 => crate::ExprOperator::Add,
            1 => crate::ExprOperator::AddChs,
            2 => crate::ExprOperator::Sub,
            3 => crate::ExprOperator::SubChs,
            4 => crate::ExprOperator::Mul,
            5 => crate::ExprOperator::MulChs,
            _ => bail!("未知的运算符类别: {}", operator_cls),
        };

        // 7. 计算结果
        let result = operator.calculate(digit1, digit2);
        let expr = format!("{} {} {} = {}", digit1, operator.as_str(), digit2, result);

        Ok(crate::OcrResult {
            result,
            expr,
            equal_symbol,
            operator,
            digit1,
            digit2,
        })
    }

    /// 从文件路径识别验证码。
    pub fn predict_file(&mut self, path: impl AsRef<Path>) -> Result<crate::OcrResult> {
        let path = path.as_ref();
        let img = image::open(path).with_context(|| format!("打开图片失败: {}", path.display()))?;
        self.predict_validate_code(&img)
    }

    /// 从原始字节识别验证码（配合 shmtu-cas 的 fetch_captcha）。
    pub fn predict_bytes(&mut self, data: &[u8]) -> Result<crate::OcrResult> {
        let img = image::load_from_memory(data).context("解析图片字节失败")?;
        self.predict_validate_code(&img)
    }
}

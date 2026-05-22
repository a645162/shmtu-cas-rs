use anyhow::{Context, Result, bail};
use image::DynamicImage;
use ort::session::Session;
use ort::value::Tensor;

use crate::const_value;

/// ONNX 推理后端。对齐 C# 的 CasOnnxBackend。
pub struct CasOnnxBackend {
    session_equal_symbol: Session,
    session_operator: Session,
    session_digit: Session,
}

impl CasOnnxBackend {
    /// 检查模型文件是否都存在。
    pub fn check_model_exists(dir: &str) -> bool {
        std::path::Path::new(&format!("{}/{}", dir, const_value::MODEL_ONNX_EQUAL_FP32)).exists()
            && std::path::Path::new(&format!("{}/{}", dir, const_value::MODEL_ONNX_OPERATOR_FP32))
                .exists()
            && std::path::Path::new(&format!("{}/{}", dir, const_value::MODEL_ONNX_DIGIT_FP32))
                .exists()
    }

    /// 从目录加载三个 ONNX 模型。
    pub fn load(dir: &str) -> Result<Self> {
        let equal_path = format!("{}/{}", dir, const_value::MODEL_ONNX_EQUAL_FP32);
        let operator_path = format!("{}/{}", dir, const_value::MODEL_ONNX_OPERATOR_FP32);
        let digit_path = format!("{}/{}", dir, const_value::MODEL_ONNX_DIGIT_FP32);

        let session_equal_symbol = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&equal_path)
            .with_context(|| format!("加载等号模型失败: {}", equal_path))?;

        let session_operator = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&operator_path)
            .with_context(|| format!("加载运算符模型失败: {}", operator_path))?;

        let session_digit = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&digit_path)
            .with_context(|| format!("加载数字模型失败: {}", digit_path))?;

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

        // 3. 根据等号类型选择裁切关键点
        let key_points = match equal_symbol {
            crate::EqualSymbol::Symbol => [0.25f32, 0.58, 0.75],
            crate::EqualSymbol::Chs => [0.15f32, 0.33, 0.46],
        };

        // 4. 裁切 digit1 / operator / digit2
        let digit1_img =
            crate::image::captcha_image::split_by_ratio(&remapped, 0.0, key_points[0]);
        let operator_img = crate::image::captcha_image::split_by_ratio(
            &remapped,
            key_points[0],
            key_points[1],
        );
        let digit2_img = crate::image::captcha_image::split_by_ratio(
            &remapped,
            key_points[1],
            key_points[2],
        );

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
    pub fn predict_file(&mut self, path: &str) -> Result<crate::OcrResult> {
        let img = image::open(path).with_context(|| format!("打开图片失败: {}", path))?;
        self.predict_validate_code(&img)
    }

    /// 从原始字节识别验证码（配合 shmtu-cas 的 fetch_captcha）。
    pub fn predict_bytes(&mut self, data: &[u8]) -> Result<crate::OcrResult> {
        let img = image::load_from_memory(data).context("解析图片字节失败")?;
        self.predict_validate_code(&img)
    }
}

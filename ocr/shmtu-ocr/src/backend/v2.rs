use anyhow::{bail, Context, Result};
use image::imageops::FilterType;
use image::DynamicImage;
use ndarray::Array4;
use ort::session::Session;
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use tracing::info;

use crate::const_value;

/// v2 ONNX 推理后端（单个 MobileNetV3 Tri-Slot Decoder 模型）。
///
/// 输入: 灰度 1×64×192, 像素 [0, 1]
/// 输出: 3 个 tensor (按 index 取):
///   outputs[0] -> digit_left_logits  (1×10)
///   outputs[1] -> operator_logits    (1×3)  // 0=+, 1=-, 2=×
///   outputs[2] -> digit_right_logits (1×10)
pub struct V2Backend {
    session: Session,
    model_name: &'static str,
}

const V2_INPUT_W: u32 = 192;
const V2_INPUT_H: u32 = 64;

impl V2Backend {
    /// 默认模型文件名（基于 const_value::v2 默认值拼出）。
    pub fn default_model_name() -> String {
        const_value::v2::build_model_name(
            const_value::v2::DEFAULT_BACKBONE,
            const_value::v2::DEFAULT_PRECISION,
        )
    }

    fn model_path(dir: &Path) -> (String, PathBuf) {
        let name = Self::default_model_name();
        let path = dir.join(&name);
        (name, path)
    }

    /// 检查 v2 模型文件是否存在。
    pub fn check_model_exists(dir: impl AsRef<Path>) -> bool {
        let (_, path) = Self::model_path(dir.as_ref());
        path.exists()
    }

    /// 列出缺失的 v2 模型文件名（Vec<String> 因为是动态拼出来的）。
    pub fn missing_model_files(dir: impl AsRef<Path>) -> Vec<String> {
        let (name, path) = Self::model_path(dir.as_ref());
        if path.exists() {
            Vec::new()
        } else {
            vec![name]
        }
    }

    /// 加载 v2 单个 ONNX 模型。
    pub fn load(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref();
        let (name, path) = Self::model_path(dir);

        if !path.exists() {
            bail!(
                "v2 模型文件不存在: {}，请先下载",
                path.display()
            );
        }

        let session = Session::builder()
            .context("创建 ONNX session builder 失败")?
            .commit_from_file(&path)
            .with_context(|| format!("加载 v2 模型失败: {}", path.display()))?;

        // 输出 tensor 名字（debug 用）。按 index 取 outputs[0..3] 解析，无需名字。
        let output_names: Vec<String> = session
            .outputs()
            .iter()
            .map(|o| o.name().to_string())
            .collect();
        info!(
            "v2 ONNX 模型加载完成: {} (outputs: {:?})",
            path.display(),
            output_names
        );

        // 把 name 泄漏为 'static 字符串。V2Backend 整个生命周期持有，进程退出前有效。
        let model_name_static: &'static str = Box::leak(name.into_boxed_str());

        Ok(Self {
            session,
            model_name: model_name_static,
        })
    }

    /// 灰度 1×64×192 预处理。像素 / 255.0，无 mean/std。
    fn preprocess(&self, img: &DynamicImage) -> Result<Array4<f32>> {
        let resized = img.resize_exact(V2_INPUT_W, V2_INPUT_H, FilterType::Triangle);
        let gray = resized.to_luma8();

        let mut tensor = Array4::<f32>::zeros((1, 1, V2_INPUT_H as usize, V2_INPUT_W as usize));
        for y in 0..V2_INPUT_H {
            for x in 0..V2_INPUT_W {
                let p = gray.get_pixel(x, y)[0];
                tensor[[0, 0, y as usize, x as usize]] = p as f32 / 255.0;
            }
        }
        Ok(tensor)
    }

    /// 一次前向推理，返回 (digit1, operator_v2, digit2) 三个 argmax 索引。
    fn forward(&mut self, img: &DynamicImage) -> Result<(i32, i32, i32)> {
        let input_array = self.preprocess(img)?;
        let input_tensor = Tensor::from_array(input_array).context("构造 v2 输入 tensor 失败")?;

        let outputs = self
            .session
            .run(ort::inputs![input_tensor])
            .context("v2 ONNX 推理失败")?;

        if outputs.len() < 3 {
            bail!(
                "v2 模型输出数量不足: 期望 >= 3，实际 {}",
                outputs.len()
            );
        }

        let argmax = |idx: usize| -> Result<i32> {
            let (_shape, data) = outputs[idx]
                .try_extract_tensor::<f32>()
                .with_context(|| format!("提取 v2 outputs[{}] 失败", idx))?;
            Ok(data
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i as i32)
                .unwrap_or(-1))
        };

        let digit1 = argmax(0)?;
        let operator = argmax(1)?;
        let digit2 = argmax(2)?;
        Ok((digit1, operator, digit2))
    }

    /// 对验证码图片执行完整识别流程（v2 简化版：不需要切等号/裁切）。
    pub fn predict_validate_code(&mut self, img: &DynamicImage) -> Result<crate::OcrResult> {
        if img.width() == 0 || img.height() == 0 {
            bail!("输入图片为空");
        }

        let (digit1, operator_idx, digit2) = self.forward(img)?;

        // v2 运算符只有 3 类：0=+, 1=-, 2=×。映射为 v1 风格的 ExprOperator（CHS 分支给 NotApplicable 替代）。
        let operator = match operator_idx {
            0 => crate::ExprOperator::Add,
            1 => crate::ExprOperator::Sub,
            2 => crate::ExprOperator::Mul,
            _ => bail!("未知的 v2 运算符类别: {}", operator_idx),
        };

        let result = operator.calculate(digit1, digit2);
        let expr = format!("{} {} {} = {}", digit1, operator.as_str(), digit2, result);

        Ok(crate::OcrResult {
            result,
            expr,
            // v2 不预测等号类型，标记为 NotApplicable（v1 API 兼容）。
            equal_symbol: crate::EqualSymbol::NotApplicable,
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

    /// 当前加载的模型文件名（用于 UI/日志展示）。
    pub fn model_name(&self) -> &str {
        self.model_name
    }
}

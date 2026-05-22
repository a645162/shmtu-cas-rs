use image::DynamicImage;
use ndarray::{Array4, Ix4};

const MODEL_SIZE: u32 = 224;

/// ImageNet 均值 (R, G, B 顺序，与 C# 一致)
const MEAN: [f32; 3] = [123.675, 116.28, 103.53];
/// ImageNet 标准差倒数 (1/std)
const NORM: [f32; 3] = [1.0 / 58.395, 1.0 / 57.12, 1.0 / 57.375];

/// 将裁切后的子图 resize 到 224×224，再按 ImageNet 标准归一化为 NCHW tensor。
/// 对齐 C# 的 ResNetProcess.ConvertImageToTensor。
pub fn convert_image_to_tensor(img: &DynamicImage) -> Array4<f32> {
    let resized = img.resize_exact(
        MODEL_SIZE,
        MODEL_SIZE,
        image::imageops::FilterType::Triangle,
    );
    let rgb = resized.to_rgb8();

    let mut tensor = Array4::zeros(Ix4(1, 3, MODEL_SIZE as usize, MODEL_SIZE as usize));

    for y in 0..MODEL_SIZE as usize {
        for x in 0..MODEL_SIZE as usize {
            let pixel = rgb.get_pixel(x as u32, y as u32);
            for c in 0..3 {
                tensor[[0, c, y, x]] = (pixel[c] as f32 - MEAN[c]) * NORM[c];
            }
        }
    }

    tensor
}

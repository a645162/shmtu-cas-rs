use image::DynamicImage;

/// 按水平比例裁切图片。对齐 C# 的 CasCaptchaImage.SplitImgByRatio。
pub fn split_by_ratio(img: &DynamicImage, start_ratio: f32, end_ratio: f32) -> DynamicImage {
    let w = img.width();
    let h = img.height();

    let x_start = (w as f32 * start_ratio) as u32;
    let x_end = if end_ratio >= 1.0 {
        w
    } else {
        (w as f32 * end_ratio) as u32
    };

    let crop_w = x_end.saturating_sub(x_start);
    img.crop_imm(x_start, 0, crop_w, h)
}

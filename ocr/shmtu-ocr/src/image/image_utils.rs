use image::{DynamicImage, Rgb, RgbImage};

/// 二值化：luminance >= threshold → 白(255)，否则 → 黑(0)。
/// 对齐 C# 的 ImageUtils.ConvertImageToBinary。
pub fn convert_to_binary(img: &DynamicImage, threshold: u8) -> DynamicImage {
    let rgb = img.to_rgb8();
    let mut out = RgbImage::new(rgb.width(), rgb.height());

    for (x, y, pixel) in rgb.enumerate_pixels() {
        let lum = ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000)
            as u8;
        let val = if lum >= threshold { 255 } else { 0 };
        out.put_pixel(x, y, Rgb([val, val, val]));
    }

    DynamicImage::ImageRgb8(out)
}

/// 通道重映射：new_R = old_B, new_G = old_R, new_B = old_G。
/// 对齐 C# 的 ImageUtils.RevertImageColor。
pub fn revert_color(img: &DynamicImage) -> DynamicImage {
    let rgb = img.to_rgb8();
    let mut out = RgbImage::new(rgb.width(), rgb.height());

    for (x, y, pixel) in rgb.enumerate_pixels() {
        out.put_pixel(x, y, Rgb([pixel[2], pixel[0], pixel[1]]));
    }

    DynamicImage::ImageRgb8(out)
}

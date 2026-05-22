use eframe::egui::{self, TextureHandle, TextureOptions, Vec2};
use std::path::PathBuf;

pub(crate) fn default_model_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let sibling_models = cwd
        .ancestors()
        .find(|path| path.file_name().and_then(|name| name.to_str()) == Some("Terminal"))
        .map(|terminal_root| terminal_root.join("shmtu-terminal-desktop/Models"));

    if let Some(path) = sibling_models {
        if path.exists() {
            return path;
        }
    }

    cwd.join("Model")
}

pub(crate) fn texture_from_bytes(
    ctx: &egui::Context,
    name: String,
    bytes: &[u8],
) -> Option<(TextureHandle, [usize; 2])> {
    let decoded = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [decoded.width() as usize, decoded.height() as usize];
    let pixels = decoded.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    let texture = ctx.load_texture(name, color_image, TextureOptions::LINEAR);
    Some((texture, size))
}

pub(crate) fn fit_size(width: f32, height: f32, max_width: f32, max_height: f32) -> Vec2 {
    if width <= 0.0 || height <= 0.0 {
        return Vec2::new(max_width.max(1.0), max_height.max(1.0));
    }

    let scale = (max_width / width).min(max_height / height).max(0.01);
    Vec2::new(width * scale, height * scale)
}

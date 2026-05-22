use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, RichText, Vec2,
};
use std::fs;
use std::sync::Arc;

pub(crate) fn configure_visuals(ctx: &egui::Context) {
    configure_fonts(ctx);

    let mut visuals = egui::Visuals::light();
    visuals.override_text_color = Some(Color32::from_rgb(34, 46, 58));
    visuals.widgets.active.bg_fill = accent_color();
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(226, 236, 248);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(246, 248, 251);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(246, 248, 251);
    visuals.selection.bg_fill = accent_color();
    visuals.window_fill = Color32::from_rgb(250, 251, 253);
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 10.0);
    style.spacing.button_padding = Vec2::new(12.0, 10.0);
    style
        .text_styles
        .insert(egui::TextStyle::Heading, FontId::proportional(26.0));
    ctx.set_style(style);
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    if let Some((name, data)) = load_cjk_font() {
        fonts
            .font_data
            .insert(name.clone(), Arc::new(FontData::from_owned(data)));

        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, name.clone());
        }
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, name);
        }
    } else {
        eprintln!("shmtu-ocr-gui: 未找到可用中文字体，界面中文可能显示异常");
    }

    ctx.set_fonts(fonts);
}

fn load_cjk_font() -> Option<(String, Vec<u8>)> {
    let candidates = [
        (
            "droid_sans_fallback",
            "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        ),
        (
            "noto_sans_cjk_regular",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        ),
        ("wqy_zenhei", "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc"),
    ];

    for (name, path) in candidates {
        if let Ok(data) = fs::read(path) {
            eprintln!("shmtu-ocr-gui: 使用中文字体 {}", path);
            return Some((name.to_string(), data));
        }
    }

    None
}

pub(crate) fn accent_color() -> Color32 {
    Color32::from_rgb(26, 118, 210)
}

pub(crate) fn pill(ui: &mut egui::Ui, text: &str, fill: Color32) {
    Frame::new()
        .fill(fill)
        .corner_radius(999.0)
        .inner_margin(Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(Color32::WHITE));
        });
}

pub(crate) fn section_divider(ui: &mut egui::Ui) {
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
}

pub(crate) fn status_color(status: &str) -> Color32 {
    match status {
        "完成" => Color32::from_rgb(53, 114, 72),
        "失败" => Color32::from_rgb(170, 52, 47),
        _ => Color32::from_rgb(120, 130, 141),
    }
}

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, RichText, Vec2,
};
use std::fs;
use std::sync::Arc;

pub(crate) fn configure_visuals(ctx: &egui::Context, dark_mode: bool) {
    configure_fonts(ctx);

    let visuals = if dark_mode {
        let mut v = egui::Visuals::dark();
        v.override_text_color = Some(Color32::from_rgb(210, 216, 227));
        v.widgets.active.bg_fill = accent_color(true);
        v.widgets.hovered.bg_fill = Color32::from_rgb(42, 48, 62);
        v.widgets.inactive.bg_fill = Color32::from_rgb(37, 40, 48);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(37, 40, 48);
        v.selection.bg_fill = accent_color(true);
        v.window_fill = Color32::from_rgb(24, 27, 34);
        v
    } else {
        let mut v = egui::Visuals::light();
        v.override_text_color = Some(Color32::from_rgb(34, 46, 58));
        v.widgets.active.bg_fill = accent_color(false);
        v.widgets.hovered.bg_fill = Color32::from_rgb(226, 236, 248);
        v.widgets.inactive.bg_fill = Color32::from_rgb(246, 248, 251);
        v.widgets.inactive.weak_bg_fill = Color32::from_rgb(246, 248, 251);
        v.selection.bg_fill = accent_color(false);
        v.window_fill = Color32::from_rgb(250, 251, 253);
        v
    };
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

// --- Semantic color helpers (theme-aware) ---

pub(crate) fn accent_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(82, 158, 247)
    } else {
        Color32::from_rgb(26, 118, 210)
    }
}

pub(crate) fn card_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(37, 40, 48)
    } else {
        Color32::from_rgb(247, 249, 252)
    }
}

pub(crate) fn card_stroke(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(54, 58, 69)
    } else {
        Color32::from_rgb(218, 225, 232)
    }
}

pub(crate) fn surface_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(28, 31, 38)
    } else {
        Color32::WHITE
    }
}

pub(crate) fn surface_stroke(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(46, 50, 64)
    } else {
        Color32::from_rgb(221, 227, 235)
    }
}

pub(crate) fn dim_text(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(139, 147, 165)
    } else {
        Color32::from_rgb(100, 112, 126)
    }
}

pub(crate) fn panel_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(28, 31, 38)
    } else {
        Color32::from_rgb(248, 250, 252)
    }
}

pub(crate) fn central_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(24, 27, 34)
    } else {
        Color32::from_rgb(250, 251, 253)
    }
}

pub(crate) fn model_panel_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(26, 35, 52)
    } else {
        Color32::from_rgb(232, 240, 250)
    }
}

pub(crate) fn model_panel_stroke(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(40, 52, 72)
    } else {
        Color32::from_rgb(197, 213, 232)
    }
}

pub(crate) fn status_bar_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(30, 33, 40)
    } else {
        Color32::from_rgb(244, 246, 248)
    }
}

pub(crate) fn status_bar_stroke(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(50, 55, 65)
    } else {
        Color32::from_rgb(220, 226, 233)
    }
}

pub(crate) fn batch_item_bg(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(32, 36, 46)
    } else {
        Color32::from_rgb(241, 245, 249)
    }
}

pub(crate) fn batch_item_stroke(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(50, 55, 65)
    } else {
        Color32::from_rgb(220, 226, 233)
    }
}

pub(crate) fn success_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(76, 175, 122)
    } else {
        Color32::from_rgb(53, 114, 72)
    }
}

pub(crate) fn warning_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(218, 155, 45)
    } else {
        Color32::from_rgb(145, 84, 25)
    }
}

pub(crate) fn error_color(dark_mode: bool) -> Color32 {
    if dark_mode {
        Color32::from_rgb(224, 85, 85)
    } else {
        Color32::from_rgb(170, 52, 47)
    }
}

// --- Reusable UI components ---

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

pub(crate) fn status_color(status: &str, dark_mode: bool) -> Color32 {
    match status {
        "完成" => success_color(dark_mode),
        "失败" => error_color(dark_mode),
        _ => dim_text(dark_mode),
    }
}

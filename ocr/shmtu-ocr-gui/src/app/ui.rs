use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Layout, Margin, ProgressBar, RichText, ScrollArea,
    Sense, Stroke, Vec2,
};

use super::OcrGuiApp;
use crate::theme::{accent_color, pill, section_divider, status_color};
use crate::worker::WorkerCommand;

impl OcrGuiApp {
    fn draw_model_panel(&mut self, ui: &mut egui::Ui) {
        let frame = Frame::group(ui.style())
            .fill(Color32::from_rgb(232, 240, 250))
            .stroke(Stroke::new(1.0, Color32::from_rgb(197, 213, 232)))
            .inner_margin(Margin::same(14));

        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("模型目录").strong());
                    ui.add_space(6.0);
                    ui.add_enabled(
                        !self.is_busy,
                        egui::TextEdit::singleline(&mut self.model_dir)
                            .desired_width(ui.available_width().clamp(260.0, 640.0)),
                    );
                    ui.add_space(8.0);
                    if ui
                        .add_enabled(
                            !self.is_busy,
                            egui::Button::new("检查 / 下载模型").fill(accent_color()),
                        )
                        .clicked()
                    {
                        self.download_progress = 0.0;
                        self.models_ready = shmtu_ocr::backend::CasOnnxBackend::check_model_exists(
                            self.current_model_dir(),
                        );
                        self.send_command(
                            WorkerCommand::EnsureModels {
                                model_dir: self.current_model_dir(),
                            },
                            "正在检查 / 下载模型...".to_string(),
                        );
                    }

                    let badge_color = if self.models_ready {
                        Color32::from_rgb(53, 114, 72)
                    } else {
                        Color32::from_rgb(145, 84, 25)
                    };
                    pill(
                        ui,
                        if self.models_ready {
                            "模型已就绪"
                        } else {
                            "模型未就绪"
                        },
                        badge_color,
                    );
                });

                ui.add_space(8.0);
                ui.add(
                    ProgressBar::new(self.download_progress / 100.0).desired_width(f32::INFINITY),
                );
            });
        });
    }

    fn draw_status_bar(&self, ui: &mut egui::Ui) {
        let frame = Frame::group(ui.style())
            .fill(Color32::from_rgb(244, 246, 248))
            .stroke(Stroke::new(1.0, Color32::from_rgb(220, 226, 233)))
            .inner_margin(Margin::same(12));
        frame.show(ui, |ui| {
            ui.label(&self.status_message);
        });
    }

    fn draw_preview_panel(&mut self, ui: &mut egui::Ui, panel_height: f32) {
        let frame = Frame::group(ui.style())
            .fill(Color32::from_rgb(247, 249, 252))
            .stroke(Stroke::new(1.0, Color32::from_rgb(218, 225, 232)))
            .inner_margin(Margin::same(14));

        frame.show(ui, |ui| {
            ui.vertical(|ui| {
                let available_width = ui.available_width();
                let preview_height = (panel_height - 152.0).clamp(220.0, 620.0);
                let (rect, _) = ui.allocate_exact_size(
                    Vec2::new(available_width, preview_height),
                    Sense::hover(),
                );

                ui.painter()
                    .rect_filled(rect, 10.0, Color32::from_rgb(255, 255, 255));
                ui.painter().rect_stroke(
                    rect,
                    10.0,
                    Stroke::new(1.0, Color32::from_rgb(221, 227, 235)),
                    egui::StrokeKind::Outside,
                );

                if let Some(current) = &self.current {
                    if let (Some(texture), [width, height]) = (&current.texture, current.size) {
                        let desired = crate::util::fit_size(
                            width as f32,
                            height as f32,
                            rect.width() - 24.0,
                            rect.height() - 24.0,
                        );
                        let image_rect = Align2::CENTER_CENTER
                            .align_size_within_rect(desired, rect.shrink(12.0));
                        ui.painter().image(
                            texture.id(),
                            image_rect,
                            egui::Rect::from_min_max(
                                egui::Pos2::new(0.0, 0.0),
                                egui::Pos2::new(1.0, 1.0),
                            ),
                            Color32::WHITE,
                        );
                    } else {
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            "图片已加载\n但预览解码失败",
                            FontId::proportional(22.0),
                            Color32::from_rgb(124, 136, 151),
                        );
                    }
                } else {
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "拖入或打开一张验证码图片",
                        FontId::proportional(24.0),
                        Color32::from_rgb(124, 136, 151),
                    );
                }

                ui.add_space(10.0);
                let source = self
                    .current
                    .as_ref()
                    .map(|img| img.source.as_str())
                    .unwrap_or("（当前无图片）");
                ui.label(
                    RichText::new(source)
                        .small()
                        .color(Color32::from_rgb(116, 127, 141)),
                );
                ui.add_space(10.0);

                Frame::group(ui.style())
                    .fill(Color32::from_rgb(255, 255, 255))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(221, 227, 235)))
                    .inner_margin(Margin::same(14))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new("识别结果")
                                    .small()
                                    .color(Color32::from_rgb(120, 130, 141)),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(&self.result_expr)
                                    .size(34.0)
                                    .strong()
                                    .color(accent_color()),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("用时：{} 毫秒", self.current_elapsed_ms))
                                    .small()
                                    .color(Color32::from_rgb(120, 130, 141)),
                            );
                        });
                    });
            });
        });
    }

    fn draw_actions_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, panel_height: f32) {
        let frame = Frame::group(ui.style())
            .fill(Color32::from_rgb(247, 249, 252))
            .stroke(Stroke::new(1.0, Color32::from_rgb(218, 225, 232)))
            .inner_margin(Margin::same(14));

        frame.show(ui, |ui| {
            ScrollArea::vertical()
                .max_height(panel_height.max(220.0))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("获取图片").strong());
                        ui.add_space(8.0);
                        ui.add_enabled(
                            !self.is_busy,
                            egui::TextEdit::singleline(&mut self.captcha_url)
                                .desired_width(f32::INFINITY),
                        );
                        ui.add_space(8.0);

                        if ui
                            .add_enabled(
                                !self.is_busy,
                                egui::Button::new("下载验证码")
                                    .min_size(Vec2::new(ui.available_width(), 38.0)),
                            )
                            .clicked()
                        {
                            self.send_command(
                                WorkerCommand::DownloadCaptcha {
                                    url: self.captcha_url.trim().to_string(),
                                },
                                format!("正在下载验证码：{}", self.captcha_url.trim()),
                            );
                        }

                        if ui
                            .add_enabled(
                                !self.is_busy,
                                egui::Button::new("打开本地图片")
                                    .min_size(Vec2::new(ui.available_width(), 38.0)),
                            )
                            .clicked()
                        {
                            self.open_local_image(ctx);
                        }

                        section_divider(ui);
                        ui.label(RichText::new("识别").strong());
                        ui.add_space(8.0);

                        if ui
                            .add_enabled(
                                !self.is_busy && self.current.is_some(),
                                egui::Button::new(
                                    RichText::new("▶ OCR 识别")
                                        .size(20.0)
                                        .strong()
                                        .color(Color32::WHITE),
                                )
                                .fill(accent_color())
                                .min_size(Vec2::new(ui.available_width(), 56.0)),
                            )
                            .clicked()
                        {
                            self.start_recognize_current();
                        }

                        section_divider(ui);
                        ui.label(RichText::new("批量").strong());
                        ui.add_space(8.0);

                        if ui
                            .add_enabled(
                                !self.is_busy && self.current.is_some(),
                                egui::Button::new("加入批量列表")
                                    .min_size(Vec2::new(ui.available_width(), 34.0)),
                            )
                            .clicked()
                        {
                            self.add_current_to_batch(ctx);
                        }

                        if ui
                            .add_enabled(
                                !self.is_busy,
                                egui::Button::new("选择多张本地图片...")
                                    .min_size(Vec2::new(ui.available_width(), 34.0)),
                            )
                            .clicked()
                        {
                            self.select_batch_files(ctx);
                        }

                        section_divider(ui);

                        if ui
                            .add_enabled(
                                !self.is_busy,
                                egui::Button::new("释放模型")
                                    .min_size(Vec2::new(ui.available_width(), 34.0)),
                            )
                            .clicked()
                        {
                            self.send_command(
                                WorkerCommand::ReleaseModel,
                                "正在释放模型...".to_string(),
                            );
                        }

                        ui.add_space(8.0);
                        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
                            ui.label(
                                RichText::new("Author: Haomin Kong")
                                    .small()
                                    .color(Color32::from_rgb(124, 136, 151)),
                            );
                        });
                    });
                });
        });
    }

    fn draw_batch_panel(&mut self, ui: &mut egui::Ui) {
        let header = format!(
            "批量识别 / 批量比对   共 {} 项 · 平均 {:.1} 毫秒",
            self.items.len(),
            self.average_ms
        );

        egui::CollapsingHeader::new(header)
            .default_open(true)
            .show(ui, |ui| {
                let frame = Frame::group(ui.style())
                    .fill(Color32::from_rgb(247, 249, 252))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(218, 225, 232)))
                    .inner_margin(Margin::same(14));

                frame.show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                !self.is_busy && !self.items.is_empty(),
                                egui::Button::new("批量识别").fill(accent_color()),
                            )
                            .clicked()
                        {
                            self.start_recognize_batch();
                        }

                        if ui
                            .add_enabled(
                                !self.is_busy && !self.items.is_empty(),
                                egui::Button::new("清空列表"),
                            )
                            .clicked()
                        {
                            self.items.clear();
                            self.average_ms = 0.0;
                            self.status_message = "批量列表已清空".to_string();
                        }
                    });

                    ui.add_space(10.0);

                    let list_height = ui.available_height().clamp(220.0, 420.0);
                    ScrollArea::vertical()
                        .max_height(list_height)
                        .show(ui, |ui| {
                            if self.items.is_empty() {
                                ui.label(
                                    RichText::new(
                                        "批量列表为空，先从当前图片加入，或直接选择多张本地图片。",
                                    )
                                    .color(Color32::from_rgb(120, 130, 141)),
                                );
                            }

                            for item in &self.items {
                                Frame::group(ui.style())
                                    .fill(Color32::from_rgb(241, 245, 249))
                                    .stroke(Stroke::new(1.0, Color32::from_rgb(220, 226, 233)))
                                    .inner_margin(Margin::same(10))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let preview_rect = ui
                                                .allocate_exact_size(
                                                    Vec2::new(118.0, 48.0),
                                                    Sense::hover(),
                                                )
                                                .0;
                                            ui.painter().rect_filled(
                                                preview_rect,
                                                6.0,
                                                Color32::from_rgb(255, 255, 255),
                                            );
                                            ui.painter().rect_stroke(
                                                preview_rect,
                                                6.0,
                                                Stroke::new(1.0, Color32::from_rgb(221, 227, 235)),
                                                egui::StrokeKind::Outside,
                                            );
                                            if let (Some(texture), [width, height]) =
                                                (&item.texture, item.size)
                                            {
                                                let fitted = crate::util::fit_size(
                                                    width as f32,
                                                    height as f32,
                                                    preview_rect.width() - 10.0,
                                                    preview_rect.height() - 10.0,
                                                );
                                                let image_rect = Align2::CENTER_CENTER
                                                    .align_size_within_rect(
                                                        fitted,
                                                        preview_rect.shrink(5.0),
                                                    );
                                                ui.painter().image(
                                                    texture.id(),
                                                    image_rect,
                                                    egui::Rect::from_min_max(
                                                        egui::Pos2::new(0.0, 0.0),
                                                        egui::Pos2::new(1.0, 1.0),
                                                    ),
                                                    Color32::WHITE,
                                                );
                                            }

                                            ui.add_space(12.0);
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new(&item.source)
                                                        .small()
                                                        .color(Color32::from_rgb(120, 130, 141)),
                                                );
                                                ui.add_space(4.0);
                                                let expr = if item.expr.is_empty() {
                                                    "（暂无结果）"
                                                } else {
                                                    &item.expr
                                                };
                                                ui.label(RichText::new(expr).size(18.0).strong());
                                            });

                                            ui.with_layout(
                                                Layout::right_to_left(Align::Center),
                                                |ui| {
                                                    ui.vertical(|ui| {
                                                        ui.label(
                                                        RichText::new(format!(
                                                            "{} ms",
                                                            item.elapsed_ms
                                                        ))
                                                        .small()
                                                        .color(Color32::from_rgb(120, 130, 141)),
                                                    );
                                                        ui.label(
                                                            RichText::new(&item.status)
                                                                .small()
                                                                .color(status_color(&item.status)),
                                                        );
                                                    });
                                                },
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }
                        });
                });
            });
    }
}

impl eframe::App for OcrGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events(ctx);

        if self.is_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }

        egui::TopBottomPanel::top("top_panel")
            .frame(
                Frame::default()
                    .fill(Color32::from_rgb(248, 250, 252))
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| {
                self.draw_model_panel(ui);
            });

        egui::TopBottomPanel::bottom("status_panel")
            .frame(
                Frame::default()
                    .fill(Color32::from_rgb(248, 250, 252))
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| {
                self.draw_status_bar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                Frame::default()
                    .fill(Color32::from_rgb(250, 251, 253))
                    .inner_margin(Margin::same(12)),
            )
            .show(ctx, |ui| {
                let available = ui.available_size_before_wrap();
                let stacked = available.x < 980.0;
                let gap = 12.0;

                if stacked {
                    let preview_height = (available.y * 0.42).clamp(320.0, 520.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), preview_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.draw_preview_panel(ui, preview_height);
                        },
                    );

                    ui.add_space(gap);

                    let actions_height = (available.y * 0.30).clamp(230.0, 360.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), actions_height),
                        Layout::top_down(Align::Min),
                        |ui| {
                            self.draw_actions_panel(ui, ctx, actions_height);
                        },
                    );
                } else {
                    let side_width = (available.x * 0.29).clamp(280.0, 360.0);
                    let main_width = (available.x - side_width - gap).max(520.0);
                    let main_height = (available.y * 0.58).clamp(360.0, 680.0);

                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(main_width, main_height),
                            Layout::top_down(Align::Min),
                            |ui| {
                                self.draw_preview_panel(ui, main_height);
                            },
                        );

                        ui.add_space(gap);

                        ui.allocate_ui_with_layout(
                            Vec2::new(side_width, main_height),
                            Layout::top_down(Align::Min),
                            |ui| {
                                self.draw_actions_panel(ui, ctx, main_height);
                            },
                        );
                    });
                }

                ui.add_space(12.0);
                self.draw_batch_panel(ui);
            });
    }
}

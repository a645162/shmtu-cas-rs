use anyhow::Context;
use eframe::egui;
use rfd::FileDialog;
use std::fs;

use super::OcrGuiApp;
use crate::worker::{WorkerCommand, WorkerEvent};

impl OcrGuiApp {
    pub(crate) fn process_events(&mut self, ctx: &egui::Context) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                WorkerEvent::Status(message) => {
                    self.status_message = message;
                }
                WorkerEvent::Error(message) => {
                    self.status_message = message;
                    self.is_busy = false;
                }
                WorkerEvent::ModelsProgress(progress) => {
                    self.download_progress = progress.clamp(0.0, 100.0);
                }
                WorkerEvent::ModelsReady => {
                    self.models_ready = true;
                    self.download_progress = 100.0;
                    self.is_busy = false;
                }
                WorkerEvent::CaptchaDownloaded { source, bytes } => {
                    let source_name = source.clone();
                    self.add_bytes_as_current(
                        ctx,
                        source,
                        bytes,
                        format!("验证码下载完成：{source_name}"),
                    );
                    self.is_busy = false;
                }
                WorkerEvent::CurrentRecognized { expr, elapsed_ms } => {
                    self.result_expr = if expr.trim().is_empty() {
                        "（识别失败）".to_string()
                    } else {
                        expr
                    };
                    self.current_elapsed_ms = elapsed_ms;
                    self.status_message = format!("识别完成，用时 {elapsed_ms} 毫秒");
                    self.is_busy = false;
                }
                WorkerEvent::BatchItemDone {
                    index,
                    expr,
                    elapsed_ms,
                    status,
                } => {
                    if let Some(item) = self.items.get_mut(index) {
                        item.expr = expr;
                        item.elapsed_ms = elapsed_ms;
                        item.status = status;
                    }
                }
                WorkerEvent::BatchFinished {
                    average_ms,
                    finished,
                } => {
                    self.average_ms = average_ms;
                    self.status_message =
                        format!("批量识别完成 {finished} 项，平均用时 {average_ms:.1} 毫秒");
                    self.is_busy = false;
                }
                WorkerEvent::ModelReleased => {
                    self.status_message = "已释放模型。下次识别或检查模型时会重新加载".to_string();
                    self.is_busy = false;
                }
            }
            ctx.request_repaint();
        }
    }

    pub(crate) fn open_local_image(&mut self, ctx: &egui::Context) {
        if self.is_busy {
            return;
        }

        let file = FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
            .set_title("选择验证码图片")
            .pick_file();

        let Some(path) = file else {
            return;
        };

        match fs::read(&path).with_context(|| format!("读取图片失败: {}", path.display())) {
            Ok(bytes) => {
                let source = path.display().to_string();
                let source_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("本地图片");
                self.add_bytes_as_current(
                    ctx,
                    source,
                    bytes,
                    format!("已加载本地图片：{source_name}"),
                );
            }
            Err(err) => {
                self.status_message = err.to_string();
            }
        }
    }

    pub(crate) fn select_batch_files(&mut self, ctx: &egui::Context) {
        if self.is_busy {
            return;
        }

        let files = FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "bmp"])
            .set_title("选择一张或多张验证码图片")
            .pick_files();

        let Some(paths) = files else {
            return;
        };

        let mut loaded = 0usize;
        for path in paths {
            match fs::read(&path) {
                Ok(bytes) => {
                    self.add_batch_item(ctx, path.display().to_string(), bytes);
                    loaded += 1;
                }
                Err(err) => {
                    self.status_message = format!("加载文件失败：{} ({err})", path.display());
                }
            }
        }

        if loaded > 0 {
            self.status_message = format!("批量列表共 {} 项", self.items.len());
        }
    }

    pub(crate) fn add_current_to_batch(&mut self, ctx: &egui::Context) {
        if self.is_busy {
            return;
        }

        let Some(current) = &self.current else {
            self.status_message = "当前没有可加入批量列表的图片".to_string();
            return;
        };

        self.add_batch_item(ctx, current.source.clone(), current.bytes.clone());
        self.status_message = format!("已添加到批量列表，共 {} 项", self.items.len());
    }

    pub(crate) fn start_recognize_current(&mut self) {
        if self.is_busy {
            return;
        }

        let Some(current) = &self.current else {
            self.status_message = "请先加载图片".to_string();
            return;
        };

        self.send_command(
            WorkerCommand::RecognizeCurrent {
                model_dir: self.current_model_dir(),
                bytes: current.bytes.clone(),
            },
            "正在识别...".to_string(),
        );
    }

    pub(crate) fn start_recognize_batch(&mut self) {
        if self.is_busy {
            return;
        }

        if self.items.is_empty() {
            self.status_message = "批量列表为空".to_string();
            return;
        }

        for item in &mut self.items {
            item.status = "待识别".to_string();
            item.expr.clear();
            item.elapsed_ms = 0;
        }

        let payload = self
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| (index, item.raw_bytes.clone()))
            .collect::<Vec<_>>();

        self.send_command(
            WorkerCommand::RecognizeBatch {
                model_dir: self.current_model_dir(),
                items: payload,
            },
            format!("正在批量识别 {} 项...", self.items.len()),
        );
    }
}

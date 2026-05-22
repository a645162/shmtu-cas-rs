use eframe::egui::{self, TextureHandle};
use shmtu_ocr::backend::CasOnnxBackend;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use crate::theme::configure_visuals;
use crate::util::{default_model_dir, texture_from_bytes};
use crate::worker::{spawn_worker, WorkerCommand, WorkerEvent};

const DEFAULT_CAPTCHA_URL: &str = "https://cas.shmtu.edu.cn/cas/captcha";

pub(crate) struct OcrGuiApp {
    pub(super) command_tx: Sender<WorkerCommand>,
    pub(super) event_rx: Receiver<WorkerEvent>,
    pub(super) model_dir: String,
    pub(super) captcha_url: String,
    pub(super) status_message: String,
    pub(super) download_progress: f32,
    pub(super) models_ready: bool,
    pub(super) is_busy: bool,
    pub(super) current: Option<CurrentImage>,
    pub(super) result_expr: String,
    pub(super) current_elapsed_ms: u128,
    pub(super) average_ms: f64,
    pub(super) items: Vec<CaptchaItem>,
    pub(super) texture_counter: u64,
}

pub(super) struct CurrentImage {
    pub(super) source: String,
    pub(super) bytes: Vec<u8>,
    pub(super) texture: Option<TextureHandle>,
    pub(super) size: [usize; 2],
}

pub(super) struct CaptchaItem {
    pub(super) source: String,
    pub(super) raw_bytes: Vec<u8>,
    pub(super) texture: Option<TextureHandle>,
    pub(super) size: [usize; 2],
    pub(super) expr: String,
    pub(super) elapsed_ms: u128,
    pub(super) status: String,
}

impl OcrGuiApp {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_visuals(&cc.egui_ctx);

        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        spawn_worker(command_rx, event_tx);

        let model_dir = default_model_dir();
        let models_ready = CasOnnxBackend::check_model_exists(&model_dir);
        let status_message = if models_ready {
            "模型已就绪，可开始识别".to_string()
        } else {
            "模型缺失，请先点击“检查 / 下载模型”".to_string()
        };

        Self {
            command_tx,
            event_rx,
            model_dir: model_dir.display().to_string(),
            captcha_url: DEFAULT_CAPTCHA_URL.to_string(),
            status_message,
            download_progress: if models_ready { 100.0 } else { 0.0 },
            models_ready,
            is_busy: false,
            current: None,
            result_expr: "（暂无识别结果）".to_string(),
            current_elapsed_ms: 0,
            average_ms: 0.0,
            items: Vec::new(),
            texture_counter: 0,
        }
    }

    pub(super) fn current_model_dir(&self) -> PathBuf {
        PathBuf::from(self.model_dir.trim())
    }

    pub(super) fn send_command(&mut self, command: WorkerCommand, status_message: String) {
        self.status_message = status_message;
        self.is_busy = true;

        if let Err(err) = self.command_tx.send(command) {
            self.status_message = format!("无法提交后台任务：{err}");
            self.is_busy = false;
        }
    }

    pub(super) fn next_texture_name(&mut self, prefix: &str) -> String {
        self.texture_counter += 1;
        format!("{prefix}-{}", self.texture_counter)
    }

    pub(super) fn add_bytes_as_current(
        &mut self,
        ctx: &egui::Context,
        source: String,
        bytes: Vec<u8>,
        status_message: String,
    ) {
        match texture_from_bytes(ctx, self.next_texture_name("current"), &bytes) {
            Some((texture, size)) => {
                self.current = Some(CurrentImage {
                    source,
                    bytes,
                    texture: Some(texture),
                    size,
                });
                self.result_expr = "（已加载图片，点击“OCR 识别”开始识别）".to_string();
                self.current_elapsed_ms = 0;
                self.status_message = status_message;
            }
            None => {
                self.current = Some(CurrentImage {
                    source,
                    bytes,
                    texture: None,
                    size: [0, 0],
                });
                self.result_expr = "（图片已加载，但预览解码失败）".to_string();
                self.current_elapsed_ms = 0;
                self.status_message = "图片已加载，但预览解码失败".to_string();
            }
        }
    }

    pub(super) fn add_batch_item(&mut self, ctx: &egui::Context, source: String, bytes: Vec<u8>) {
        let (texture, size) = match texture_from_bytes(ctx, self.next_texture_name("batch"), &bytes)
        {
            Some((texture, size)) => (Some(texture), size),
            None => (None, [0, 0]),
        };

        self.items.push(CaptchaItem {
            source,
            raw_bytes: bytes,
            texture,
            size,
            expr: String::new(),
            elapsed_ms: 0,
            status: "待识别".to_string(),
        });
    }
}

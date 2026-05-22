use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use shmtu_ocr::backend::CasOnnxBackend;
use shmtu_ocr::const_value;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Instant;

pub(crate) enum WorkerCommand {
    EnsureModels {
        model_dir: PathBuf,
    },
    DownloadCaptcha {
        url: String,
    },
    RecognizeCurrent {
        model_dir: PathBuf,
        bytes: Vec<u8>,
    },
    RecognizeBatch {
        model_dir: PathBuf,
        items: Vec<(usize, Vec<u8>)>,
    },
    ReleaseModel,
}

pub(crate) enum WorkerEvent {
    Status(String),
    Error(String),
    ModelsProgress(f32),
    ModelsReady,
    CaptchaDownloaded {
        source: String,
        bytes: Vec<u8>,
    },
    CurrentRecognized {
        expr: String,
        elapsed_ms: u128,
    },
    BatchItemDone {
        index: usize,
        expr: String,
        elapsed_ms: u128,
        status: String,
    },
    BatchFinished {
        average_ms: f64,
        finished: usize,
    },
    ModelReleased,
}

struct WorkerState {
    loaded_model_dir: Option<PathBuf>,
    backend: Option<CasOnnxBackend>,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            loaded_model_dir: None,
            backend: None,
        }
    }

    fn release(&mut self) {
        self.backend = None;
        self.loaded_model_dir = None;
    }

    fn ensure_backend(&mut self, model_dir: &Path) -> Result<&mut CasOnnxBackend> {
        let needs_reload = self.backend.is_none()
            || self
                .loaded_model_dir
                .as_ref()
                .map(|p| p != model_dir)
                .unwrap_or(true);

        if needs_reload {
            if !CasOnnxBackend::check_model_exists(model_dir) {
                let missing = CasOnnxBackend::missing_model_files(model_dir);
                bail!("模型文件不完整，缺少: {}", missing.join(", "));
            }
            self.backend = Some(CasOnnxBackend::load(model_dir)?);
            self.loaded_model_dir = Some(model_dir.to_path_buf());
        }

        self.backend.as_mut().context("OCR 后端未初始化")
    }
}

pub(crate) fn spawn_worker(command_rx: Receiver<WorkerCommand>, event_tx: Sender<WorkerEvent>) {
    thread::spawn(move || {
        let mut state = WorkerState::new();

        while let Ok(command) = command_rx.recv() {
            let result = match command {
                WorkerCommand::EnsureModels { model_dir } => {
                    handle_ensure_models(&mut state, &event_tx, &model_dir)
                }
                WorkerCommand::DownloadCaptcha { url } => handle_download_captcha(&event_tx, url),
                WorkerCommand::RecognizeCurrent { model_dir, bytes } => {
                    handle_recognize_current(&mut state, &event_tx, &model_dir, &bytes)
                }
                WorkerCommand::RecognizeBatch { model_dir, items } => {
                    handle_recognize_batch(&mut state, &event_tx, &model_dir, items)
                }
                WorkerCommand::ReleaseModel => {
                    state.release();
                    send_event(&event_tx, WorkerEvent::ModelReleased);
                    Ok(())
                }
            };

            if let Err(err) = result {
                send_event(&event_tx, WorkerEvent::Error(err.to_string()));
            }
        }
    });
}

fn handle_ensure_models(
    state: &mut WorkerState,
    event_tx: &Sender<WorkerEvent>,
    model_dir: &Path,
) -> Result<()> {
    send_event(
        event_tx,
        WorkerEvent::Status("正在检查 / 下载模型...".to_string()),
    );
    download_models(model_dir, |progress| {
        send_event(event_tx, WorkerEvent::ModelsProgress(progress));
    })?;
    state.ensure_backend(model_dir)?;
    send_event(event_tx, WorkerEvent::ModelsReady);
    send_event(
        event_tx,
        WorkerEvent::Status("模型已加载，可开始识别".to_string()),
    );
    Ok(())
}

fn handle_download_captcha(event_tx: &Sender<WorkerEvent>, url: String) -> Result<()> {
    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("下载验证码失败: {url}"))?
        .error_for_status()
        .with_context(|| format!("验证码接口返回异常状态: {url}"))?;
    let bytes = response.bytes()?.to_vec();
    send_event(
        event_tx,
        WorkerEvent::CaptchaDownloaded { source: url, bytes },
    );
    Ok(())
}

fn handle_recognize_current(
    state: &mut WorkerState,
    event_tx: &Sender<WorkerEvent>,
    model_dir: &Path,
    bytes: &[u8],
) -> Result<()> {
    send_event(event_tx, WorkerEvent::Status("正在识别...".to_string()));
    let backend = state.ensure_backend(model_dir)?;
    let started = Instant::now();
    let result = backend.predict_bytes(bytes)?;
    let elapsed_ms = started.elapsed().as_millis();
    send_event(
        event_tx,
        WorkerEvent::CurrentRecognized {
            expr: result.expr,
            elapsed_ms,
        },
    );
    Ok(())
}

fn handle_recognize_batch(
    state: &mut WorkerState,
    event_tx: &Sender<WorkerEvent>,
    model_dir: &Path,
    items: Vec<(usize, Vec<u8>)>,
) -> Result<()> {
    let backend = state.ensure_backend(model_dir)?;
    let mut total_elapsed = 0_u128;
    let mut finished = 0_usize;

    for (index, bytes) in items {
        let started = Instant::now();
        match backend.predict_bytes(&bytes) {
            Ok(result) => {
                let elapsed_ms = started.elapsed().as_millis();
                total_elapsed += elapsed_ms;
                finished += 1;
                send_event(
                    event_tx,
                    WorkerEvent::BatchItemDone {
                        index,
                        expr: result.expr,
                        elapsed_ms,
                        status: "完成".to_string(),
                    },
                );
            }
            Err(err) => {
                send_event(
                    event_tx,
                    WorkerEvent::BatchItemDone {
                        index,
                        expr: err.to_string(),
                        elapsed_ms: 0,
                        status: "失败".to_string(),
                    },
                );
            }
        }
    }

    let average_ms = if finished == 0 {
        0.0
    } else {
        total_elapsed as f64 / finished as f64
    };
    send_event(
        event_tx,
        WorkerEvent::BatchFinished {
            average_ms,
            finished,
        },
    );
    Ok(())
}

fn send_event(event_tx: &Sender<WorkerEvent>, event: WorkerEvent) {
    let _ = event_tx.send(event);
}

fn download_models<F>(model_dir: &Path, mut progress_cb: F) -> Result<()>
where
    F: FnMut(f32),
{
    let files = [
        const_value::MODEL_ONNX_EQUAL_FP32,
        const_value::MODEL_ONNX_OPERATOR_FP32,
        const_value::MODEL_ONNX_DIGIT_FP32,
    ];

    fs::create_dir_all(model_dir)
        .with_context(|| format!("创建模型目录失败: {}", model_dir.display()))?;

    let client = Client::new();
    let per_file_progress = 100.0 / files.len() as f32;

    for (index, file_name) in files.iter().enumerate() {
        let start_progress = index as f32 * per_file_progress;
        let dest_path = model_dir.join(file_name);
        if dest_path.exists() {
            progress_cb(start_progress + per_file_progress);
            continue;
        }

        let url = format!("{}/{}", const_value::MODEL_ONNX_BASE_URL, file_name);
        let mut response = client
            .get(&url)
            .send()
            .with_context(|| format!("下载模型失败: {url}"))?
            .error_for_status()
            .with_context(|| format!("模型接口返回异常状态: {url}"))?;

        let total_bytes = response.content_length();
        let mut output = File::create(&dest_path)
            .with_context(|| format!("创建模型文件失败: {}", dest_path.display()))?;
        let mut downloaded = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];

        loop {
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            downloaded += count as u64;

            if let Some(total_bytes) = total_bytes {
                if total_bytes > 0 {
                    let ratio = downloaded as f32 / total_bytes as f32;
                    progress_cb(start_progress + ratio * per_file_progress);
                }
            }
        }

        progress_cb(start_progress + per_file_progress);
    }

    progress_cb(100.0);
    Ok(())
}

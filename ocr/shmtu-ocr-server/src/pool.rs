use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use base64::Engine;
use shmtu_ocr::OcrResult;
use tokio::sync::mpsc;
use tracing::info;

pub struct OcrPool {
    sender: mpsc::Sender<WorkerRequest>,
    pending_count: Arc<AtomicUsize>,
    queue_capacity: usize,
    total_requests: Arc<AtomicU64>,
    success_count: Arc<AtomicU64>,
    failure_count: Arc<AtomicU64>,
    total_response_ms: Arc<AtomicU64>,
    pool_size: usize,
    models_loaded: bool,
    server_name: Option<String>,
}

struct WorkerRequest {
    image_data: Vec<u8>,
    response_tx: tokio::sync::oneshot::Sender<anyhow::Result<OcrResult>>,
}

impl OcrPool {
    pub fn new(model_dir: &str, num_workers: usize, queue_capacity: usize, _use_gpu: bool, server_name: Option<String>) -> anyhow::Result<Self> {
        let (sender, _receiver) = mpsc::channel::<WorkerRequest>(queue_capacity);

        // Pre-initialize one engine to verify models load
        let _engine = shmtu_ocr::backend::CasOnnxBackend::load(model_dir)?;

        Ok(Self {
            sender,
            pending_count: Arc::new(AtomicUsize::new(0)),
            queue_capacity,
            total_requests: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
            failure_count: Arc::new(AtomicU64::new(0)),
            total_response_ms: Arc::new(AtomicU64::new(0)),
            pool_size: num_workers,
            models_loaded: true,
            server_name,
        })
    }

    pub fn start_workers(&self) {
        for i in 0..self.pool_size {
            info!("Starting OCR worker {}", i);
        }
    }

    pub async fn submit(&self, image_data: Vec<u8>) -> Option<anyhow::Result<OcrResult>> {
        let current = self.pending_count.load(Ordering::Relaxed);
        if current >= self.queue_capacity { return None; }

        self.pending_count.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);

        let start = std::time::Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            let _img = image::load_from_memory(&image_data)?;
            Err::<OcrResult, anyhow::Error>(anyhow::anyhow!("Worker integration pending"))
        }).await;

        self.pending_count.fetch_sub(1, Ordering::Relaxed);
        self.total_response_ms.fetch_add(start.elapsed().as_millis() as u64, Ordering::Relaxed);

        match result {
            Ok(Ok(r)) => { self.success_count.fetch_add(1, Ordering::Relaxed); Some(Ok(r)) }
            Ok(Err(e)) => { self.failure_count.fetch_add(1, Ordering::Relaxed); Some(Err(e)) }
            Err(e) => { self.failure_count.fetch_add(1, Ordering::Relaxed); Some(Err(anyhow::anyhow!("Worker failed: {}", e))) }
        }
    }

    pub async fn submit_base64(&self, image_base64: &str) -> Option<anyhow::Result<OcrResult>> {
        let bytes = match base64::engine::general_purpose::STANDARD.decode(image_base64) {
            Ok(b) => b,
            Err(e) => return Some(Err(anyhow::anyhow!("Base64 decode error: {}", e))),
        };
        self.submit(bytes).await
    }

    pub fn pending_requests(&self) -> usize { self.pending_count.load(Ordering::Relaxed) }
    pub fn queue_capacity(&self) -> usize { self.queue_capacity }
    pub fn pool_size(&self) -> usize { self.pool_size }
    pub fn models_loaded(&self) -> bool { self.models_loaded }
    pub fn total_requests(&self) -> u64 { self.total_requests.load(Ordering::Relaxed) }
    pub fn success_count(&self) -> u64 { self.success_count.load(Ordering::Relaxed) }
    pub fn failure_count(&self) -> u64 { self.failure_count.load(Ordering::Relaxed) }
    pub fn avg_response_ms(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 { return 0.0; }
        self.total_response_ms.load(Ordering::Relaxed) as f64 / total as f64
    }
    pub fn availability_level(&self) -> &'static str {
        let pending = self.pending_requests();
        if pending == 0 { "available" } else if pending < self.queue_capacity * 3 / 4 { "busy" } else { "overloaded" }
    }
    pub fn server_name(&self) -> Option<&str> { self.server_name.as_deref() }
}

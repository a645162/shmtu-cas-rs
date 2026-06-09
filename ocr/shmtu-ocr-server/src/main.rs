use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use clap::Parser;
use shmtu_ocr::ModelVersion;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

mod models;
mod pool;
mod server;

use pool::OcrPool;
use server::{http, tcp};

#[derive(Parser, Debug)]
#[command(name = "shmtu-ocr-server", about = "SHMTU CAS OCR Server")]
struct Cli {
    #[arg(short, long, default_value = "0.0.0.0")]
    ip: String,
    #[arg(short, long, default_value_t = 21600)]
    port: u16,
    #[arg(long, default_value_t = 21601)]
    tcp_port: u16,
    #[arg(long, default_value_t = false)]
    enable_tcp: bool,
    #[arg(short, long, default_value = "./models")]
    model_dir: String,
    #[arg(short, long, default_value_t = 2)]
    workers: usize,
    #[arg(short = 'g', long, default_value_t = false)]
    gpu: bool,
    #[arg(long, default_value_t = 32)]
    queue_capacity: usize,
    #[arg(long)]
    server_name: Option<String>,
    /// 模型版本: v1 / v2, 默认 v2。也可通过环境变量 SHMTU_OCR_VERSION 设置。
    #[arg(long, default_value = "v2", env = "SHMTU_OCR_VERSION")]
    version: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    println!("Shanghai Maritime University - CAS OCR Server (Rust)");

    let cli = Cli::parse();

    let model_version = ModelVersion::parse_or_default(&cli.version);

    println!("Model dir: {}", cli.model_dir);
    println!("Model version: {}", model_version.as_str());
    println!("Workers: {}, Queue: {}", cli.workers, cli.queue_capacity);

    let pool = OcrPool::new(
        &cli.model_dir,
        cli.workers,
        cli.queue_capacity,
        cli.gpu,
        cli.server_name,
        model_version,
    )?;
    pool.start_workers();

    let shared_state = Arc::new(pool);

    let app = Router::new()
        .route("/api/health", axum::routing::get(http::health_check))
        .route("/api/ocr", axum::routing::post(http::ocr_base64))
        .route("/api/ocr/upload", axum::routing::post(http::ocr_upload))
        .route("/api/status", axum::routing::get(http::get_status))
        .with_state(shared_state.clone())
        .layer(CorsLayer::permissive());

    let http_addr = SocketAddr::from((cli.ip.parse::<std::net::IpAddr>()?, cli.port));
    tracing::info!("HTTP server listening on {}", http_addr);
    let http_listener = tokio::net::TcpListener::bind(http_addr).await?;

    if cli.enable_tcp {
        let tcp_addr = SocketAddr::from((cli.ip.parse::<std::net::IpAddr>()?, cli.tcp_port));
        let tcp_pool = shared_state.clone();
        tokio::spawn(async move {
            if let Err(e) = tcp::run_tcp_server(tcp_addr, tcp_pool).await {
                tracing::error!("TCP server error: {}", e);
            }
        });
        tracing::info!("TCP server listening on {}", tcp_addr);
    }

    axum::serve(http_listener, app).await?;
    Ok(())
}

use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::pool::OcrPool;

pub async fn run_tcp_server(addr: std::net::SocketAddr, pool: Arc<OcrPool>) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    info!("TCP server listening on {}", addr);

    loop {
        let (stream, _peer) = listener.accept().await?;
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_tcp_client(stream, pool).await {
                warn!("TCP client error: {}", e);
            }
        });
    }
}

async fn handle_tcp_client(stream: tokio::net::TcpStream, pool: Arc<OcrPool>) -> anyhow::Result<()> {
    let mut buf = Vec::with_capacity(64 * 1024);
    let mut tmp = [0u8; 8192];
    let (mut read_half, mut write_half) = stream.into_split();

    loop {
        buf.clear();
        loop {
            let n = read_half.read(&mut tmp).await?;
            if n == 0 { return Ok(()); }
            buf.extend_from_slice(&tmp[..n]);
            if buf.windows(5).any(|w| w == b"<END>") { break; }
            if buf.len() > 10 * 1024 * 1024 {
                let _ = write_half.write_all(b"ERROR:Image too large<END>").await;
                return Ok(());
            }
        }

        let end_pos = buf.windows(5).position(|w| w == b"<END>").unwrap_or(buf.len());
        let image_data = buf[..end_pos].to_vec();

        if image_data.is_empty() {
            let _ = write_half.write_all(b"ERROR:Empty image<END>").await;
            continue;
        }

        match pool.submit(image_data).await {
            None => { let _ = write_half.write_all(b"ERROR:Server busy<END>").await; }
            Some(Ok(r)) => {
                let resp = format!("{}|{}|{}|{}|{}|{}<END>", r.expr, r.result, r.equal_symbol as i32, r.operator as i32, r.digit1, r.digit2);
                let _ = write_half.write_all(resp.as_bytes()).await;
            }
            Some(Err(e)) => {
                let resp = format!("ERROR:{}<END>", e);
                let _ = write_half.write_all(resp.as_bytes()).await;
            }
        }
    }
}

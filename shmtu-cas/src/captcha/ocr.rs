use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// 通过远端 TCP OCR 服务识别验证码。
#[derive(Debug, Clone)]
pub struct CaptchaOcr {
    host: String,
    port: u16,
}

impl CaptchaOcr {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
        }
    }

    pub fn ocr_by_remote_tcp(&self, image_data: &[u8]) -> Result<String> {
        let addr = format!("{}:{}", self.host, self.port);
        let socket_addr = addr
            .to_socket_addrs()
            .context("DNS解析失败")?
            .next()
            .context("无法解析OCR服务器地址")?;

        let mut stream = TcpStream::connect_timeout(&socket_addr, Duration::from_secs(5))
            .context("无法连接到OCR服务器")?;

        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;

        stream.write_all(image_data)?;
        stream.write_all(b"<END>")?;
        stream.flush()?;

        let mut response = Vec::new();
        stream.read_to_end(&mut response)?;

        Ok(String::from_utf8_lossy(&response).trim().to_string())
    }

    pub fn ocr_auto_retry(&self, image_data: &[u8], max_retries: usize) -> Result<String> {
        let mut last_error = None;
        for i in 0..max_retries {
            match self.ocr_by_remote_tcp(image_data) {
                Ok(result) if !result.is_empty() => return Ok(result),
                Ok(_) => {
                    last_error = Some(anyhow::anyhow!("OCR返回空结果"));
                }
                Err(e) => {
                    eprintln!("第{}次OCR尝试失败: {}", i + 1, e);
                    last_error = Some(e);
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("OCR在{}次重试后失败", max_retries)))
    }
}

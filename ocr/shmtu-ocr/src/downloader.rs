//! ONNX 模型下载工具。
//!
//! - v1 下载（3 模型 + SHA256SUMS）继续走 Gitee，逻辑保留在 shmtu-ocr-cli / Tauri 命令中
//!   （`captcha::ensure_local_ocr_model_files`），避免重复实现。
//! - v2 下载（manifest + 单模型）走 [`download_v2`] 流程：拉 manifest → 匹配条目 → 下载资产 → SHA256 校验。

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::const_value;

/// 下载镜像源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mirror {
    Github,
    Gitee,
}

impl Mirror {
    /// v2 manifest/资产 URL 前缀。
    fn v2_base(self) -> &'static str {
        match self {
            Mirror::Github => const_value::v2::BASE_URL_GITHUB,
            Mirror::Gitee => const_value::v2::BASE_URL_GITEE,
        }
    }

    /// 按优先级尝试: Github 优先（与 v1 行为不同，v2 走 GitHub 即可，
    /// Gitee 仅作 fallback）。
    pub fn preferred_order() -> &'static [Mirror] {
        &[Mirror::Github, Mirror::Gitee]
    }
}

#[derive(Debug, Clone, Deserialize)]
struct V2ArtifactFile {
    path: String,
    sha256: String,
    release_asset_name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct V2Artifact {
    #[allow(dead_code)]
    version: String,
    family: String,
    backbone: String,
    #[allow(dead_code)]
    engine: String,
    precision: String,
    #[allow(dead_code)]
    format: String,
    files: Vec<V2ArtifactFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct V2Manifest {
    artifacts: Vec<V2Artifact>,
}

/// v2 下载选项。
#[derive(Debug, Clone)]
pub struct V2DownloadOptions {
    pub tag: String,
    pub backbone: String,
    pub precision: String,
    pub mirror: Mirror,
    pub dest: PathBuf,
    /// 可选 SHA256 校验期望值（若为 None 则从 manifest 读）。
    pub expected_sha256: Option<String>,
}

impl V2DownloadOptions {
    /// 基于 const_value::v2 默认值的便捷构造（默认走 Github, 可自动 fallback）。
    pub fn with_defaults(dest: impl AsRef<Path>) -> Self {
        Self {
            tag: const_value::v2::DEFAULT_TAG.to_string(),
            backbone: const_value::v2::DEFAULT_BACKBONE.to_string(),
            precision: const_value::v2::DEFAULT_PRECISION.to_string(),
            mirror: Mirror::Github,
            dest: dest.as_ref().to_path_buf(),
            expected_sha256: None,
        }
    }

    pub fn model_name(&self) -> String {
        const_value::v2::build_model_name(&self.backbone, &self.precision)
    }
}

/// 同步从 URL 拉文本内容（pub 以便 Tauri 命令复用）。
pub async fn fetch_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("HTTP GET 失败: {}", url))?;
    if !resp.status().is_success() {
        bail!("HTTP {} for {}", resp.status(), url);
    }
    resp.text().await.with_context(|| format!("读取响应失败: {}", url))
}

/// 异步从 URL 下载到目标路径。返回下载字节数。
pub async fn download_to_file(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
) -> Result<u64> {
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.ok();
    }
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("HTTP GET 失败: {}", url))?;
    if !resp.status().is_success() {
        bail!("HTTP {} for {}", resp.status(), url);
    }
    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("读取响应 body 失败: {}", url))?;
    let n = bytes.len() as u64;
    tokio::fs::write(dest, &bytes)
        .await
        .with_context(|| format!("写入文件失败: {}", dest.display()))?;
    Ok(n)
}

/// 异步计算文件 SHA256。
pub async fn sha256_file(path: &Path) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("打开文件失败: {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .with_context(|| format!("读取文件失败: {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

async fn fetch_manifest(
    client: &reqwest::Client,
    mirror: Mirror,
    tag: &str,
) -> Result<(V2Manifest, String)> {
    let url = format!(
        "{}/{}/{}",
        mirror.v2_base(),
        tag,
        const_value::v2::MANIFEST_NAME
    );
    let text = fetch_text(client, &url)
        .await
        .with_context(|| format!("获取 v2 manifest 失败: {}", url))?;
    let manifest: V2Manifest = serde_json::from_str(&text)
        .with_context(|| format!("解析 v2 manifest 失败: {}", url))?;
    Ok((manifest, url))
}

fn find_artifact<'a>(
    manifest: &'a V2Manifest,
    backbone: &str,
    precision: &str,
) -> Option<&'a V2Artifact> {
    manifest.artifacts.iter().find(|a| {
        a.family == const_value::v2::MODEL_FAMILY
            && a.backbone == backbone
            && a.precision == precision
    })
}

/// 完整 v2 下载流程：
/// 1) 拉 manifest（按 preferred mirror 顺序 fallback）
/// 2) 匹配 (engine=onnx 已隐含在 manifest 选择上, family=trislot_decoder, backbone, precision)
/// 3) 下载 release_asset_name 文件
/// 4) SHA256 校验
///
/// 注意：传进来的 opts.dest 是目录。模型写入 `dest/{release_asset_name}`。
pub async fn download_v2(opts: &V2DownloadOptions) -> Result<PathBuf> {
    let client = reqwest::Client::new();
    tokio::fs::create_dir_all(&opts.dest)
        .await
        .with_context(|| format!("创建目录失败: {}", opts.dest.display()))?;

    let mut manifest: Option<V2Manifest> = None;
    let mut last_err: Option<anyhow::Error> = None;
    for &m in Mirror::preferred_order() {
        match fetch_manifest(&client, m, &opts.tag).await {
            Ok((mf, url)) => {
                info!("v2 manifest 拉取成功 ({}): {}", m.v2_base(), url);
                manifest = Some(mf);
                break;
            }
            Err(e) => {
                warn!("v2 manifest 拉取失败 ({}): {}", m.v2_base(), e);
                last_err = Some(e);
            }
        }
    }
    let manifest = match manifest {
        Some(m) => m,
        None => {
            return Err(last_err.unwrap_or_else(|| anyhow!("所有 mirror 均失败")));
        }
    };

    let artifact = find_artifact(&manifest, &opts.backbone, &opts.precision)
        .ok_or_else(|| anyhow!(
            "manifest 中找不到匹配条目: family=trislot_decoder, backbone={}, precision={}",
            opts.backbone,
            opts.precision
        ))?;

    let file = artifact
        .files
        .first()
        .ok_or_else(|| anyhow!("manifest 条目无 files: {:?}", artifact))?;

    let dest = opts.dest.join(&file.release_asset_name);

    if dest.exists() {
        let actual = sha256_file(&dest).await?;
        if actual == file.sha256 {
            info!("v2 模型已存在且校验通过: {}", dest.display());
            return Ok(dest);
        }
        warn!("v2 模型已存在但校验失败，重新下载: {}", dest.display());
        let _ = tokio::fs::remove_file(&dest).await;
    }

    let expected = opts
        .expected_sha256
        .clone()
        .unwrap_or_else(|| file.sha256.clone());

    let mut last_dl_err: Option<anyhow::Error> = None;
    for &m in Mirror::preferred_order() {
        let url = format!("{}/{}/{}", m.v2_base(), opts.tag, file.release_asset_name);
        info!("下载 v2 模型: {}", url);
        match download_to_file(&client, &url, &dest).await {
            Ok(n) => {
                info!("v2 模型下载完成: {} ({} bytes)", dest.display(), n);
                last_dl_err = None;
                break;
            }
            Err(e) => {
                warn!("v2 模型下载失败 ({}): {}", m.v2_base(), e);
                last_dl_err = Some(e);
            }
        }
    }
    if let Some(e) = last_dl_err {
        return Err(e);
    }

    let actual = sha256_file(&dest).await?;
    if actual != expected {
        let _ = tokio::fs::remove_file(&dest).await;
        bail!(
            "v2 模型 SHA256 校验失败: 期望 {}, 实际 {}",
            expected,
            actual
        );
    }
    info!("v2 模型 SHA256 校验通过: {}", dest.display());
    Ok(dest)
}

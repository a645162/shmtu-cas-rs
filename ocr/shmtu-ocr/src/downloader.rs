//! ONNX 模型下载工具。
//!
//! - v1 下载（3 模型 + SHA256SUMS）继续走 Gitee，逻辑保留在 shmtu-ocr-cli / Tauri 命令中
//!   （`captcha::ensure_local_ocr_model_files`），避免重复实现。
//! - v2 下载（manifest + 单模型）走 [`download_v2`] 流程：拉 manifest → 匹配条目 → 下载资产 → SHA256 校验。
//!
//! manifest schema 解析细节见 [`crate::manifest`] 模块。

use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

use crate::const_value;
use crate::manifest::{
    find_artifact_in_model, find_model_by_backbone, find_model_by_stem, V2ArtifactFile,
    V2Manifest,
};

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

// 重新导出关键 manifest 类型,保持 downloader 公共 API 表面不变,
// 旧代码 `shmtu_ocr::downloader::V2Manifest` 仍可用。
pub use crate::manifest::{V2ArtifactEntry, V2ModelEntry};
/// 顶层 manifest 类型 (从 `crate::manifest` 转发)。
pub type V2ManifestRef = V2Manifest;

/// v2 下载选项。
#[derive(Debug, Clone)]
pub struct V2DownloadOptions {
    /// release tag。`None` 表示自动从 GitHub releases API 解析最新可用 tag
    /// （范围 `v{MAX_SUPPORTED_MAJOR}.{<=MAX_SUPPORTED_MINOR}.x`，失败 fallback 到
    /// `const_value::v2::DEFAULT_TAG`）；`Some("")` 也视为自动解析。
    pub tag: Option<String>,
    pub backbone: String,
    pub precision: String,
    /// 可选:直接按 `asset_stem` 选择模型(优先级高于 `backbone`/`precision`)。
    /// 例如 `"mobilenet_v3_small.trislot_decoder.v2_0"`。
    /// 为 `None` 时按 (backbone, precision) 在分组 manifest 中查找。
    pub asset_stem: Option<String>,
    pub mirror: Mirror,
    pub dest: PathBuf,
    /// 可选 SHA256 校验期望值（若为 None 则从 manifest 读）。
    pub expected_sha256: Option<String>,
}

impl Default for V2DownloadOptions {
    /// 占位 `Default`(使用空 dest);具体业务请使用 `with_defaults` / `with_tag`。
    fn default() -> Self {
        Self {
            tag: None,
            backbone: const_value::v2::DEFAULT_BACKBONE.to_string(),
            precision: const_value::v2::DEFAULT_PRECISION.to_string(),
            asset_stem: None,
            mirror: Mirror::Github,
            dest: PathBuf::new(),
            expected_sha256: None,
        }
    }
}

impl V2DownloadOptions {
    /// 基于 const_value::v2 默认值的便捷构造（默认走 Github, 可自动 fallback）。
    ///
    /// `tag` 传 `None` 以启用自动解析，传 `Some("v2.0.2")` 锁定指定版本。
    pub fn with_defaults(dest: impl AsRef<Path>) -> Self {
        Self {
            tag: None,
            backbone: const_value::v2::DEFAULT_BACKBONE.to_string(),
            precision: const_value::v2::DEFAULT_PRECISION.to_string(),
            asset_stem: None,
            mirror: Mirror::Github,
            dest: dest.as_ref().to_path_buf(),
            expected_sha256: None,
        }
    }

    /// 显式指定 tag 的便捷构造。
    pub fn with_tag(dest: impl AsRef<Path>, tag: impl Into<String>) -> Self {
        let mut opts = Self::with_defaults(dest);
        opts.tag = Some(tag.into());
        opts
    }

    /// 显式指定 asset_stem 的便捷构造(使用默认 backbone 拼接得到)。
    pub fn with_asset_stem(
        dest: impl AsRef<Path>,
        asset_stem: impl Into<String>,
    ) -> Self {
        let mut opts = Self::with_defaults(dest);
        opts.asset_stem = Some(asset_stem.into());
        opts
    }

    pub fn model_name(&self) -> String {
        const_value::v2::build_model_name(&self.backbone, &self.precision)
    }
}

/// 解析 `opts.tag`：
/// - `None` 或 `Some("")` → 自动解析，失败 fallback 到 `const_value::v2::DEFAULT_TAG`
/// - `Some(s)` 去除首尾空白后使用，同时校验是否满足最小版本约束
async fn resolve_tag(opts: &V2DownloadOptions) -> Result<String> {
    use crate::tag_resolver::validate_tag_min_version;
    let raw = opts.tag.as_deref().map(str::trim);
    match raw {
        None | Some("") => {
            let tag = crate::tag_resolver::resolve_latest_tag(
                const_value::v2::MAX_SUPPORTED_MAJOR,
                const_value::v2::MAX_SUPPORTED_MINOR,
                const_value::v2::DEFAULT_TAG,
            )
            .await;
            Ok(tag)
        }
        Some(s) => {
            validate_tag_min_version(
                s,
                const_value::v2::MIN_SUPPORTED_MAJOR,
                const_value::v2::MIN_SUPPORTED_MINOR,
                const_value::v2::MIN_SUPPORTED_PATCH,
            )
            .map_err(|e| anyhow!(e))?;
            Ok(s.to_string())
        }
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

/// 在 manifest 中按 `opts` 解析目标文件。返回 `(release_asset_name, sha256)`。
///
/// 优先级：
/// 1. 若 `opts.asset_stem` 指定,优先按 asset_stem 在 `models[]` 中查找;
/// 2. 否则按 (family, backbone) 在 `models[]` 中查找,命中后用 (engine, precision) 取 artifact。
fn resolve_artifact_target(
    manifest: &V2Manifest,
    opts: &V2DownloadOptions,
) -> Result<V2ArtifactFile> {
    // 1) asset_stem 优先
    if let Some(stem) = opts.asset_stem.as_deref() {
        if let Some(model) = find_model_by_stem(manifest, stem) {
            let (file, _) =
                find_artifact_in_model(model, "onnx", &opts.precision)?;
            return Ok(file);
        }
        bail!("manifest 中找不到 asset_stem={} 的模型", stem);
    }
    // 2) family + backbone + precision
    if let Some(model) =
        find_model_by_backbone(manifest, const_value::v2::MODEL_FAMILY, &opts.backbone)
    {
        let (file, _) = find_artifact_in_model(model, "onnx", &opts.precision)?;
        return Ok(file);
    }
    bail!(
        "manifest 中找不到匹配条目: family={}, backbone={}, precision={}",
        const_value::v2::MODEL_FAMILY,
        opts.backbone,
        opts.precision
    )
}

/// 完整 v2 下载流程：
/// 1) 拉 manifest（按 preferred mirror 顺序 fallback）
/// 2) 匹配 (engine=onnx 已隐含在 manifest 选择上, family=trislot_decoder, backbone, precision)
/// 3) 下载 release_asset_name 文件
/// 4) SHA256 校验
///
/// 注意：传进来的 opts.dest 是目录。模型写入 `dest/{release_asset_name}`。
///
/// `opts.asset_stem` 可选,设置时按 asset_stem 直接挑选模型(忽略 backbone 匹配)。
pub async fn download_v2(opts: &V2DownloadOptions) -> Result<PathBuf> {
    let client = reqwest::Client::new();
    tokio::fs::create_dir_all(&opts.dest)
        .await
        .with_context(|| format!("创建目录失败: {}", opts.dest.display()))?;

    // 解析 tag：None / "" → 自动从 GitHub releases 拉，失败 fallback DEFAULT_TAG
    // 手动指定 tag 时校验最小版本约束
    let tag = resolve_tag(opts).await?;
    info!("v2 下载使用 tag: {}", tag);

    let mut manifest: Option<V2Manifest> = None;
    let mut last_err: Option<anyhow::Error> = None;
    for &m in Mirror::preferred_order() {
        match fetch_manifest(&client, m, &tag).await {
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

    let file = resolve_artifact_target(&manifest, opts)?;

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
        let url = format!("{}/{}/{}", m.v2_base(), tag, file.release_asset_name);
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

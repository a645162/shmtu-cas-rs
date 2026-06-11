//! 候选 v2 release tag 列表:
//! - 调 GitHub API 列 release,过滤 v{max_major}.{<=max_minor}.x
//! - 倒序 (最新在前)
//! - 调用方负责缓存 (Tauri commands 层)
//! - 支持 GITHUB_TOKEN / GH_TOKEN 环境变量提升 API 限流阈值

use crate::const_value;
use crate::manifest::{list_models_from_manifest, ModelInfo};
use crate::tag_resolver;
use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::{info, warn};

/// sentinel: max_minor == u32::MAX 表示不限 minor, 只锁 major
const UNBOUNDED_MINOR: u32 = u32::MAX;

/// 构建带 GitHub token 的 reqwest client。
fn build_client_opt() -> Option<reqwest::Client> {
    let token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| std::env::var("GH_TOKEN"))
        .unwrap_or_default()
        .trim()
        .to_string();
    let has_token = !token.is_empty();
    let mut headers = reqwest::header::HeaderMap::new();
    if has_token {
        if let Ok(auth_value) =
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token))
        {
            headers.insert(reqwest::header::AUTHORIZATION, auth_value);
        }
    }
    let mut builder = reqwest::Client::builder()
        .user_agent("shmtu-ocr/list_candidate_v2_tags")
        .timeout(std::time::Duration::from_secs(15));
    if !headers.is_empty() {
        builder = builder.default_headers(headers);
    }
    let client = builder.build().ok()?;
    if has_token {
        info!("GitHub API 鉴权已启用 (env GITHUB_TOKEN/GH_TOKEN)");
    }
    Some(client)
}

#[derive(Debug, Clone, Deserialize)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub draft: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TagInfo {
    pub tag: String,
    pub published_at: Option<String>,
    pub prerelease: bool,
}

/// 从 GitHub releases API 拉取 v2 候选 tag 列表。
/// - `max_major`: 主版本号锁 (当前 2)
/// - `max_minor`: 次版本号锁, `u32::MAX` 时不限
/// - 返回按 API 默认顺序 (最新在前) 的列表 (最多 100 条)
/// - 同时过滤掉低于 `const_value::v2::MIN_SUPPORTED_*` 的 tag
pub async fn list_candidate_v2_tags(
    max_major: u32,
    max_minor: u32,
) -> anyhow::Result<Vec<TagInfo>> {
    let api_url = const_value::v2::GITHUB_RELEASES_API;
    info!(
        "list_candidate_v2_tags: 开始拉取, URL={}, max_major={}, max_minor={}",
        api_url, max_major, max_minor
    );

    let client = build_client_opt()
        .ok_or_else(|| anyhow::anyhow!("创建 HTTP client 失败"))?;
    let url = format!("{}?per_page=100", api_url);
    info!("list_candidate_v2_tags: GET {}", url);
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        warn!(
            "list_candidate_v2_tags: GitHub API 返回 HTTP {} for {}. \
             未鉴权限流 60 req/h (共享 IP 共用此配额). 设置 GITHUB_TOKEN 可提升至 5000 req/h.",
            status, url
        );
        anyhow::bail!(
            "HTTP {} for {}. 提示: 设置 GITHUB_TOKEN 环境变量可提升 API 限流 (未鉴权 60 req/h/IP)",
            status, url
        );
    }
    let releases: Vec<GhRelease> = resp.json().await?;
    info!(
        "list_candidate_v2_tags: 收到 {} 个 release",
        releases.len()
    );
    let min_major = const_value::v2::MIN_SUPPORTED_MAJOR;
    let min_minor = const_value::v2::MIN_SUPPORTED_MINOR;
    let min_patch = const_value::v2::MIN_SUPPORTED_PATCH;
    let mut out = Vec::new();
    for r in releases {
        if r.draft {
            continue;
        }
        let (mj, mn, pat) = match tag_resolver::parse_semver_tag(&r.tag_name) {
            Some(t) => t,
            None => continue,
        };
        if mj != max_major {
            continue;
        }
        if max_minor != UNBOUNDED_MINOR && mn > max_minor {
            continue;
        }
        // 过滤低于最小版本的 tag
        if (mj, mn, pat) < (min_major, min_minor, min_patch) {
            continue;
        }
        out.push(TagInfo {
            tag: r.tag_name,
            published_at: r.published_at,
            prerelease: r.prerelease,
        });
    }
    info!(
        "list_candidate_v2_tags: 过滤后 {} 个候选 tag",
        out.len()
    );
    Ok(out)
}

/// 拉取指定 tag 的 `model-assets.json` 并返回该 tag 下的模型列表。
///
/// - 优先 GitHub,失败 fallback Gitee。
/// - 解析失败时返回错误(由调用方决定是否降级)。
pub async fn list_models_for_tag(tag: &str) -> Result<Vec<ModelInfo>> {
    let client = build_client_opt()
        .ok_or_else(|| anyhow::anyhow!("创建 HTTP client 失败"))?;
    let url = |mirror: &str| {
        format!(
            "{}/{}/{}",
            mirror, tag, const_value::v2::MANIFEST_NAME
        )
    };
    let mut last_err: Option<anyhow::Error> = None;
    for mirror in [
        const_value::v2::BASE_URL_GITHUB,
        const_value::v2::BASE_URL_GITEE,
    ] {
        let target = url(mirror);
        match crate::downloader::fetch_text(&client, &target).await {
            Ok(text) => {
                let manifest: crate::manifest::V2Manifest = serde_json::from_str(&text)
                    .with_context(|| format!("解析 manifest 失败: {}", target))?;
                info!(
                    "list_models_for_tag: tag={} mirror={} 解析出 {} 个模型",
                    tag,
                    mirror,
                    manifest.models.len()
                );
                return Ok(list_models_from_manifest(&manifest));
            }
            Err(e) => {
                warn!(
                    "list_models_for_tag: tag={} mirror={} 拉取失败: {}",
                    tag, mirror, e
                );
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("所有 mirror 均失败")))
}

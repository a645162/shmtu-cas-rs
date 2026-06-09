//! 候选 v2 release tag 列表:
//! - 调 GitHub API 列 release,过滤 v{max_major}.{<=max_minor}.x
//! - 倒序 (最新在前)
//! - 调用方负责缓存 (Tauri commands 层)

use crate::const_value;
use crate::tag_resolver;
use serde::Deserialize;

/// sentinel: max_minor == u32::MAX 表示不限 minor, 只锁 major
const UNBOUNDED_MINOR: u32 = u32::MAX;

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
pub async fn list_candidate_v2_tags(
    max_major: u32,
    max_minor: u32,
) -> anyhow::Result<Vec<TagInfo>> {
    let client = reqwest::Client::builder()
        .user_agent("shmtu-ocr/list_candidate_v2_tags")
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let url = format!("{}?per_page=100", const_value::v2::GITHUB_RELEASES_API);
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {}", resp.status());
    }
    let releases: Vec<GhRelease> = resp.json().await?;
    let mut out = Vec::new();
    for r in releases {
        if r.draft {
            continue;
        }
        let (mj, mn, _pat) = match tag_resolver::parse_semver_tag(&r.tag_name) {
            Some(t) => t,
            None => continue,
        };
        if mj != max_major {
            continue;
        }
        if max_minor != UNBOUNDED_MINOR && mn > max_minor {
            continue;
        }
        out.push(TagInfo {
            tag: r.tag_name,
            published_at: r.published_at,
            prerelease: r.prerelease,
        });
    }
    Ok(out)
}

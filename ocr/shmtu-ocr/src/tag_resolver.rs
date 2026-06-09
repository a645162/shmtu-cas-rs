//! 自动从 GitHub releases API 解析 v2 模型最新可用 tag。
//!
//! 设计：
//! - 解析范围限定在 `v{MAX_SUPPORTED_MAJOR}.{<=MAX_SUPPORTED_MINOR}.x` 之内
//!   （如 `v2.0.0`/`v2.0.1`/`v2.0.2`），不会拉到不兼容的 v3 / v2.1 等版本。
//! - 想要"只锁主版本号、不限制 minor"时，把 `max_minor` 传 `u32::MAX` 即可，
//!   此时允许 `v2.0.x` / `v2.1.x` / `v2.2.x` 等任意 minor。
//! - 任何错误（网络失败、API 限流、JSON 解析失败、范围内无匹配）一律
//!   fallback 到调用方提供的 `fallback` tag，并打 `tracing::warn!`。
//! - 仅影响 v2 流程，v1 完全不动。

use serde::Deserialize;
use tracing::warn;

use crate::const_value;

/// 解析 `v{major}.{minor}.{patch}` 格式的 tag。失败返回 None。
///
/// 不依赖 regex crate，纯字符串切分。
fn parse_semver_tag(tag: &str) -> Option<(u32, u32, u32)> {
    let stripped = tag.strip_prefix('v')?;
    let parts: Vec<&str> = stripped.split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}

/// `max_minor == u32::MAX` 表示"不限 minor,只锁 major"。
const UNBOUNDED_MINOR: u32 = u32::MAX;

/// GitHub releases API 返回的 release 对象（仅用到的字段）。
#[derive(Debug, Clone, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
}

/// 从 GitHub releases API 自动解析最新可用 tag。
///
/// 行为：
/// 1. `GET {GITHUB_RELEASES_API}?per_page=100` 拉取 release 列表
/// 2. 过滤 `draft == false && prerelease == false`
/// 3. 解析 `v{major}.{minor}.{patch}`，保留 `major == max_major && (max_minor == u32::MAX || minor <= max_minor)`
/// 4. 按 (major, minor, patch) 降序排序，取首个
/// 5. 任何错误（网络/JSON/无匹配）→ 返回 `fallback`，打 `tracing::warn!`
///
/// 注意：传入的 `max_major` / `max_minor` 是客户端的"已知兼容"上限，
/// 即 `crate::const_value::v2::MAX_SUPPORTED_MAJOR` / `MAX_SUPPORTED_MINOR`。
/// 想只锁主版本号、不限制 minor 时把 `max_minor` 传 `u32::MAX`。
pub async fn resolve_latest_tag(max_major: u32, max_minor: u32, fallback: &str) -> String {
    let url = const_value::v2::GITHUB_RELEASES_API;
    let filter_desc = if max_minor == UNBOUNDED_MINOR {
        format!("v{}.x.x", max_major)
    } else {
        format!("v{}.{}.x", max_major, max_minor)
    };
    match fetch_and_pick(url, max_major, max_minor).await {
        Some(tag) => {
            tracing::info!("自动解析 v2 最新 tag: {} (范围 {})", tag, filter_desc);
            tag
        }
        None => {
            warn!(
                "无法从 GitHub releases 解析最新 v2 tag (范围 {}),fallback -> {}",
                filter_desc, fallback
            );
            fallback.to_string()
        }
    }
}

async fn fetch_and_pick(api_url: &str, max_major: u32, max_minor: u32) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("shmtu-ocr/resolve_latest_tag")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let url = format!("{}?per_page=100", api_url);
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        warn!(
            "GitHub releases API 返回非成功状态: HTTP {}",
            resp.status()
        );
        return None;
    }
    let releases: Vec<GhRelease> = resp.json().await.ok()?;

    let mut candidates: Vec<(u32, u32, u32, String)> = releases
        .into_iter()
        .filter(|r| !r.draft && !r.prerelease)
        .filter_map(|r| {
            let (maj, min, pat) = parse_semver_tag(&r.tag_name)?;
            if maj != max_major {
                return None;
            }
            // max_minor == u32::MAX 表示无 minor 上界（只锁 major）。
            if max_minor != UNBOUNDED_MINOR && min > max_minor {
                return None;
            }
            Some((maj, min, pat, r.tag_name))
        })
        .collect();

    // 降序排列:major desc, minor desc, patch desc
    candidates.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then(b.2.cmp(&a.2)));
    candidates.into_iter().next().map(|(_, _, _, tag)| tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_semver_tag_basic() {
        assert_eq!(parse_semver_tag("v2.0.2"), Some((2, 0, 2)));
        assert_eq!(parse_semver_tag("v2.0.0"), Some((2, 0, 0)));
        assert_eq!(parse_semver_tag("v1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver_tag("v10.20.30"), Some((10, 20, 30)));
    }

    #[test]
    fn parse_semver_tag_invalid() {
        assert_eq!(parse_semver_tag("2.0.2"), None); // 缺 v 前缀
        assert_eq!(parse_semver_tag("v2.0"), None); // 缺 patch
        assert_eq!(parse_semver_tag("v2.0.2.1"), None); // 4 段
        assert_eq!(parse_semver_tag("v2.0.x"), None); // 非数字
        assert_eq!(parse_semver_tag(""), None);
        assert_eq!(parse_semver_tag("v"), None);
    }

    #[test]
    fn unbounded_minor_constant_matches_u32_max() {
        assert_eq!(UNBOUNDED_MINOR, u32::MAX);
    }
}

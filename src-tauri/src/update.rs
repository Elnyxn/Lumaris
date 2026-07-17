//! GitHub 更新检测 + 外链打开

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

pub const GITHUB_OWNER: &str = "Elnyxn";
pub const GITHUB_REPO: &str = "Lumaris";
pub const GITHUB_URL: &str = "https://github.com/Elnyxn/Lumaris";
pub const GITHUB_RELEASES_URL: &str = "https://github.com/Elnyxn/Lumaris/releases";

const CHECK_COOLDOWN: Duration = Duration::from_secs(60);

static LAST_CHECK: Mutex<Option<(Instant, UpdateCheckResult)>> = Mutex::new(None);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: String,
    pub html_url: String,
    pub body: Option<String>,
    /// 无网 / API 失败时的提示（可空）
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    draft: Option<bool>,
    prerelease: Option<bool>,
}

/// 仅允许 http(s) 且主机为 github.com / api.github.com
pub fn validate_external_url(url: &str) -> Result<(), String> {
    let url = url.trim();
    if url.len() > 512 {
        return Err("链接过长".into());
    }
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err("仅支持 http(s) 链接".into());
    }
    let rest = lower
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or("");
    let host = rest.split('/').next().unwrap_or("").split(':').next().unwrap_or("");
    if host != "github.com" && host != "www.github.com" && host != "api.github.com" {
        return Err("仅允许打开 GitHub 相关链接".into());
    }
    Ok(())
}

pub fn open_external_url(url: &str) -> Result<(), String> {
    validate_external_url(url)?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // start "" "url"
        std::process::Command::new("cmd.exe")
            .args(["/C", "start", "", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = url;
        Err("当前平台不支持打开外链".into())
    }
}

fn normalize_version(s: &str) -> String {
    s.trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

/// 简易 semver 比较：返回 Ordering（major.minor.patch，非数字段当 0）
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
        normalize_version(s)
            .split(|c| c == '.' || c == '-' || c == '+')
            .take(3)
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let mut va = parse(a);
    let mut vb = parse(b);
    while va.len() < 3 {
        va.push(0);
    }
    while vb.len() < 3 {
        vb.push(0);
    }
    va.cmp(&vb)
}

pub fn check_for_updates(force: bool) -> UpdateCheckResult {
    let current = env!("CARGO_PKG_VERSION").to_string();

    if !force {
        if let Some((at, cached)) = LAST_CHECK.lock().as_ref() {
            if at.elapsed() < CHECK_COOLDOWN {
                return cached.clone();
            }
        }
    }

    let result = fetch_latest_release(&current);
    *LAST_CHECK.lock() = Some((Instant::now(), result.clone()));
    result
}

fn fetch_latest_release(current: &str) -> UpdateCheckResult {
    let api = format!(
        "https://api.github.com/repos/{GITHUB_OWNER}/{GITHUB_REPO}/releases/latest"
    );
    let ua = format!("Lumaris/{} (+{GITHUB_URL})", current);

    let response = (|| -> Result<GhRelease, String> {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(12))
            .build();
        let resp = agent
            .get(&api)
            .set("User-Agent", &ua)
            .set("Accept", "application/vnd.github+json")
            .call()
            .map_err(|e| format!("网络请求失败: {e}"))?;
        if !(200..300).contains(&resp.status()) {
            return Err(format!("GitHub API 状态 {}", resp.status()));
        }
        resp.into_json::<GhRelease>()
            .map_err(|e| format!("解析发布信息失败: {e}"))
    })();

    match response {
        Ok(rel) => {
            if rel.draft.unwrap_or(false) {
                return UpdateCheckResult {
                    current_version: current.into(),
                    latest_version: current.into(),
                    update_available: false,
                    release_url: GITHUB_RELEASES_URL.into(),
                    html_url: GITHUB_URL.into(),
                    body: None,
                    error: Some("最新发布为草稿，已忽略".into()),
                };
            }
            // 预发布：仍告知，但不自动标为必须更新也可
            let latest = normalize_version(&rel.tag_name);
            let cur_n = normalize_version(current);
            let update_available = version_cmp(&latest, &cur_n) == std::cmp::Ordering::Greater;
            UpdateCheckResult {
                current_version: cur_n,
                latest_version: latest,
                update_available,
                release_url: if rel.html_url.is_empty() {
                    GITHUB_RELEASES_URL.into()
                } else {
                    rel.html_url.clone()
                },
                html_url: rel.html_url,
                body: rel.body.map(|b| {
                    if b.chars().count() > 800 {
                        format!("{}…", b.chars().take(800).collect::<String>())
                    } else {
                        b
                    }
                }),
                error: if rel.prerelease.unwrap_or(false) {
                    Some("当前最新为预发布版本".into())
                } else {
                    None
                },
            }
        }
        Err(e) => UpdateCheckResult {
            current_version: normalize_version(current),
            latest_version: normalize_version(current),
            update_available: false,
            release_url: GITHUB_RELEASES_URL.into(),
            html_url: GITHUB_URL.into(),
            body: None,
            error: Some(e),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_order() {
        assert_eq!(version_cmp("1.0.0", "1.0.1"), std::cmp::Ordering::Less);
        assert_eq!(version_cmp("v1.2.0", "1.1.9"), std::cmp::Ordering::Greater);
        assert_eq!(version_cmp("1.0", "1.0.0"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn url_guard() {
        assert!(validate_external_url("https://github.com/Elnyxn/Lumaris").is_ok());
        assert!(validate_external_url("https://evil.com").is_err());
        assert!(validate_external_url("file:///etc/passwd").is_err());
    }
}

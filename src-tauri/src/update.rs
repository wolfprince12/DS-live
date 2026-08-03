//! 启动时的版本更新检查（每次启动只跑一次，由前端调用）。
//!
//! 为什么要自己写而不用 tauri-plugin-updater：
//! 官方 updater 走「自动下载 + 就地替换」，需要签名密钥与公证；本项目是 ad-hoc
//! 签名、无 Apple 开发者账号，自动替换会被 Gatekeeper 拦。所以这里只做「检测 +
//! 引导用户手动下载」，不做自动安装。
//!
//! 国内网络现实：api.github.com 常年不稳。因此按三级降级探测最新版本：
//!   1. 直连 api.github.com          —— 成功即判定「用户能读 GitHub」
//!   2. 国内加速镜像代理同一个 API   —— 拿得到完整 release 信息（含更新说明）
//!   3. jsDelivr 的包元数据接口      —— 只能拿到最新 tag，但国内可达性最好
//! 三级全挂 = 断网或全被墙，返回 checked=false，前端静默不打扰用户。

use serde::{Deserialize, Serialize};
use std::time::Duration;

const REPO: &str = "wolfprince12/DSonDT";
/// 单个源的超时。启动时后台跑，宁可快速失败也不要拖慢体验。
const TIMEOUT: Duration = Duration::from_secs(8);

/// 国内可用的 GitHub **下载**加速镜像前缀（拼在完整 https://github.com/... 前面）。
/// 2026-08-02 实测三家对 releases/download 均可用（HTTP 206）。
/// 顺序即优先级；任一失效不影响其它，前端也会把备选镜像全列给用户。
const DL_MIRRORS: &[&str] = &[
    "https://ghfast.top/",
    "https://gh-proxy.com/",
    "https://ghproxy.net/",
];

/// 能代理 **api.github.com** 的镜像。注意：这和下载镜像不是一回事——
/// 实测 ghfast.top / ghproxy.net 对 API 地址一律回 "Invalid input."，只有
/// gh-proxy.com 能转发 API（但共享 IP 常触发 GitHub 限流，所以还有 jsDelivr 兜底）。
const API_MIRRORS: &[&str] = &["https://gh-proxy.com/"];

#[derive(Debug, Serialize, Clone, Default)]
pub struct UpdateInfo {
    /// 是否成功查到了远端版本。false = 所有源都失败（断网/被墙），前端应静默。
    pub checked: bool,
    /// 是否存在比当前更新的版本。
    pub has_update: bool,
    /// 当前运行的版本号（不带 v 前缀）。
    pub current: String,
    /// 远端最新版本号（不带 v 前缀）。
    pub latest: String,
    /// GitHub 是否可以直连。false = 需要引导用户走国内镜像。
    pub github_reachable: bool,
    /// 首选下载地址：直连可达时是官方链接，否则是镜像加速链接。
    pub download_url: String,
    /// 备用镜像地址（GitHub 不可达时给用户多几条路）。
    pub mirror_urls: Vec<String>,
    /// 更新说明（jsDelivr 兜底路径下可能为空）。
    pub notes: String,
    /// 官方 Release 页面地址。
    pub release_url: String,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct JsdVersion {
    version: String,
}

/// jsDelivr 包元数据。实测本仓库的 `tags` 是空对象，真正有用的是 `versions`
/// 数组（形如 ["0.3.3","0.3.2",...]），所以两个字段都读、以版本号最大者为准。
#[derive(Deserialize)]
struct JsdMeta {
    #[serde(default)]
    tags: std::collections::HashMap<String, String>,
    #[serde(default)]
    versions: Vec<JsdVersion>,
}

/// 把 "v0.3.10-beta" 这类版本串解析成可比较的数字段，非数字段按 0 处理。
fn parse_ver(s: &str) -> Vec<u32> {
    s.trim()
        .trim_start_matches(['v', 'V'])
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .split('.')
        .map(|p| {
            p.chars()
                .take_while(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

/// 逐段比较，latest > current 才算有更新（0.3.10 > 0.3.9，不做字符串比较）。
fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (parse_ver(latest), parse_ver(current));
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// 抓一个 release 接口。
///
/// 返回 `(reached, release)`：
/// - `reached` = 服务器给了任何 HTTP 响应（哪怕 403 限流）。这正是判断「用户能不能
///   读 GitHub」的正确依据——限流不等于被墙，此时仍应引导用户走官方链接。
/// - `release` = 只有 2xx 且 JSON 能解析时才有值。
async fn fetch_release(client: &reqwest::Client, url: &str) -> (bool, Option<GhRelease>) {
    match client.get(url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                return (true, None); // 连上了，但没数据（限流 / 404）
            }
            (true, resp.json::<GhRelease>().await.ok())
        }
        Err(_) => (false, None), // 超时 / DNS / 连接被拒 —— 判定不可达
    }
}

/// 兜底：问 jsDelivr「这仓库发布过哪些版本」，取最大的一个。拿不到发布说明。
async fn fetch_latest_tag_via_jsdelivr(client: &reqwest::Client) -> Option<String> {
    let url = format!("https://data.jsdelivr.com/v1/packages/gh/{REPO}");
    let resp = client.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let meta = resp.json::<JsdMeta>().await.ok()?;
    if let Some(t) = meta.tags.get("latest") {
        return Some(t.clone());
    }
    // versions 一般已降序，但不依赖这个假设，自己比一遍最大值
    meta.versions
        .into_iter()
        .map(|v| v.version)
        .reduce(|acc, v| if is_newer(&v, &acc) { v } else { acc })
}

/// 从 release 资产里挑 macOS 安装包（.dmg）；没有就退回 Release 页面。
fn pick_dmg(rel: &GhRelease) -> Option<String> {
    rel.assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".dmg"))
        .map(|a| a.browser_download_url.clone())
}

#[tauri::command]
pub async fn check_update() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    // GitHub API 强制要求 User-Agent，缺了直接 403。
    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(format!("DSonDT/{current} (macOS; update-check)"))
        .build()
        .map_err(|e| e.to_string())?;

    let api = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let release_page = format!("https://github.com/{REPO}/releases/latest");

    let mut info = UpdateInfo {
        current: current.clone(),
        release_url: release_page.clone(),
        ..Default::default()
    };

    // ---- 第 1 级：直连 GitHub API。只要有 HTTP 响应就认为 GitHub 可达 ----
    let (reached, mut release) = fetch_release(&client, &api).await;
    info.github_reachable = reached;

    // ---- 第 2 级：镜像代理同一个 API（直连被墙、或限流拿不到数据时） ----
    if release.is_none() {
        for m in API_MIRRORS {
            let (_, r) = fetch_release(&client, &format!("{m}{api}")).await;
            if r.is_some() {
                release = r;
                break;
            }
        }
    }

    let (latest_tag, notes, official_dl) = match &release {
        Some(r) => {
            let dl = pick_dmg(r).unwrap_or_else(|| {
                if r.html_url.is_empty() {
                    release_page.clone()
                } else {
                    r.html_url.clone()
                }
            });
            (r.tag_name.clone(), r.body.clone(), dl)
        }
        None => {
            // ---- 第 3 级：jsDelivr 只报最新 tag，够判断「要不要提示」了 ----
            match fetch_latest_tag_via_jsdelivr(&client).await {
                // 拿不到 assets 列表，按固定命名约定拼 DMG 直链
                // （DSonDT-<版本>-aarch64.dmg，历次发布都是这个格式）。
                // 这一步很关键：Release 页面本身镜像代理不了，但 releases/download
                // 直链三家镜像都能转发，所以必须给直链而不是页面地址。
                Some(tag) => {
                    let v = tag.trim_start_matches(['v', 'V']);
                    let dl = format!(
                        "https://github.com/{REPO}/releases/download/v{v}/DSonDT-{v}-aarch64.dmg"
                    );
                    (tag.clone(), String::new(), dl)
                }
                None => return Ok(info), // checked=false，前端静默
            }
        }
    };

    let latest = latest_tag.trim_start_matches(['v', 'V']).to_string();
    info.checked = true;
    info.latest = latest.clone();
    info.notes = notes;
    info.has_update = is_newer(&latest, &current);

    if !info.has_update {
        return Ok(info);
    }

    // 有更新：按可达性决定给官方链接还是镜像链接。
    info.mirror_urls = DL_MIRRORS.iter().map(|m| format!("{m}{official_dl}")).collect();
    info.download_url = if info.github_reachable {
        official_dl
    } else {
        info.mirror_urls
            .first()
            .cloned()
            .unwrap_or_else(|| official_dl.clone())
    };

    Ok(info)
}

/// 返回当前安装的版本号（来自 Cargo.toml 的 CARGO_PKG_VERSION）。
/// 不联网，给「关于」弹窗即时显示本地版本用。
#[tauri::command]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn version_compare() {
        assert!(is_newer("0.3.4", "0.3.3"));
        assert!(is_newer("v0.4.1", "0.3.9"));
        assert!(is_newer("0.3.10", "0.3.9")); // 数字比较，不是字符串
        assert!(!is_newer("0.3.3", "0.3.3"));
        assert!(!is_newer("0.3.2", "0.3.3"));
        assert!(!is_newer("garbage", "0.3.3"));
    }
}

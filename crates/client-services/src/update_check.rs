//! 在共用仓库的发布列表里查找本平台、本版本（edition，如 `full`、五笔版）有没有更新的构建。
//!
//! 各平台都发布到 `metasequoiaime/msime`，各用自己的 tag 前缀（`windows-v1.2.0`、`linux-v1.2.0`，见 `.github/workflows/release-*.yml`），所以仓库唯一的「latest」发布通常属于别的平台，带前缀的 tag 本身也不是版本号。[`select_platform_release`] 因此读整个列表，只保留某一平台最新的已发布版本。同一平台的各个版本共用一个 tag，只靠资产名区分，所以在 Linux 和 Windows 上它还要挑出属于本版本（Linux 上还要求本架构）的唯一一个安装包，设置页可以展示它的文件名和 SHA-256。
//!
//! 各宿主的设置界面都调用 [`check_for_update`]：Tauri 外壳经由它的 `update_check` 命令，原生 Windows 设置窗口和 HarmonyOS 页面经由 `msime_client_update_check`。结果怎么向用户说明（未签名警告、可复制的校验命令、镜像链接）仍由各界面自己决定。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::time::Duration;

/// 仓库的发布列表，最新的在前；一页就能装下所有平台近期的发布。
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/metasequoiaime/msime/releases?per_page=100";
/// 仓库所有发布页面所在的地址；不在它下面的发布 URL 一律不提供。
pub const RELEASES_PAGE_URL: &str = "https://github.com/metasequoiaime/msime/releases";
/// 一次检查最多等 GitHub 多久，超时即报告不可用。
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// 一页 100 个发布连同说明和资产，大小在几 MB。
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
/// GitHub 的 REST API 会拒绝不带 User-Agent 的请求。
const USER_AGENT: &str = "msime-client";

/// 设置页展示和比较时使用的发布版本号：只保留点分数字，去掉开头的 `v` 以及预发布、构建后缀。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Version {
    pub display: String,
    pub parts: Vec<u64>,
}

/// 某个平台的最新发布；能选出本版本的安装包时一并带上。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReleaseUpdate {
    pub version: Version,
    /// 该发布在 [`RELEASES_PAGE_URL`] 下的页面。
    pub release_url: String,
    /// 本版本（Linux 上还限定本架构）唯一的安装包；该发布没有或有不止一个时为 `None`。
    pub installer_name: Option<String>,
    /// GitHub 计算的安装包 SHA-256，小写十六进制；仅在 GitHub 给出时才有。
    pub installer_sha256: Option<String>,
    /// Linux 和 Windows 的发布 workflow 发布的是未签名安装包，因此为 `Some(false)`；不清楚的平台为 `None`。
    pub signed: Option<bool>,
}

/// 一次检查的结果，宿主序列化后的形状为 `{"status": "available" | "current" | "none", "update": ReleaseUpdate | null}`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", content = "update", rename_all = "lowercase")]
pub enum UpdateCheck {
    /// 有版本号高于当前运行版本号的发布。
    Available(ReleaseUpdate),
    /// 最新发布的版本号不高于当前运行的版本号。
    Current(ReleaseUpdate),
    /// 该平台还没有已发布的版本。
    None,
}

/// 宿主发来的请求：本平台的 tag 前缀、当前运行的版本号，以及可选的版本 id（缺省为 `full`）和 Rust 对本机架构的命名。
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCheckRequest {
    pub platform: String,
    pub current_version: String,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UpdateCheckError {
    /// 请求没有给出平台，或者给出的当前运行版本号不合法。
    #[error("invalid update check request")]
    Invalid,
    /// 连不上 GitHub、GitHub 返回了错误，或者返回的内容不是发布列表。
    #[error("the release list is unavailable")]
    Unavailable,
}

/// 拉取发布列表，把其中 `request.platform` 的最新发布与当前运行的版本号比较。会在网络上阻塞最多 [`TIMEOUT`]，不要在 UI 线程或输入线程上调用。
pub fn check_for_update(request: &UpdateCheckRequest) -> Result<UpdateCheck, UpdateCheckError> {
    validate_request(request)?;
    evaluate_update(&fetch_release_list()?, request)
}

/// [`check_for_update`] 拿到发布列表的字节之后所做的判断。
pub fn evaluate_update(
    release_list: &[u8],
    request: &UpdateCheckRequest,
) -> Result<UpdateCheck, UpdateCheckError> {
    let current = validate_request(request)?;
    let releases: Vec<Value> =
        serde_json::from_slice(release_list).map_err(|_| UpdateCheckError::Unavailable)?;
    let Some(update) = select_platform_release(
        &releases,
        &request.platform,
        request.edition.as_deref(),
        request.arch.as_deref(),
    ) else {
        return Ok(UpdateCheck::None);
    };
    Ok(
        if compare_versions(&update.version, &current) == Ordering::Greater {
            UpdateCheck::Available(update)
        } else {
            UpdateCheck::Current(update)
        },
    )
}

fn validate_request(request: &UpdateCheckRequest) -> Result<Version, UpdateCheckError> {
    if request.platform.is_empty() || request.platform.len() > 32 {
        return Err(UpdateCheckError::Invalid);
    }
    parse_version(&request.current_version).ok_or(UpdateCheckError::Invalid)
}

fn fetch_release_list() -> Result<Vec<u8>, UpdateCheckError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| UpdateCheckError::Unavailable)?;
    let response = client
        .get(RELEASES_URL)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|_| UpdateCheckError::Unavailable)?;
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(UpdateCheckError::Unavailable);
    }
    crate::bounded_io::read_bounded(response, MAX_RESPONSE_BYTES)
        .map_err(|_| UpdateCheckError::Unavailable)
}

/// 把 `v1.2.0`、`1.2`、`1.2.0-beta` 或 `1.2.0+build` 解析为开头的点分数字；其他形式（包括前后夹带文字的）都不算版本号。
pub fn parse_version(value: &str) -> Option<Version> {
    let value = value.trim();
    let value = value
        .strip_prefix('v')
        .or_else(|| value.strip_prefix('V'))
        .unwrap_or(value);
    let end = value.find(['-', '+']).unwrap_or(value.len());
    let display = &value[..end];
    let parts = display
        .split('.')
        .map(|part| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse::<u64>().ok())
                .flatten()
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Version {
        display: display.to_owned(),
        parts,
    })
}

/// 逐段比较，缺少的段按 0 计：`1.2` 等于 `1.2.0`。
pub fn compare_versions(left: &Version, right: &Version) -> Ordering {
    let length = left.parts.len().max(right.parts.len());
    (0..length)
        .map(|index| {
            let part = |version: &Version| version.parts.get(index).copied().unwrap_or(0);
            part(left).cmp(&part(right))
        })
        .find(|ordering| ordering.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// 仓库发布列表中 `platform` 最新的已发布版本，在 Linux 和 Windows 上附带本版本的安装包。草稿和预发布不提供。
///
/// `edition` 是当前所运行版本（edition）的 id，缺省为 `full`；同一平台的各版本共用 tag，只能靠资产名区分。`arch` 是 Rust 对宿主架构的命名；Linux 上只提供该架构的安装包。
pub fn select_platform_release(
    releases: &[Value],
    platform: &str,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<ReleaseUpdate> {
    let prefix = format!("{platform}-");
    let mut newest: Option<ReleaseUpdate> = None;
    for release in releases {
        if release.get("draft") == Some(&Value::Bool(true))
            || release.get("prerelease") == Some(&Value::Bool(true))
        {
            continue;
        }
        let Some(tag) = release
            .get("tag_name")
            .and_then(Value::as_str)
            .and_then(|tag| tag.strip_prefix(&prefix))
        else {
            continue;
        };
        let Some(html_url) = release.get("html_url").and_then(Value::as_str) else {
            continue;
        };
        let Some(mut update) = validate_release(tag, html_url) else {
            continue;
        };
        let assets = release.get("assets").and_then(Value::as_array);
        match platform {
            // Linux 的产物都没有签名（`.deb` 没有，也没有分离的 GPG 签名）。
            "linux" => {
                let package = linux_package(assets, edition, arch);
                update.installer_name = package.as_ref().map(|asset| asset.name.clone());
                update.installer_sha256 = package.and_then(|asset| asset.sha256);
                update.signed = Some(false);
            }
            // 发布 workflow 发布的是未签名的安装程序；签名是本地手动完成的步骤。
            "windows" => {
                let installer = edition_installer_prefix(edition).and_then(|prefix| {
                    select_unique_release_asset(
                        assets.map(|assets| assets.iter().collect()),
                        &[&|name: &str| windows_installer_matches(name, &prefix)],
                    )
                });
                update.installer_name = installer.as_ref().map(|asset| asset.name.clone());
                update.installer_sha256 = installer.and_then(|asset| asset.sha256);
                update.signed = Some(false);
            }
            _ => {}
        }
        if newest
            .as_ref()
            .is_none_or(|newest| compare_versions(&update.version, &newest.version).is_gt())
        {
            newest = Some(update);
        }
    }
    newest
}

/// 页面位于 [`RELEASES_PAGE_URL`] 之下、且去掉平台前缀后的 tag 是合法版本号的发布。
fn validate_release(tag: &str, html_url: &str) -> Option<ReleaseUpdate> {
    if !is_https_url(html_url) || !html_url.starts_with(&format!("{RELEASES_PAGE_URL}/tag/")) {
        return None;
    }
    Some(ReleaseUpdate {
        version: parse_version(tag)?,
        release_url: html_url.to_owned(),
        installer_name: None,
        installer_sha256: None,
        signed: None,
    })
}

fn is_https_url(value: &str) -> bool {
    value.starts_with("https://")
        && !value.chars().any(|c| {
            c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '<' | '>' | '\\' | '|' | '&')
        })
}

/// 为设置页选出的发布资产。
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReleaseAsset {
    name: String,
    sha256: Option<String>,
}

type AssetPattern<'a> = &'a dyn Fn(&str) -> bool;

/// 由第一个有匹配的模式选出的唯一资产。某个模式匹配到两个资产时什么都不选，也不再尝试后面的模式：有两个候选时，无论展示哪一个摘要，总会对某些用户是错的。
fn select_unique_release_asset(
    assets: Option<Vec<&Value>>,
    patterns: &[AssetPattern<'_>],
) -> Option<ReleaseAsset> {
    let assets = assets?;
    for pattern in patterns {
        let mut matches = assets.iter().filter_map(|asset| {
            let name = asset.get("name")?.as_str()?;
            pattern(name).then_some((name, *asset))
        });
        let Some((name, asset)) = matches.next() else {
            continue;
        };
        if matches.next().is_some() {
            return None;
        }
        return Some(ReleaseAsset {
            name: name.to_owned(),
            sha256: github_digest(asset.get("digest")),
        });
    }
    None
}

/// GitHub 的 `sha256:<lowercase hex>` 资产摘要。较老的 API 响应会省略它或给 `null`；其他算法或格式一律不信任。
fn github_digest(digest: Option<&Value>) -> Option<String> {
    let hex = digest?.as_str()?.strip_prefix("sha256:")?;
    (hex.len() == 64
        && hex
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')))
    .then(|| hex.to_owned())
}

fn is_full_edition(edition: Option<&str>) -> bool {
    edition.is_none_or(|edition| edition == "full")
}

/// 可以拼进资产名的版本 id。
fn is_edition_id(id: &str) -> bool {
    id.bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// 该版本 Windows 安装程序的文件名前缀，匹配时不区分大小写：`full`（以及未给出版本时）为 `MetasequoiaIME-Full_Setup_v`，五笔版为 `MetasequoiaIME-Wubi_Setup_v`。不是合法版本 id 时为 `None`。`scripts/test-editions.py` 推导出同样的名字。
fn edition_installer_prefix(edition: Option<&str>) -> Option<String> {
    let id = edition.unwrap_or("full");
    let mut characters = id.chars();
    let first = characters.next()?;
    is_edition_id(id).then(|| {
        format!(
            "MetasequoiaIME-{}{}_Setup_v",
            first.to_ascii_uppercase(),
            characters.as_str()
        )
    })
}

/// `<prefix>` + 一个或多个 `[A-Za-z0-9_.-]` + `.exe`，不区分大小写。文件名会出现在用户可能复制的 shell 命令里，所以只允许不需要加引号的字符。
fn windows_installer_matches(name: &str, prefix: &str) -> bool {
    strip_prefix_ignore_case(name, prefix)
        .and_then(|rest| strip_suffix_ignore_case(rest, ".exe"))
        .is_some_and(|middle| !middle.is_empty() && middle.chars().all(is_installer_char))
}

fn is_installer_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// CPack 在 Linux 包名中除包名本身以外会用到的字符（见 `platforms/linux/cmake/packaging.cmake`）。
fn is_package_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '~' | '-')
}

/// 本机上该版本的 Linux 安装包：恰好有一个 `.deb` 时选它，否则选 `.tar.gz`。
fn linux_package(
    assets: Option<&Vec<Value>>,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<ReleaseAsset> {
    let mut assets: Vec<&Value> = assets?.iter().collect();
    let full = is_full_edition(edition);
    if full {
        // `full` 的模式也会匹配到其他版本的安装包（`msime-linux-wubi_…`）；`full` 的包名在 `msime-linux` 之后紧跟 `_` 或版本号。
        assets.retain(|asset| !asset_name(asset).is_some_and(is_other_edition_linux_package));
    }
    if let Some((deb, tarball)) = arch.and_then(linux_package_architecture) {
        // 一个发布里每种架构各有一个 `.deb` 和一个 `.tar.gz`，只保留本机架构的；架构未知时全部保留。
        assets.retain(|asset| {
            asset_name(asset).is_some_and(|name| {
                name.ends_with(&format!("_{deb}.deb"))
                    || name.ends_with(&format!("-linux-{tarball}.tar.gz"))
            })
        });
    }
    if full {
        return select_unique_release_asset(
            Some(assets),
            &[
                &|name: &str| full_linux_package_matches(name, ".deb"),
                &|name: &str| full_linux_package_matches(name, ".tar.gz"),
            ],
        );
    }
    let id = edition.filter(|id| is_edition_id(id))?;
    select_unique_release_asset(
        Some(assets),
        &[
            &|name: &str| edition_deb_matches(name, id),
            &|name: &str| edition_tarball_matches(name, id),
        ],
    )
}

fn asset_name(asset: &Value) -> Option<&str> {
    asset.get("name")?.as_str()
}

/// 包名里 dpkg 和 CMake 对宿主架构的命名，以 Rust 的架构名为键。
fn linux_package_architecture(arch: &str) -> Option<(&'static str, &'static str)> {
    match arch {
        "x86_64" => Some(("amd64", "x86_64")),
        "aarch64" => Some(("arm64", "aarch64")),
        _ => None,
    }
}

/// `msime-linux-<letter>…`：其他版本的安装包。
fn is_other_edition_linux_package(name: &str) -> bool {
    strip_prefix_ignore_case(name, "msime-linux-")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_alphabetic())
}

/// 任何不需要加引号、开头也不像命令行选项的包名：`[A-Za-z0-9]`，接 `[\w.+~-]*`，再接 `suffix`。
fn full_linux_package_matches(name: &str, suffix: &str) -> bool {
    strip_suffix_ignore_case(name, suffix).is_some_and(|stem| {
        let mut characters = stem.chars();
        characters.next().is_some_and(|c| c.is_ascii_alphanumeric())
            && characters.all(is_package_char)
    })
}

/// `msime-linux-<id>_` + 一个或多个包名字符 + `.deb`。
fn edition_deb_matches(name: &str, id: &str) -> bool {
    strip_prefix_ignore_case(name, &format!("msime-linux-{id}_"))
        .and_then(|rest| strip_suffix_ignore_case(rest, ".deb"))
        .is_some_and(|middle| !middle.is_empty() && middle.chars().all(is_package_char))
}

/// `msime-linux-<id>-` + 一个数字 + 包名字符 + `.tar.gz`。
fn edition_tarball_matches(name: &str, id: &str) -> bool {
    strip_prefix_ignore_case(name, &format!("msime-linux-{id}-"))
        .and_then(|rest| strip_suffix_ignore_case(rest, ".tar.gz"))
        .is_some_and(|middle| {
            let mut characters = middle.chars();
            characters.next().is_some_and(|c| c.is_ascii_digit()) && characters.all(is_package_char)
        })
}

fn strip_prefix_ignore_case<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let head = value.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &value[prefix.len()..])
}

fn strip_suffix_ignore_case<'a>(value: &'a str, suffix: &str) -> Option<&'a str> {
    let start = value.len().checked_sub(suffix.len())?;
    let tail = value.get(start..)?;
    tail.eq_ignore_ascii_case(suffix).then(|| &value[..start])
}

#[cfg(test)]
mod tests;

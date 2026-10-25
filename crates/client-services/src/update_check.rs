//! 在共用仓库的发布列表里查找本平台、本版本（edition，如 `full`、五笔版）有没有更新的构建。
//!
//! 各平台都发布到 `metasequoiaime/msime`，各用自己的 tag 前缀（`windows-v1.2.0`、`linux-v1.2.0`，见 `.github/workflows/release-*.yml`），所以仓库唯一的「latest」发布通常属于别的平台，带前缀的 tag 本身也不是版本号。[`select_platform_release`] 因此读整个列表，只保留某一平台最新的已发布版本。同一平台的各个版本共用一个 tag，只靠资产名区分，所以在 Linux 和 Windows 上它还要挑出属于本版本（Linux 上还要求本架构）的唯一一个安装包，设置页可以展示它的文件名和 SHA-256。
//!
//! 各宿主的设置界面都调用 [`check_for_update`]：Tauri 外壳经由它的 `update_check` 命令，原生 Windows 设置窗口和 HarmonyOS 页面经由 `msime_client_update_check`。结果怎么向用户说明（未签名警告、可复制的校验命令、镜像链接）仍由各界面自己决定。
//!
//! 请求可以带更新通道（[`UpdateChannel`]）：稳定版只认正式发布；测试版另外接受 tag 恰为 `<platform>-vX.Y.Z-beta` 的预发布（develop 发出的，`-alpha` 不算）；每日构建再接受滚动的 `nightly` 发布，从它正文末尾的 `msime-nightly-state` 里取本平台的版本号、运行 ID 和附件（见 `scripts/publish-nightly.py`）。同一版本号可能有好几次构建（beta 之后发正式版、每天的 nightly），所以比较的是（版本号，发布 workflow 的运行 ID）：正式版和测试版的运行 ID 写在发布正文的 `msime-build` 隐藏标记里（`scripts/release-build-marker.py`），本程序自己的运行 ID 是构建时的 `MSIME_BUILD_RUN`（[`BUILD_RUN`]）。
//!
//! 后台定期检查、记录和提醒在 [`background`]。

pub use msime_client_core::preferences::UpdateChannel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::time::Duration;

pub mod background;

/// 仓库的发布列表，最新的在前；一页就能装下所有平台近期的发布。
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/metasequoiaime/msime/releases?per_page=100";
/// 每日构建通道另外读的滚动 `nightly` 发布。它是很久以前建的、之后原地更新，在按建立时间排序的发布列表里会被挤出第一页，所以单独按 tag 取。
pub const NIGHTLY_RELEASE_URL: &str =
    "https://api.github.com/repos/metasequoiaime/msime/releases/tags/nightly";
/// 仓库所有发布页面所在的地址；不在它下面的发布 URL 一律不提供。
pub const RELEASES_PAGE_URL: &str = "https://github.com/metasequoiaime/msime/releases";
/// 滚动的每日构建发布的 tag。
const NIGHTLY_TAG: &str = "nightly";
/// `scripts/publish-nightly.py` 写在 nightly 正文末尾的状态块的开头，到下一个 `\n-->` 为止是 JSON。
const NIGHTLY_STATE_MARKER: &str = "<!-- msime-nightly-state\n";
/// 正式版和测试版发布正文末尾的隐藏标记（`scripts/release-build-marker.py`）：`<!-- msime-build {"run":…,"source":"…","channel":"…"} -->`。
const BUILD_MARKER: &str = "<!-- msime-build ";
/// 构建本程序的发布 workflow 的运行 ID：发布 workflow（nightly 复用它们）在编译 Rust 时把 `MSIME_BUILD_RUN` 设成 `github.run_id`。本机和 CI 的测试构建、发行版从源码编的包没有它，为 `None`。
pub const BUILD_RUN: Option<u64> = parse_build_run(option_env!("MSIME_BUILD_RUN"));
/// 一次检查最多等 GitHub 多久，超时即报告不可用。
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// 一页 100 个发布连同说明和资产，大小在几 MB。
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
/// GitHub 的 REST API 会拒绝不带 User-Agent 的请求。
const USER_AGENT: &str = "msime-client";

/// 设置页展示和比较时使用的发布版本号：只保留点分数字，去掉开头的 `v` 以及预发布、构建后缀。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Version {
    pub display: String,
    pub parts: Vec<u64>,
}

/// 某个平台的最新发布；能选出本版本的安装包时一并带上。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseUpdate {
    pub version: Version,
    /// 该发布在 [`RELEASES_PAGE_URL`] 下的页面。
    pub release_url: String,
    /// 本版本（Linux 上还限定本架构）唯一的安装包；该发布没有或有不止一个时为 `None`。
    pub installer_name: Option<String>,
    /// 安装包的 SHA-256，小写十六进制：正式版和测试版取 GitHub 计算的摘要，仅在 GitHub 给出时才有；每日构建取 nightly 状态里记的摘要。
    pub installer_sha256: Option<String>,
    /// Linux 和 Windows 的发布 workflow 发布的是未签名安装包，因此为 `Some(false)`；不清楚的平台为 `None`。
    pub signed: Option<bool>,
    /// 这个发布属于哪个通道：正式发布 `stable`，`-beta` 预发布 `beta`，滚动的 nightly 发布 `nightly`。以前写下的记录没有这一项，按 `stable` 读。
    #[serde(default)]
    pub channel: UpdateChannel,
    /// 构建它的发布 workflow 运行 ID：正式版和测试版取发布正文的 `msime-build` 标记，每日构建取 nightly 状态里本平台的 `run`。没有标记的旧发布为 `None`。
    #[serde(default)]
    pub build_run: Option<u64>,
}

/// 一次检查的结果，宿主序列化后的形状为 `{"status": "available" | "current" | "none", "update": ReleaseUpdate | null}`。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "update", rename_all = "lowercase")]
pub enum UpdateCheck {
    /// 有比当前运行的构建更新的发布（见 [`is_newer_build`]）。
    Available(ReleaseUpdate),
    /// 最新发布不比当前运行的构建新。
    Current(ReleaseUpdate),
    /// 该平台还没有已发布的版本。
    None,
}

/// 宿主发来的请求：本平台的 tag 前缀、当前运行的版本号，以及可选的版本 id（缺省为 `full`）、Rust 对本机架构的命名和更新通道。
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateCheckRequest {
    pub platform: String,
    pub current_version: String,
    #[serde(default)]
    pub edition: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
    /// `stable`、`beta` 或 `nightly`，缺省为 `stable`；别的值让请求不合法（宿主从偏好读来的值已经由 client-core 把不认识的归成 `stable`）。`windows7` 只有正式发布，任何通道都按 `stable` 处理。
    #[serde(default)]
    pub channel: Option<String>,
}

impl UpdateCheckRequest {
    /// 这次请求实际用的通道：缺省为稳定版，`windows7` 一律为稳定版。通道名不合法时为 `None`。
    pub fn effective_channel(&self) -> Option<UpdateChannel> {
        let channel = match self.channel.as_deref() {
            None => UpdateChannel::Stable,
            Some(value) => UpdateChannel::parse(value)?,
        };
        Some(if self.platform == "windows7" {
            UpdateChannel::Stable
        } else {
            channel
        })
    }
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

/// 拉取发布列表（每日构建通道另外取 nightly 发布），把其中 `request.platform` 在请求通道里的最新发布与当前运行的构建比较。会在网络上阻塞，每个请求最多 [`TIMEOUT`]，不要在 UI 线程或输入线程上调用。
pub fn check_for_update(request: &UpdateCheckRequest) -> Result<UpdateCheck, UpdateCheckError> {
    let (_, channel) = validate_request(request)?;
    let client = http_client()?;
    let mut releases =
        parse_release_list(&fetch(&client, RELEASES_URL)?.ok_or(UpdateCheckError::Unavailable)?)?;
    if channel == UpdateChannel::Nightly {
        // 还没有 nightly 发布（404）时只看列表；取到了但不是一个发布对象，按 GitHub 返回了错误处理。
        if let Some(bytes) = fetch(&client, NIGHTLY_RELEASE_URL)? {
            let nightly: Value =
                serde_json::from_slice(&bytes).map_err(|_| UpdateCheckError::Unavailable)?;
            if !nightly.is_object() {
                return Err(UpdateCheckError::Unavailable);
            }
            if !releases.iter().any(is_nightly_release) {
                releases.push(nightly);
            }
        }
    }
    evaluate_releases(&releases, request, BUILD_RUN)
}

/// [`check_for_update`] 拿到发布列表的字节之后所做的判断，当前运行构建的运行 ID 取 [`BUILD_RUN`]。
pub fn evaluate_update(
    release_list: &[u8],
    request: &UpdateCheckRequest,
) -> Result<UpdateCheck, UpdateCheckError> {
    validate_request(request)?;
    evaluate_releases(&parse_release_list(release_list)?, request, BUILD_RUN)
}

/// 在已解析的发布列表里选出请求通道的最新发布，与当前运行的版本号和运行 ID `current_run` 比较。
fn evaluate_releases(
    releases: &[Value],
    request: &UpdateCheckRequest,
    current_run: Option<u64>,
) -> Result<UpdateCheck, UpdateCheckError> {
    let (current, channel) = validate_request(request)?;
    let Some(update) = select_channel_release(
        releases,
        &request.platform,
        request.edition.as_deref(),
        request.arch.as_deref(),
        channel,
    ) else {
        return Ok(UpdateCheck::None);
    };
    Ok(if is_newer_build(&update, &current, current_run) {
        UpdateCheck::Available(update)
    } else {
        UpdateCheck::Current(update)
    })
}

/// 发布 `update` 是否比当前运行的构建（版本号 `current`、运行 ID `current_run`）新：版本号更高；或者版本号相同、两边的运行 ID 都知道且它的更大。
///
/// 当前构建不知道自己的运行 ID 时（本机构建、发行版从源码编的包）只比版本号：否则同一版本号、带了 `msime-build` 标记的正式发布会被当成更新，从源码编的每一个包都会一直提醒自己的版本。发布没有运行 ID（没有标记的旧发布）时同样只比版本号。
pub fn is_newer_build(update: &ReleaseUpdate, current: &Version, current_run: Option<u64>) -> bool {
    match compare_versions(&update.version, current) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => matches!(
            (update.build_run, current_run),
            (Some(release), Some(current)) if release > current
        ),
    }
}

fn validate_request(
    request: &UpdateCheckRequest,
) -> Result<(Version, UpdateChannel), UpdateCheckError> {
    if request.platform.is_empty() || request.platform.len() > 32 {
        return Err(UpdateCheckError::Invalid);
    }
    let current = parse_version(&request.current_version).ok_or(UpdateCheckError::Invalid)?;
    let channel = request
        .effective_channel()
        .ok_or(UpdateCheckError::Invalid)?;
    Ok((current, channel))
}

fn parse_release_list(bytes: &[u8]) -> Result<Vec<Value>, UpdateCheckError> {
    serde_json::from_slice(bytes).map_err(|_| UpdateCheckError::Unavailable)
}

fn http_client() -> Result<reqwest::blocking::Client, UpdateCheckError> {
    // 版本列表在 GitHub（https 公网）；按用户的网络代理设置走。
    crate::proxy::configure_blocking(reqwest::blocking::Client::builder())
        .timeout(TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|_| UpdateCheckError::Unavailable)
}

/// GET `url` 的响应体；404 为 `None`，其他非成功状态、超时或超过 [`MAX_RESPONSE_BYTES`] 都是 [`UpdateCheckError::Unavailable`]。
fn fetch(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<Option<Vec<u8>>, UpdateCheckError> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|_| UpdateCheckError::Unavailable)?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !response.status().is_success()
        || response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(UpdateCheckError::Unavailable);
    }
    crate::bounded_io::read_bounded(response, MAX_RESPONSE_BYTES)
        .map(Some)
        .map_err(|_| UpdateCheckError::Unavailable)
}

/// `MSIME_BUILD_RUN` 的值：一到十九位十进制数字、不为零；其他写法（空、带符号、带空白）都当作没有。
const fn parse_build_run(value: Option<&str>) -> Option<u64> {
    let Some(value) = value else {
        return None;
    };
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > 19 {
        return None;
    }
    let mut run: u64 = 0;
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if !byte.is_ascii_digit() {
            return None;
        }
        run = run * 10 + (byte - b'0') as u64;
        index += 1;
    }
    if run == 0 {
        None
    } else {
        Some(run)
    }
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

/// 仓库发布列表中 `platform` 最新的已发布版本，在 Linux 和 Windows 上附带本版本的安装包。草稿和预发布不提供。等于稳定版通道的 [`select_channel_release`]。
///
/// `edition` 是当前所运行版本（edition）的 id，缺省为 `full`；同一平台的各版本共用 tag，只能靠资产名区分。`arch` 是 Rust 对宿主架构的命名；Linux 上只提供该架构的安装包。
pub fn select_platform_release(
    releases: &[Value],
    platform: &str,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<ReleaseUpdate> {
    select_channel_release(releases, platform, edition, arch, UpdateChannel::Stable)
}

/// 仓库发布列表中 `platform` 在 `channel` 里最新的构建，按（版本号，运行 ID）取最大，两者都相同时取列表里靠前（较新发布）的那个。草稿一律不提供。
///
/// - 稳定版：只认正式发布，与 [`select_platform_release`] 相同。
/// - 测试版：另外接受 tag 恰为 `<platform>-vX.Y.Z-beta` 的预发布；`-alpha` 和其他预发布不算。
/// - 每日构建：再接受滚动的 `nightly` 发布里本平台带版本号的那一项（正文末尾的 `msime-nightly-state` 状态块）。
///
/// `windows7` 只有正式发布，调用方应先按 [`UpdateCheckRequest::effective_channel`] 把它的通道换成稳定版；这里对它也不接受任何预发布。
pub fn select_channel_release(
    releases: &[Value],
    platform: &str,
    edition: Option<&str>,
    arch: Option<&str>,
    channel: UpdateChannel,
) -> Option<ReleaseUpdate> {
    let channel = if platform == "windows7" {
        UpdateChannel::Stable
    } else {
        channel
    };
    let prefix = format!("{platform}-");
    let mut newest: Option<ReleaseUpdate> = None;
    let mut consider = |update: ReleaseUpdate| {
        if newest
            .as_ref()
            .is_none_or(|newest| build_key_cmp(&update, newest).is_gt())
        {
            newest = Some(update);
        }
    };
    for release in releases {
        if release.get("draft") == Some(&Value::Bool(true)) {
            continue;
        }
        if is_nightly_release(release) {
            if channel == UpdateChannel::Nightly {
                if let Some(update) = nightly_release(release, platform, edition, arch) {
                    consider(update);
                }
            }
            continue;
        }
        let Some(tag) = release
            .get("tag_name")
            .and_then(Value::as_str)
            .and_then(|tag| tag.strip_prefix(&prefix))
        else {
            continue;
        };
        let release_channel = if release.get("prerelease") != Some(&Value::Bool(true)) {
            UpdateChannel::Stable
        } else if channel != UpdateChannel::Stable && is_beta_tag(tag) {
            UpdateChannel::Beta
        } else {
            continue;
        };
        let Some(html_url) = release.get("html_url").and_then(Value::as_str) else {
            continue;
        };
        let Some(mut update) = validate_release(tag, html_url) else {
            continue;
        };
        update.channel = release_channel;
        update.build_run = build_marker_run(release.get("body"));
        let assets = release.get("assets").and_then(Value::as_array);
        match platform {
            "linux" | "windows" => {
                let installer = platform_installer(platform, assets, edition, arch);
                update.installer_name = installer.as_ref().map(|asset| asset.name.clone());
                update.installer_sha256 = installer.and_then(|asset| asset.sha256);
                update.signed = Some(false);
            }
            "windows7" => {
                if tag != format!("v{}", update.version.display)
                    || html_url != format!("{RELEASES_PAGE_URL}/tag/windows7-{tag}")
                {
                    continue;
                }
                let architecture = match arch {
                    Some("x86_64" | "x64") => "x64",
                    Some("x86" | "i686") => "x86",
                    _ => continue,
                };
                // 专用发布必须包含本版本唯一的兼容安装包；缺包时继续找较早的可安装发布。
                let installer = edition_installer_prefix(edition).and_then(|prefix| {
                    let expected = format!(
                        "{}{}_win7_{architecture}.exe",
                        prefix, update.version.display
                    );
                    select_unique_release_asset(
                        assets.map(|assets| assets.iter().collect()),
                        &[&|name: &str| name.eq_ignore_ascii_case(&expected)],
                    )
                });
                let Some(installer) = installer else {
                    continue;
                };
                update.installer_name = Some(installer.name);
                update.installer_sha256 = installer.sha256;
                update.signed = Some(false);
            }
            _ => {}
        }
        consider(update);
    }
    newest
}

/// 两个候选谁更新：先比版本号，再比运行 ID（没有的按 0）。
fn build_key_cmp(left: &ReleaseUpdate, right: &ReleaseUpdate) -> Ordering {
    compare_versions(&left.version, &right.version).then_with(|| {
        left.build_run
            .unwrap_or(0)
            .cmp(&right.build_run.unwrap_or(0))
    })
}

/// Linux 和 Windows 发布里本版本（Linux 上还限定本架构）唯一的安装包。两边的发布 workflow 发的都是未签名安装包：Linux 的产物都没有签名（`.deb` 没有，也没有分离的 GPG 签名），Windows 的签名是本地手动完成的步骤。
fn platform_installer(
    platform: &str,
    assets: Option<&Vec<Value>>,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<ReleaseAsset> {
    match platform {
        "linux" => linux_package(assets, edition, arch),
        "windows" => edition_installer_prefix(edition).and_then(|prefix| {
            select_unique_release_asset(
                assets.map(|assets| assets.iter().collect()),
                &[&|name: &str| windows_installer_matches(name, &prefix)],
            )
        }),
        _ => None,
    }
}

/// tag 恰为 `vX.Y.Z-beta`（去掉平台前缀以后）：develop 发出的测试版。
fn is_beta_tag(tag: &str) -> bool {
    tag.strip_suffix("-beta")
        .and_then(|version| version.strip_prefix('v'))
        .is_some_and(|version| {
            parse_version(version).is_some_and(|parsed| parsed.display == version)
        })
}

fn is_nightly_release(release: &Value) -> bool {
    release.get("tag_name").and_then(Value::as_str) == Some(NIGHTLY_TAG)
}

/// 发布正文里 `msime-build` 标记的运行 ID；正文最后一行不是标记、标记不是 JSON 或 `run` 不是正整数时为 `None`。
///
/// 只认最后一行：`release-*.yml` 把标记追加在 `release-notes.md` 最末尾，而它前面的更新说明是 `generate-release-notes.py` 拿 PR 标题、正文和提交说明让模型写的，外部贡献者能控制其中的文字。正文中间出现的同名标记（PR 描述里的示例、故意写的大运行 ID）不能算数，否则一个写坏的会让这个发布没有运行 ID，一个写大的会让装了这个版本的每个用户一直被提醒同版本号的「新版本」。
fn build_marker_run(body: Option<&Value>) -> Option<u64> {
    let last_line = body?.as_str()?.trim_end().lines().next_back()?;
    let payload = last_line.strip_prefix(BUILD_MARKER)?.strip_suffix(" -->")?;
    let marker: Value = serde_json::from_str(payload).ok()?;
    marker.get("run")?.as_u64().filter(|run| *run > 0)
}

/// 滚动 nightly 发布里 `platform` 的那一项：版本号取状态里本平台的 `version`（没有它的旧条目不提供），运行 ID 取 `run`，附件和摘要取状态里的 `files`（必须也在发布的附件里，GitHub 给的摘要与状态不符的不要）。
fn nightly_release(
    release: &Value,
    platform: &str,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<ReleaseUpdate> {
    let html_url = release.get("html_url")?.as_str()?;
    if html_url != format!("{RELEASES_PAGE_URL}/tag/{NIGHTLY_TAG}") {
        return None;
    }
    let state = nightly_state(release.get("body")?.as_str()?)?;
    if state.get("version").and_then(Value::as_u64) != Some(1) {
        return None;
    }
    let build = state.get("platforms")?.get(platform)?;
    let version_text = build.get("version")?.as_str()?;
    let version = parse_version(version_text).filter(|version| version.display == version_text)?;
    let build_run = build
        .get("run")
        .and_then(Value::as_u64)
        .filter(|run| *run > 0);
    let mut update = ReleaseUpdate {
        version,
        release_url: html_url.to_owned(),
        installer_name: None,
        installer_sha256: None,
        signed: None,
        channel: UpdateChannel::Nightly,
        build_run,
    };
    if matches!(platform, "linux" | "windows") {
        // 附件名在扩展名前多了 `-nightly.<run>.<attempt>`；去掉它再交给正式版的匹配规则，选中以后换回真实的附件名。
        let published = release.get("assets").and_then(Value::as_array);
        let mut canonical_names = Vec::new();
        let mut synthetic = Vec::new();
        for file in build
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (Some(name), Some(sha256)) = (
                file.get("name").and_then(Value::as_str),
                file.get("sha256").and_then(Value::as_str),
            ) else {
                continue;
            };
            if !is_lowercase_sha256(sha256) || !name.chars().all(is_installer_char) {
                continue;
            }
            let Some(asset) = published
                .into_iter()
                .flatten()
                .find(|asset| asset_name(asset) == Some(name))
            else {
                continue;
            };
            if asset
                .get("digest")
                .and_then(Value::as_str)
                .is_some_and(|digest| digest != format!("sha256:{sha256}"))
            {
                continue;
            }
            let canonical = strip_nightly_infix(name).unwrap_or_else(|| name.to_owned());
            synthetic.push(serde_json::json!({
                "name": canonical,
                "digest": format!("sha256:{sha256}"),
            }));
            canonical_names.push((canonical, name.to_owned()));
        }
        let installer = platform_installer(platform, Some(&synthetic), edition, arch);
        update.installer_name = installer.as_ref().and_then(|asset| {
            canonical_names
                .iter()
                .find(|(canonical, _)| *canonical == asset.name)
                .map(|(_, actual)| actual.clone())
        });
        update.installer_sha256 = installer.and_then(|asset| asset.sha256);
        update.signed = Some(false);
    }
    Some(update)
}

/// nightly 正文末尾的状态 JSON。
fn nightly_state(body: &str) -> Option<Value> {
    let start = body.find(NIGHTLY_STATE_MARKER)? + NIGHTLY_STATE_MARKER.len();
    let end = start + body[start..].find("\n-->")?;
    serde_json::from_str(&body[start..end]).ok()
}

/// `<stem>-nightly.<run>.<attempt>.<ext>` → `<stem>.<ext>`（`scripts/publish-nightly.py` 的命名，`ext` 是最后一段扩展名）；不是这个形状时为 `None`。
fn strip_nightly_infix(name: &str) -> Option<String> {
    let (stem, rest) = name.rsplit_once("-nightly.")?;
    let (run, rest) = rest.split_once('.')?;
    let (attempt, extension) = rest.split_once('.')?;
    let digits = |value: &str| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit());
    (!stem.is_empty()
        && digits(run)
        && digits(attempt)
        && !extension.is_empty()
        && extension.bytes().all(|byte| byte.is_ascii_alphanumeric()))
    .then(|| format!("{stem}.{extension}"))
}

fn is_lowercase_sha256(hex: &str) -> bool {
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
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
        channel: UpdateChannel::Stable,
        build_run: None,
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
    is_lowercase_sha256(hex).then(|| hex.to_owned())
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

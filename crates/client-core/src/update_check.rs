//! Checks the shared repository's release list for a newer build of this platform and edition.
//!
//! Every platform publishes to `metasequoiaime/msime` under its own tag prefix (`windows-v1.2.0`, `linux-v1.2.0`; see `.github/workflows/release-*.yml`), so the repository's single "latest" release usually belongs to another platform, and a prefixed tag is not a version. [`select_platform_release`] reads the whole list instead and keeps the newest published release of one platform. The editions share each platform's tag and are told apart by asset name, so on Linux and Windows it also picks the one package of this edition (and, on Linux, this architecture) whose name and SHA-256 the settings page may show.
//!
//! The settings surfaces of every host call [`check_for_update`]: the Tauri shell through its `update_check` command, the native Windows settings window and the HarmonyOS page through `msime_client_update_check`. What to say about the result (the unsigned warning, the copyable hash command, the mirror link) stays with each surface.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::time::Duration;

/// The repository's release list, newest first; one page holds every platform's recent releases.
pub const RELEASES_URL: &str =
    "https://api.github.com/repos/metasequoiaime/msime/releases?per_page=100";
/// Where every release page of the repository lives; a release URL outside it is not offered.
pub const RELEASES_PAGE_URL: &str = "https://github.com/metasequoiaime/msime/releases";
/// How long a check waits for GitHub before reporting it unavailable.
pub const TIMEOUT: Duration = Duration::from_secs(10);
/// A page of 100 releases with their notes and assets is a few megabytes.
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;
/// GitHub's REST API refuses requests without a User-Agent.
const USER_AGENT: &str = "msime-client";

/// A release version as the settings pages show and compare it: the dotted numbers only, without a leading `v` or any pre-release or build suffix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Version {
    pub display: String,
    pub parts: Vec<u64>,
}

/// The newest release of a platform, with the edition's package when one could be chosen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ReleaseUpdate {
    pub version: Version,
    /// The release's page under [`RELEASES_PAGE_URL`].
    pub release_url: String,
    /// The single package of this edition (and, on Linux, this architecture), or `None` when the release has none or more than one.
    pub installer_name: Option<String>,
    /// The package's SHA-256 as GitHub computed it, lowercase hex, when GitHub reported one.
    pub installer_sha256: Option<String>,
    /// `Some(false)` on Linux and Windows, whose release workflows publish unsigned packages; `None` where nothing is known.
    pub signed: Option<bool>,
}

/// What a check found, as hosts serialise it: `{"status": "available" | "current" | "none", "update": ReleaseUpdate | null}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", content = "update", rename_all = "lowercase")]
pub enum UpdateCheck {
    /// A newer release than the running version.
    Available(ReleaseUpdate),
    /// The newest release is not newer than the running version.
    Current(ReleaseUpdate),
    /// The platform has no published release yet.
    None,
}

/// What a host asks: its platform's tag prefix, the running version, and optionally its edition id (`full` when absent) and Rust's name for its architecture.
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
    /// The request named no platform or a running version that is not a version.
    #[error("invalid update check request")]
    Invalid,
    /// GitHub could not be reached, answered with an error, or sent something that is not a release list.
    #[error("the release list is unavailable")]
    Unavailable,
}

/// Fetches the release list and compares its newest release of `request.platform` with the running version. Blocks on the network for up to [`TIMEOUT`]: call it off any UI or input thread.
pub fn check_for_update(request: &UpdateCheckRequest) -> Result<UpdateCheck, UpdateCheckError> {
    validate_request(request)?;
    evaluate_update(&fetch_release_list()?, request)
}

/// The decision [`check_for_update`] makes once it has the release list's bytes.
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

/// `v1.2.0`, `1.2`, `1.2.0-beta` or `1.2.0+build` as the dotted numbers they start with; anything else, including surrounding text, is not a version.
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

/// Compares part by part, a missing part counting as zero: `1.2` equals `1.2.0`.
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

/// The newest published release of `platform` in the repository's release list, with the edition's package on Linux and Windows. Drafts and prereleases are not offered.
///
/// `edition` is the running edition's id, `full` when absent; the editions share each platform's tag and only the asset names tell them apart. `arch` is Rust's name for the host's architecture; on Linux only that architecture's package is offered.
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
            // No Linux artifact is signed (neither the .deb nor a detached GPG signature).
            "linux" => {
                let package = linux_package(assets, edition, arch);
                update.installer_name = package.as_ref().map(|asset| asset.name.clone());
                update.installer_sha256 = package.and_then(|asset| asset.sha256);
                update.signed = Some(false);
            }
            // The release workflow publishes the installer unsigned; signing is a local, manual step.
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

/// A release whose page is under [`RELEASES_PAGE_URL`] and whose tag, with the platform prefix removed, is a version.
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

/// A release asset chosen for the settings page.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReleaseAsset {
    name: String,
    sha256: Option<String>,
}

type AssetPattern<'a> = &'a dyn Fn(&str) -> bool;

/// The one asset the first pattern with any match selects. A pattern matching two assets selects nothing, and the later patterns are not tried: two candidates would make any single digest wrong for someone.
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

/// GitHub's `sha256:<lowercase hex>` asset digest. Older API responses omit it or send `null`; any other algorithm or form is not trusted.
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

/// An edition id as it may be spliced into an asset name.
fn is_edition_id(id: &str) -> bool {
    id.bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}

/// The edition's Windows installer name prefix, matched without regard to case: full (also when no edition is given) is `MetasequoiaIME-Full_Setup_v`, the Wubi edition `MetasequoiaIME-Wubi_Setup_v`. `None` for anything that is not an edition id. `scripts/test-editions.py` derives the same names.
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

/// `<prefix>` + one or more of `[A-Za-z0-9_.-]` + `.exe`, ignoring case. The name is shown inside a shell command the user may copy, so it is limited to characters that need no quoting.
fn windows_installer_matches(name: &str, prefix: &str) -> bool {
    strip_prefix_ignore_case(name, prefix)
        .and_then(|rest| strip_suffix_ignore_case(rest, ".exe"))
        .is_some_and(|middle| !middle.is_empty() && middle.chars().all(is_installer_char))
}

fn is_installer_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// What CPack puts in Linux package names besides the name itself (`platforms/linux/cmake/packaging.cmake`).
fn is_package_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '+' | '~' | '-')
}

/// The edition's Linux package on this host: the `.deb` if there is exactly one, else the `.tar.gz`.
fn linux_package(
    assets: Option<&Vec<Value>>,
    edition: Option<&str>,
    arch: Option<&str>,
) -> Option<ReleaseAsset> {
    let mut assets: Vec<&Value> = assets?.iter().collect();
    let full = is_full_edition(edition);
    if full {
        // full's patterns also match the other editions' packages (`msime-linux-wubi_…`); full's names follow `msime-linux` with `_` or the version.
        assets.retain(|asset| !asset_name(asset).is_some_and(is_other_edition_linux_package));
    }
    if let Some((deb, tarball)) = arch.and_then(linux_package_architecture) {
        // A release carries one `.deb` and one `.tar.gz` per architecture; keep this machine's. An unknown architecture keeps them all.
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

/// dpkg's and CMake's names for the host's architecture in the package names, keyed by Rust's name for it.
fn linux_package_architecture(arch: &str) -> Option<(&'static str, &'static str)> {
    match arch {
        "x86_64" => Some(("amd64", "x86_64")),
        "aarch64" => Some(("arm64", "aarch64")),
        _ => None,
    }
}

/// `msime-linux-<letter>…`: another edition's package.
fn is_other_edition_linux_package(name: &str) -> bool {
    strip_prefix_ignore_case(name, "msime-linux-")
        .and_then(|rest| rest.chars().next())
        .is_some_and(|c| c.is_ascii_alphabetic())
}

/// Any package name that needs no quoting and does not start like an option: `[A-Za-z0-9]` then `[\w.+~-]*`, then `suffix`.
fn full_linux_package_matches(name: &str, suffix: &str) -> bool {
    strip_suffix_ignore_case(name, suffix).is_some_and(|stem| {
        let mut characters = stem.chars();
        characters.next().is_some_and(|c| c.is_ascii_alphanumeric())
            && characters.all(is_package_char)
    })
}

/// `msime-linux-<id>_` + one or more package characters + `.deb`.
fn edition_deb_matches(name: &str, id: &str) -> bool {
    strip_prefix_ignore_case(name, &format!("msime-linux-{id}_"))
        .and_then(|rest| strip_suffix_ignore_case(rest, ".deb"))
        .is_some_and(|middle| !middle.is_empty() && middle.chars().all(is_package_char))
}

/// `msime-linux-<id>-` + a digit + package characters + `.tar.gz`.
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

//! Sharing installed plugin packs through the community library.
//!
//! A shared pack travels as one zip archive of the pack's files at its top level, base64-encoded in the request and the download. [`pack`] builds that archive from a pack installed under the plugins root, deterministically, and checks it by the same rules a local `.zip` import is held to; [`install`] verifies a download against the size and SHA-256 the server sent and installs it through [`super::import`], so a community pack and a pack the user picked from disk pass exactly the same checks. The transport and the service mirror the candidate-skin community (`crate::skin::candidate_community`): every rule of the server (`internal/account/community_plugin.go`) the client can check is applied before a request leaves, because the transport only reports an HTTP status.

use super::{
    import, is_builtin, list_files, load_package, read_file, validate, PluginError, PluginFailure,
    PluginKind, PluginSummary,
};
use crate::account::{
    request_with_account_session, AccountApi, AccountError, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession,
};
use crate::cloud::dictionary::percent_encode;
use crate::community::{
    valid_author, valid_description, valid_name, valid_query, valid_rating, CommunityModeration,
    MAXIMUM_JAVASCRIPT_INTEGER, MAXIMUM_PAGE_ITEMS, MODERATION_FIELDS,
};
use crate::skin::catalog::safe_id;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// The kinds the library accepts. An effect pack is a few parameters for a style built into every host, so there is nothing in it worth sharing as a file.
pub const PUBLISHABLE_KINDS: [PluginKind; 7] = [
    PluginKind::Sound,
    PluginKind::Music,
    PluginKind::CommandTable,
    PluginKind::PhraseTable,
    PluginKind::Helpcode,
    PluginKind::Wordbook,
    PluginKind::SymbolSet,
];
/// 向服务端声明本客户端能安装的新插件类型（`sound`、`music`、`command_table`、`effect` 之外的）。服务端只把冻结的旧类型集合与这里声明的类型返回给列表和详情请求，因此不带声明的已发布客户端永远不会收到它不认识的类型。每支持一种新类型就把它追加进来；`kinds_declaration_lists_the_new_publishable_kinds` 测试保证它与 [`PUBLISHABLE_KINDS`] 一致。
pub(crate) const KINDS_DECLARATION: &str = "kinds=phrase_table,helpcode,wordbook,symbol_set";
/// Largest archive the server stores for one pack.
pub const MAX_COMMUNITY_ARCHIVE_BYTES: usize = 8 * 1024 * 1024;
/// Largest offset the server pages to.
pub const MAX_COMMUNITY_OFFSET: usize = 100_000;
/// Largest search the server takes, in bytes.
pub const MAX_COMMUNITY_SEARCH_BYTES: usize = 128;
/// Largest publish request body: the archive in base64 plus the listing text with JSON escaping, the keys and the metadata.
pub const MAX_PUBLISH_BODY_BYTES: usize = 11_300_000;
/// Largest download response: the archive in base64 plus its metadata.
pub const MAX_DOWNLOAD_RESPONSE_BYTES: usize = 11_300_000;

const MAX_PUBLISH_RESPONSE_BYTES: usize = 64 * 1024;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(90);
/// The name the archive is written under before it is validated or imported; `import` tells an archive from a folder by this extension.
const ARCHIVE_FILE: &str = "pack.zip";

const KIND: &str = "plugin_community_kind";
const TOO_LARGE: &str = "plugin_community_too_large";
const CHECKSUM: &str = "plugin_community_checksum";
const MISMATCH: &str = "plugin_community_mismatch";
const STORAGE: &str = "storage";

/// One published pack as the gallery lists it. It never carries the archive.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityPlugin {
    pub id: Uuid,
    pub kind: PluginKind,
    /// The manifest id, which is also the folder the pack installs into.
    pub plugin_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub license: String,
    /// Bytes of the archive.
    pub size: u64,
    /// Hex SHA-256 of the archive.
    pub sha256: String,
    pub downloads: u64,
    pub rating_count: u64,
    pub rating_average: f64,
    pub owned: bool,
    pub my_rating: u8,
    pub created_at: String,
    /// The moderation state, sent only for the signed-in user's own item and only to a request that asked for it with `fields=moderation`; other users' items and older servers leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moderation: Option<CommunityModeration>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityPluginPage {
    pub plugins: Vec<CommunityPlugin>,
    pub has_more: bool,
    /// How many items of a kind this client cannot install (an effect pack) the server listed on this page and [`validate_page`] left out. The gallery adds them to the next offset so 加载更多 resumes after them. Only the client sets it: a server response that carries it is refused.
    #[serde(default, skip_deserializing)]
    pub skipped: usize,
}

/// 列表响应的线格式：先逐个元素按 JSON 读进来，再由 [`decode_page`] 跳过本客户端不认识的类型，所以一种新类型不会让整页读不出来。页本身仍然拒绝未知字段。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunityPluginPageWire {
    plugins: Vec<serde_json::Value>,
    has_more: bool,
}

/// 把线格式解成 [`CommunityPluginPage`]：`kind` 是本客户端不认识的字符串的元素计入 `skipped`；其余元素按 [`CommunityPlugin`] 严格解析，已知类型的条目多一个字段或类型不对仍让整页失败。
fn decode_page(wire: CommunityPluginPageWire) -> Result<CommunityPluginPage, AccountError> {
    let mut plugins = Vec::with_capacity(wire.plugins.len());
    let mut skipped = 0;
    for value in wire.plugins {
        if value
            .get("kind")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|kind| PluginKind::parse(kind).is_none())
        {
            skipped += 1;
            continue;
        }
        plugins.push(
            serde_json::from_value::<CommunityPlugin>(value)
                .map_err(|_| AccountError::Unavailable)?,
        );
    }
    Ok(CommunityPluginPage {
        plugins,
        has_more: wire.has_more,
        skipped,
    })
}

/// A downloaded pack: the archive in standard base64 with the size and SHA-256 the server recorded for it. [`install`] checks both before anything is written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityPluginDownload {
    pub id: Uuid,
    pub kind: PluginKind,
    pub plugin_id: String,
    pub version: String,
    pub size: u64,
    pub sha256: String,
    pub archive: String,
}

/// The publish request body. `id` is the client's publication UUID, which the server uses as the idempotency key: a retry with the same id and content returns the same item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CommunityPluginPublishRequest {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub kind: PluginKind,
    pub plugin_id: String,
    pub version: String,
    pub archive: String,
}

impl CommunityPluginPublishRequest {
    /// The request that publishes `packed` under the listing `name` and `description`.
    pub fn new(id: Uuid, name: String, description: String, packed: &PackedPlugin) -> Self {
        Self {
            id,
            name,
            description,
            kind: packed.kind,
            plugin_id: packed.plugin_id.clone(),
            version: packed.version.clone(),
            archive: BASE64.encode(&packed.archive),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunityPluginRating {
    stars: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunityPluginDeleteResponse {
    deleted: bool,
}

pub trait CommunityPluginApi: Send + Sync + 'static {
    /// One page of published packs; `mine` lists only the signed-in user's own, removed ones included, with their moderation state.
    fn community_plugins(
        &self,
        offset: usize,
        search: &str,
        kind: Option<PluginKind>,
        mine: bool,
        token: Option<&str>,
    ) -> Result<CommunityPluginPage, AccountError>;
    fn community_plugin(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CommunityPlugin, AccountError>;
    fn publish_community_plugin(
        &self,
        request: &CommunityPluginPublishRequest,
        token: &str,
    ) -> Result<CommunityPlugin, AccountError>;
    fn download_community_plugin(
        &self,
        id: Uuid,
        token: &str,
    ) -> Result<CommunityPluginDownload, AccountError>;
    fn rate_community_plugin(&self, id: Uuid, stars: u8, token: &str) -> Result<(), AccountError>;
    fn delete_community_plugin(&self, id: Uuid, token: &str) -> Result<(), AccountError>;
}

impl CommunityPluginApi for BackendAccountClient {
    fn community_plugins(
        &self,
        offset: usize,
        search: &str,
        kind: Option<PluginKind>,
        mine: bool,
        token: Option<&str>,
    ) -> Result<CommunityPluginPage, AccountError> {
        validate_query(offset, search, kind)?;
        if mine && token.is_none() {
            return Err(AccountError::Unauthorized);
        }
        let kind = kind
            .map(|kind| format!("&kind={}", kind.as_str()))
            .unwrap_or_default();
        let scope = if mine {
            format!("&scope=mine&{MODERATION_FIELDS}")
        } else {
            String::new()
        };
        let path = format!(
            "/v1/community/plugins?offset={offset}&q={}{kind}{scope}&{KINDS_DECLARATION}",
            percent_encode(search)
        );
        let wire = self.json::<CommunityPluginPageWire, ()>(Method::GET, &path, token, None)?;
        validate_page(decode_page(wire)?)
    }

    fn community_plugin(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CommunityPlugin, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!(
            "/v1/community/plugins/{}?{MODERATION_FIELDS}&{KINDS_DECLARATION}",
            id.hyphenated()
        );
        let item = self.json::<CommunityPlugin, ()>(Method::GET, &path, token, None)?;
        validate_item(&item)?;
        if item.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn publish_community_plugin(
        &self,
        request: &CommunityPluginPublishRequest,
        token: &str,
    ) -> Result<CommunityPlugin, AccountError> {
        validate_publish(request)?;
        let item = self.json_with_limits_timeout::<CommunityPlugin, _>(
            Method::POST,
            "/v1/community/plugins",
            Some(token),
            Some(request),
            MAX_PUBLISH_BODY_BYTES,
            MAX_PUBLISH_RESPONSE_BYTES,
            TRANSFER_TIMEOUT,
        )?;
        validate_item(&item)?;
        if item.id != request.id || item.kind != request.kind || item.plugin_id != request.plugin_id
        {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn download_community_plugin(
        &self,
        id: Uuid,
        token: &str,
    ) -> Result<CommunityPluginDownload, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct DownloadRequest {}
        let path = format!("/v1/community/plugins/{}/download", id.hyphenated());
        let download = self.json_with_limit_timeout::<CommunityPluginDownload, _>(
            Method::POST,
            &path,
            Some(token),
            Some(&DownloadRequest {}),
            MAX_DOWNLOAD_RESPONSE_BYTES,
            TRANSFER_TIMEOUT,
        )?;
        validate_download(&download)?;
        if download.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(download)
    }

    fn rate_community_plugin(&self, id: Uuid, stars: u8, token: &str) -> Result<(), AccountError> {
        if id.is_nil() || !(1..=5).contains(&stars) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct RatingRequest {
            stars: u8,
        }
        let path = format!("/v1/community/plugins/{}/rating", id.hyphenated());
        let result = self.json::<CommunityPluginRating, _>(
            Method::PUT,
            &path,
            Some(token),
            Some(&RatingRequest { stars }),
        )?;
        if result.stars != stars {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }

    fn delete_community_plugin(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/community/plugins/{}", id.hyphenated());
        let result = self.json::<CommunityPluginDeleteResponse, ()>(
            Method::DELETE,
            &path,
            Some(token),
            None,
        )?;
        if !result.deleted {
            return Err(AccountError::Unavailable);
        }
        Ok(())
    }
}

pub struct BackendCommunityPluginService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendCommunityPluginService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendCommunityPluginService<A, S>
where
    A: AccountApi + CommunityPluginApi,
    S: AccountSessionStorage,
{
    /// One page of published packs, newest first, of one kind or of every kind. Signed in, each item also says whether it is the user's own and how the user rated it. `mine` lists only the user's own, removed ones included, and so requires a session.
    pub fn list(
        &self,
        offset: usize,
        search: &str,
        kind: Option<PluginKind>,
        mine: bool,
    ) -> Result<CommunityPluginPage, AccountError> {
        validate_query(offset, search, kind)?;
        request_with_account_session(&self.api, &self.session, mine, |api, token| {
            api.community_plugins(offset, search, kind, mine, token)
        })
    }

    pub fn detail(&self, id: Uuid) -> Result<CommunityPlugin, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, false, |api, token| {
            api.community_plugin(id, token)
        })
    }

    pub fn publish(
        &self,
        request: &CommunityPluginPublishRequest,
    ) -> Result<CommunityPlugin, AccountError> {
        validate_publish(request)?;
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.publish_community_plugin(request, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// Download a pack, which the server counts once per user. The echoed id is checked, so the archive returned is the one asked for.
    pub fn download(&self, id: Uuid) -> Result<CommunityPluginDownload, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.download_community_plugin(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn rate(&self, id: Uuid, stars: u8) -> Result<(), AccountError> {
        if id.is_nil() || !(1..=5).contains(&stars) {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.rate_community_plugin(id, stars, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn delete(&self, id: Uuid) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.delete_community_plugin(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }
}

fn publishable(kind: PluginKind) -> bool {
    PUBLISHABLE_KINDS.contains(&kind)
}

/// A pack id the library can hold for `kind`: a valid folder name that no built-in pack of that kind uses.
fn valid_plugin_id(kind: PluginKind, id: &str) -> bool {
    safe_id(id) && !is_builtin(kind, id)
}

fn valid_version(version: &str) -> bool {
    !version.trim().is_empty() && crate::text::is_bounded_text(version, 32)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Length of the padded standard base64 encoding of `bytes` bytes.
fn base64_length(bytes: usize) -> usize {
    bytes.div_ceil(3) * 4
}

fn validate_query(
    offset: usize,
    search: &str,
    kind: Option<PluginKind>,
) -> Result<(), AccountError> {
    if !valid_query(offset, search)
        || offset > MAX_COMMUNITY_OFFSET
        || search.len() > MAX_COMMUNITY_SEARCH_BYTES
        || kind.is_some_and(|kind| !publishable(kind))
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_publish(request: &CommunityPluginPublishRequest) -> Result<(), AccountError> {
    if request.id.is_nil()
        || !valid_name(&request.name)
        || !valid_description(&request.description)
        || !publishable(request.kind)
        || !valid_plugin_id(request.kind, &request.plugin_id)
        || !valid_version(&request.version)
        || request.archive.is_empty()
        || request.archive.len() > base64_length(MAX_COMMUNITY_ARCHIVE_BYTES)
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

/// 检查一页列表，去掉本客户端不能安装的类型（特效包）。服务端会列出它接受的每种类型，所以这样的一条不能让整页读不出来；去掉的条目连同 [`decode_page`] 已跳过的未知类型一起计入 `skipped`，让翻页与服务端的 offset 对齐。
fn validate_page(mut page: CommunityPluginPage) -> Result<CommunityPluginPage, AccountError> {
    let listed = page.plugins.len() + page.skipped;
    if listed > MAXIMUM_PAGE_ITEMS || (page.has_more && listed == 0) {
        return Err(AccountError::Unavailable);
    }
    page.plugins.retain(|item| publishable(item.kind));
    page.skipped = listed - page.plugins.len();
    if page.plugins.iter().any(|item| validate_item(item).is_err()) {
        return Err(AccountError::Unavailable);
    }
    let mut ids = BTreeSet::new();
    if page.plugins.iter().any(|item| !ids.insert(item.id)) {
        return Err(AccountError::Unavailable);
    }
    Ok(page)
}

fn validate_item(item: &CommunityPlugin) -> Result<(), AccountError> {
    if item.id.is_nil()
        || !publishable(item.kind)
        || !valid_plugin_id(item.kind, &item.plugin_id)
        || !valid_name(&item.name)
        || !valid_description(&item.description)
        || !valid_author(&item.author)
        || !valid_version(&item.version)
        || item.license.trim().is_empty()
        || !crate::text::is_bounded_text(&item.license, 64)
        || item.size == 0
        || item.size > MAX_COMMUNITY_ARCHIVE_BYTES as u64
        || !is_sha256_hex(&item.sha256)
        || item.downloads > MAXIMUM_JAVASCRIPT_INTEGER
        || !valid_rating(item.rating_count, item.rating_average, item.my_rating)
        || item.created_at.is_empty()
        || !crate::text::is_bounded_text(&item.created_at, 64)
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

/// The cheap shape checks on a download; [`install`] checks the archive itself.
fn validate_download(download: &CommunityPluginDownload) -> Result<(), AccountError> {
    if download.id.is_nil()
        || !publishable(download.kind)
        || !valid_plugin_id(download.kind, &download.plugin_id)
        || !valid_version(&download.version)
        || download.size == 0
        || download.size > MAX_COMMUNITY_ARCHIVE_BYTES as u64
        || !is_sha256_hex(&download.sha256)
        || download.archive.is_empty()
        || download.archive.len() > base64_length(MAX_COMMUNITY_ARCHIVE_BYTES)
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

/// A pack ready to publish, built by [`pack`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedPlugin {
    pub kind: PluginKind,
    pub plugin_id: String,
    pub version: String,
    pub license: String,
    /// The zip archive, byte for byte what is uploaded.
    pub archive: Vec<u8>,
    /// Hex SHA-256 of `archive`, which the server records and every download is checked against.
    pub sha256: String,
    pub file_count: usize,
    /// The manifest name cut to the 32 characters a listing name allows, or the pack id when that is not a valid listing name.
    pub suggested_name: String,
    /// The manifest description cut to the 280 characters a listing description allows, or `""`.
    pub suggested_description: String,
}

/// Build the archive that publishes the pack `id` of `kind` installed under the plugins root `root`.
///
/// The pack must load by the rules `scan` lists it by and must not be built in. Its files - every one `scan` reads, which leaves out hidden names such as `.DS_Store` - go into the archive at its top level in byte order of their names, each with a fixed timestamp and permissions, so the same pack always yields the same bytes and a retried publish carries identical content. The archive is then checked by [`validate`], exactly as a local `.zip` import would check it, so a pack that installs here also installs on every other client.
pub fn pack(root: &Path, kind: PluginKind, id: &str) -> Result<PackedPlugin, PluginFailure> {
    if !publishable(kind) {
        return Err(PluginFailure::code(KIND));
    }
    if is_builtin(kind, id) {
        return Err(PluginError::Reserved.into());
    }
    let summary = load_package(root, None, kind, id).map_err(PluginError::Invalid)?;
    let files = list_files(&summary.directory).map_err(PluginError::Invalid)?;
    // The pack rules allow ASCII letters of either case, and the server stores archive members case-insensitively, so two names that differ only by case cannot both be shared.
    let mut lowercase = BTreeSet::new();
    if files
        .keys()
        .any(|name| !lowercase.insert(name.to_ascii_lowercase()))
    {
        return Err(PluginError::Invalid("有文件名只差大小写".into()).into());
    }
    let archive = write_archive(&summary.directory, files.keys())?;
    let staging = tempfile::Builder::new()
        .prefix("msime-share-")
        .tempdir()
        .map_err(|_| PluginFailure::code(STORAGE))?;
    let path = staging.path().join(ARCHIVE_FILE);
    fs::write(&path, &archive).map_err(|_| PluginFailure::code(STORAGE))?;
    let checked = validate(&path)?;
    if checked.kind() != kind || checked.id != summary.id || checked.version != summary.version {
        return Err(PluginFailure::code(MISMATCH));
    }
    let suggested: String = summary.name.chars().take(32).collect();
    let suggested = suggested.trim();
    let suggested_name = if valid_name(suggested) {
        suggested.to_owned()
    } else {
        summary.id.chars().take(32).collect()
    };
    let description: String = summary
        .description
        .as_deref()
        .unwrap_or_default()
        .chars()
        .take(280)
        .collect();
    let description = description.trim();
    let suggested_description = if valid_description(description) {
        description.to_owned()
    } else {
        String::new()
    };
    Ok(PackedPlugin {
        kind,
        plugin_id: summary.id,
        version: summary.version,
        license: summary.license,
        sha256: hex::encode(Sha256::digest(&archive)),
        archive,
        file_count: files.len(),
        suggested_name,
        suggested_description,
    })
}

/// The deterministic zip of `names` read from `directory`, refused once it grows past what the server stores.
fn write_archive<'a>(
    directory: &Path,
    names: impl Iterator<Item = &'a String>,
) -> Result<Vec<u8>, PluginFailure> {
    let storage = |_| PluginFailure::code(STORAGE);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644);
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in names {
        // Bounded by the largest file any kind allows; `load_package` has already held each file to its own kind's limit.
        let bytes = read_file(directory, name, super::music_pack::MAX_TRACK_BYTES)
            .map_err(|_| PluginError::Invalid(format!("{name} 无法读取")))?;
        writer.start_file(name.as_str(), options).map_err(storage)?;
        writer
            .write_all(&bytes)
            .map_err(|_| PluginFailure::code(STORAGE))?;
        if writer
            .get_ref()
            .is_some_and(|cursor| cursor.get_ref().len() > MAX_COMMUNITY_ARCHIVE_BYTES)
        {
            return Err(PluginFailure::code(TOO_LARGE));
        }
    }
    let archive = writer.finish().map_err(storage)?.into_inner();
    if archive.len() > MAX_COMMUNITY_ARCHIVE_BYTES {
        return Err(PluginFailure::code(TOO_LARGE));
    }
    Ok(archive)
}

/// Install a downloaded pack under the plugins root `root` and return it as `scan` now lists it.
///
/// `kind` and `plugin_id` are what the listing the user confirmed named: the page asks before replacing an installed pack of that kind and id, so a download that names any other pack is refused before it is even decoded. The archive is then checked against the size and SHA-256 in the response before anything touches the disk, written to a temporary `.zip` and checked by [`validate`]: a pack whose kind, id or version differs from what the response promised is refused before it can replace an installed pack. Only then does [`super::import`] install it, through the same staging, lock and swap a local `.zip` import uses, replacing an installed pack of that id whole.
pub fn install(
    root: &Path,
    download: &CommunityPluginDownload,
    kind: PluginKind,
    plugin_id: &str,
) -> Result<PluginSummary, PluginFailure> {
    if download.kind != kind
        || download.plugin_id != plugin_id
        || !publishable(download.kind)
        || !valid_plugin_id(download.kind, &download.plugin_id)
    {
        return Err(PluginFailure::code(MISMATCH));
    }
    if download.archive.len() > base64_length(MAX_COMMUNITY_ARCHIVE_BYTES)
        || download.size > MAX_COMMUNITY_ARCHIVE_BYTES as u64
    {
        return Err(PluginFailure::code(TOO_LARGE));
    }
    let archive = BASE64
        .decode(&download.archive)
        .map_err(|_| PluginFailure::code(CHECKSUM))?;
    if archive.len() as u64 != download.size
        || hex::encode(Sha256::digest(&archive)) != download.sha256
    {
        return Err(PluginFailure::code(CHECKSUM));
    }
    let staging = tempfile::Builder::new()
        .prefix("msime-download-")
        .tempdir()
        .map_err(|_| PluginFailure::code(STORAGE))?;
    let path = staging.path().join(ARCHIVE_FILE);
    fs::write(&path, &archive).map_err(|_| PluginFailure::code(STORAGE))?;
    let checked = validate(&path)?;
    if checked.kind() != download.kind
        || checked.id != download.plugin_id
        || checked.version != download.version
    {
        return Err(PluginFailure::code(MISMATCH));
    }
    Ok(import(&path, root)?)
}

#[cfg(test)]
mod tests;

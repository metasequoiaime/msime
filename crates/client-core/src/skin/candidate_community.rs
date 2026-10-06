//! Sharing candidate-window skin packages through the community library.
//!
//! A shared package is an installed skin folder reduced to what the backend accepts: `skin.toml`, sent verbatim, plus the PNG or JPEG images its manifest references (the preview, the decoration image and the background image), base64-encoded. [`pack`] builds that payload from a folder under the host's skin root and [`install`] writes a downloaded one back as a folder the catalog lists. Both mirror every rule of the server (`internal/account/community_candidate.go`) the client can check, because the transport only reports an HTTP status and the page can then name the exact problem.

use super::catalog::{self, SkinLicense, SkinSummary};
pub(crate) use super::catalog::{IMAGE_DIMENSIONS_TOO_LARGE, MAX_IMAGE_SIDE, MAX_PACKAGE_PIXELS};
use super::category::INCLUDE_CATEGORY;
use crate::account::{
    request_with_account_session, AccountApi, AccountError, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession,
};
use crate::cloud::dictionary::percent_encode;
use crate::community::{
    valid_author, valid_description, valid_name, valid_query, valid_rating, CommunityModeration,
    MAXIMUM_JAVASCRIPT_INTEGER, MAXIMUM_PAGE_ITEMS, MODERATION_FIELDS,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

/// Most images one package may carry. The manifest can reference only the preview, the decoration image and the background image, and every file must be referenced.
pub const MAX_PACKAGE_FILES: usize = 3;
/// Largest single image, as uploaded and after the server re-encodes it.
pub const MAX_PACKAGE_FILE_BYTES: usize = 1 << 20;
/// Largest total of all images in one package; `skin.toml` is bounded separately.
pub const MAX_PACKAGE_BYTES: usize = 2 << 20;
/// Largest preview image, so every gallery card loads a small thumbnail.
pub const MAX_PREVIEW_BYTES: usize = 256 * 1024;
/// Largest `skin.toml`, the same bound the catalog reads manifests with.
pub const MAX_MANIFEST_BYTES: usize = 65_536;
/// Largest publish request body the server reads: 2 MiB of images in base64 plus the manifest with JSON escaping, the keys and the metadata.
pub const MAX_PUBLISH_BODY_BYTES: usize = 3_200_000;
/// Largest download response. Go's JSON encoder HTML-escapes `<`, `>` and `&` in the manifest string, which can inflate it up to six times, so this leaves room above the publish bound.
pub const MAX_DOWNLOAD_RESPONSE_BYTES: usize = 4 << 20;
/// Largest preview response: a 256 KiB image in base64 plus its path and type.
pub const MAX_PREVIEW_RESPONSE_BYTES: usize = 512 * 1024;
/// 服务端精选目录给自己保留的候选皮肤 ID（`internal/skins/catalog.go`）。它们现在也都在 `catalog::is_reserved` 里，这里的检查与之重复，留着是为了让这份清单与服务端的对应关系一目了然。
pub const SERVER_BUILTIN_IDS: [&str; 4] = ["fluent", "wechat", "graphite", "willow_green"];

const MAX_PUBLISH_RESPONSE_BYTES: usize = 64 * 1024;
/// Largest sync listing: the per-user library cap of 100 rows at well under 1 KiB each, with room for the cap to grow.
const MAX_SYNC_RESPONSE_BYTES: usize = 512 * 1024;
/// Most rows one sync listing may carry, far above the server's per-user library cap.
const MAX_SYNC_ENTRIES: usize = 1000;
/// The query every list and detail request carries so the server includes `visibility`, `updated_at` and the signed-in user's private packages. Clients released before private packages existed read items with unknown fields refused, so the server sends the new fields only to clients that ask for them.
const SYNC_FIELDS: &str = "fields=sync";
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_RESOURCE_PATH_BYTES: usize = 256;
/// 解码一张图时允许分配的内存上限。每边 2048 的图按 16 位 RGBA 展开是 32 MiB，这里留出一倍给解码器自己的缓冲。尺寸在完整解码前就从文件头读出并按 [`MAX_IMAGE_SIDE`] 拒绝，这道上限是 PNG 解码的第二道防线：1 MiB 以内的文件声明巨大尺寸（解压炸弹）时，解码器也无法因此分配超出它的内存。JPEG 由 `zune-jpeg` 解码，它没有分配上限，由同样的每边上限兜底，最多展开成 2048×2048 的 RGB，即 12 MiB。
const MAX_IMAGE_DECODE_ALLOC: u64 = 64 << 20;
/// 一张渐进式 JPEG 最多的扫描段（SOS）数，与服务端 `maxCandidateJPEGScans` 一致。
const MAX_JPEG_SCANS: usize = 32;
const MANIFEST_FILE: &str = "skin.toml";
/// Where [`install`] writes a package before swapping it in. It sits in the skin root, so the rename that publishes it never crosses a filesystem.
const STAGING_DIRECTORY: &str = ".community-staging";
const PNG_MAGIC: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];

const PACKAGE: &str = "candidate_skin_package";
const FILE_TYPE: &str = "candidate_skin_file_type";
const FILE_PATH: &str = "candidate_skin_file_path";
const TOO_LARGE: &str = "candidate_skin_too_large";
const IMAGE_INVALID: &str = "candidate_skin_image_invalid";
const LICENSE_REQUIRED: &str = "candidate_skin_license_required";
const PREVIEW_REQUIRED: &str = "candidate_skin_preview_required";
const EXISTS: &str = "candidate_skin_exists";
const STORAGE: &str = "storage";

/// License text of a published package, each `""` when the manifest leaves it out.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinLicense {
    pub code: String,
    pub assets: String,
    pub source: String,
}

/// Who can see a package in the library. A private package is listed, previewed and downloadable only by its owner, and needs no asset license.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CandidateSkinVisibility {
    #[default]
    Public,
    Private,
}

/// 社区候选窗口皮肤的发布分类，与社区键盘皮肤共用同一套分类。分类只是发布元数据，不属于 `skin.toml`。
pub use super::category::SkinCategory as CandidateSkinCategory;

/// One published package as the gallery lists it. It never carries the manifest or any image bytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinItem {
    pub id: Uuid,
    /// The manifest id, which is also the folder the package installs into.
    pub package_id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub license: CandidateSkinLicense,
    /// Total bytes of the re-encoded images.
    pub size: u64,
    /// Images in the package; `skin.toml` is not counted.
    pub file_count: u64,
    pub downloads: u64,
    pub rating_count: u64,
    pub rating_average: f64,
    pub owned: bool,
    pub my_rating: u8,
    pub created_at: String,
    /// Absent from a server that predates private packages, which only ever held public ones.
    #[serde(default)]
    pub visibility: CandidateSkinVisibility,
    /// When the content or visibility last changed; `""` from a server that predates it.
    #[serde(default)]
    pub updated_at: String,
    /// [`request_digest`] of the request that last set the content. The server sends it only for the signed-in user's own packages; `""` otherwise.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub request_sha256: String,
    /// The moderation state, sent only for the signed-in user's own package and only to a request that asked for it with `fields=moderation`; other users' packages and older servers leave it out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moderation: Option<CommunityModeration>,
    /// 发布分类。客户端总是带 `include=category` 请求，早于分类功能的服务端不返回它，此时为 `None`。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<CandidateSkinCategory>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinPage {
    pub skins: Vec<CandidateSkinItem>,
    pub has_more: bool,
}

/// A package's preview image. By the time a caller receives one, `data` is standard base64 of a PNG or JPEG whose leading bytes match `content_type`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinPreview {
    pub path: String,
    pub content_type: String,
    pub data: String,
}

/// A downloaded package: the manifest verbatim and each image, keyed by its package-relative path, in standard base64. [`install`] checks it before writing anything.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinPackage {
    pub id: Uuid,
    pub package_id: String,
    pub manifest: String,
    pub files: BTreeMap<String, String>,
}

/// The publish request body. `id` is the client's publication UUID, which the server uses as the idempotency key: a retry with the same id and content returns the same item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateSkinPublishRequest {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub manifest: String,
    pub files: BTreeMap<String, String>,
    pub visibility: CandidateSkinVisibility,
    /// 发布分类；`None` 时不发送该字段，由服务端归入默认分类。分类不计入 [`request_digest`]。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<CandidateSkinCategory>,
}

impl CandidateSkinPublishRequest {
    /// The request that publishes `packed` under the listing `name` and `description`.
    pub fn new(
        id: Uuid,
        name: String,
        description: String,
        packed: PackedSkin,
        visibility: CandidateSkinVisibility,
        category: Option<CandidateSkinCategory>,
    ) -> Self {
        Self {
            id,
            name,
            description,
            manifest: packed.manifest,
            files: packed.files,
            visibility,
            category,
        }
    }
}

/// The body that replaces the content of a package the user already owns. The package id inside `manifest` must stay the same.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CandidateSkinReplaceRequest {
    pub name: String,
    pub description: String,
    pub manifest: String,
    pub files: BTreeMap<String, String>,
}

/// One row of the signed-in user's library as the sync listing returns it. `request_sha256` is [`request_digest`] of the request that last set its content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSkinSyncEntry {
    pub id: Uuid,
    pub package_id: String,
    pub request_sha256: String,
    pub visibility: CandidateSkinVisibility,
    pub updated_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateSkinSyncList {
    skins: Vec<CandidateSkinSyncEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateSkinRating {
    stars: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateSkinUnpublishResponse {
    deleted: bool,
}

pub trait CandidateSkinCommunityApi: Send + Sync + 'static {
    fn candidate_skins(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<CandidateSkinCategory>,
        token: Option<&str>,
    ) -> Result<CandidateSkinPage, AccountError>;
    fn candidate_skin(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn candidate_skin_preview(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CandidateSkinPreview, AccountError>;
    fn publish_candidate_skin(
        &self,
        request: &CandidateSkinPublishRequest,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn download_candidate_skin(
        &self,
        id: Uuid,
        token: &str,
    ) -> Result<CandidateSkinPackage, AccountError>;
    fn rate_candidate_skin(&self, id: Uuid, stars: u8, token: &str) -> Result<(), AccountError>;
    fn unpublish_candidate_skin(&self, id: Uuid, token: &str) -> Result<(), AccountError>;
    fn candidate_skin_sync_list(
        &self,
        token: &str,
    ) -> Result<Vec<CandidateSkinSyncEntry>, AccountError>;
    fn replace_candidate_skin(
        &self,
        id: Uuid,
        request: &CandidateSkinReplaceRequest,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn set_candidate_skin_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn set_candidate_skin_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError>;
}

impl CandidateSkinCommunityApi for BackendAccountClient {
    fn candidate_skins(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<CandidateSkinCategory>,
        token: Option<&str>,
    ) -> Result<CandidateSkinPage, AccountError> {
        validate_query(offset, search)?;
        if mine && token.is_none() {
            return Err(AccountError::Unauthorized);
        }
        // `fields` is repeated rather than comma-joined: a server that reads only the first value still gets the `sync` it requires.
        let scope = if mine {
            format!("&scope=mine&{SYNC_FIELDS}&{MODERATION_FIELDS}")
        } else {
            format!("&{SYNC_FIELDS}")
        };
        let filter = category
            .map(|category| format!("&category={}", category.as_str()))
            .unwrap_or_default();
        let path = format!(
            "/v1/community/candidate-skins?offset={offset}&q={}{scope}{filter}&{INCLUDE_CATEGORY}",
            percent_encode(search)
        );
        let page = self.json::<CandidateSkinPage, ()>(Method::GET, &path, token, None)?;
        validate_page(&page)?;
        Ok(page)
    }

    fn candidate_skin(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CandidateSkinItem, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!(
            "/v1/community/candidate-skins/{}?{SYNC_FIELDS}&{MODERATION_FIELDS}&{INCLUDE_CATEGORY}",
            id.hyphenated()
        );
        let item = self.json::<CandidateSkinItem, ()>(Method::GET, &path, token, None)?;
        validate_item(&item)?;
        if item.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn candidate_skin_preview(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CandidateSkinPreview, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/community/candidate-skins/{}/preview", id.hyphenated());
        let preview = self.json_with_limit::<CandidateSkinPreview, ()>(
            Method::GET,
            &path,
            token,
            None,
            MAX_PREVIEW_RESPONSE_BYTES,
        )?;
        validate_preview(&preview)?;
        Ok(preview)
    }

    fn publish_candidate_skin(
        &self,
        request: &CandidateSkinPublishRequest,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        validate_publish(request)?;
        let item = self.json_with_limits_timeout::<CandidateSkinItem, _>(
            Method::POST,
            &format!("/v1/community/candidate-skins?{INCLUDE_CATEGORY}"),
            Some(token),
            Some(request),
            MAX_PUBLISH_BODY_BYTES,
            MAX_PUBLISH_RESPONSE_BYTES,
            TRANSFER_TIMEOUT,
        )?;
        validate_item(&item)?;
        if item.id != request.id {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn download_candidate_skin(
        &self,
        id: Uuid,
        token: &str,
    ) -> Result<CandidateSkinPackage, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/community/candidate-skins/{}/download", id.hyphenated());
        let package = self.json_with_limit_timeout::<CandidateSkinPackage, ()>(
            Method::POST,
            &path,
            Some(token),
            None,
            MAX_DOWNLOAD_RESPONSE_BYTES,
            TRANSFER_TIMEOUT,
        )?;
        validate_package(&package)?;
        if package.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(package)
    }

    fn rate_candidate_skin(&self, id: Uuid, stars: u8, token: &str) -> Result<(), AccountError> {
        if id.is_nil() || !(1..=5).contains(&stars) {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct RatingRequest {
            stars: u8,
        }
        let path = format!("/v1/community/candidate-skins/{}/rating", id.hyphenated());
        let result = self.json::<CandidateSkinRating, _>(
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

    fn unpublish_candidate_skin(&self, id: Uuid, token: &str) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/community/candidate-skins/{}", id.hyphenated());
        let result = self.json::<CandidateSkinUnpublishResponse, ()>(
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

    fn candidate_skin_sync_list(
        &self,
        token: &str,
    ) -> Result<Vec<CandidateSkinSyncEntry>, AccountError> {
        let list = self.json_with_limit::<CandidateSkinSyncList, ()>(
            Method::GET,
            "/v1/community/candidate-skins/sync",
            Some(token),
            None,
            MAX_SYNC_RESPONSE_BYTES,
        )?;
        validate_sync_list(&list.skins)?;
        Ok(list.skins)
    }

    fn replace_candidate_skin(
        &self,
        id: Uuid,
        request: &CandidateSkinReplaceRequest,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        validate_replace(id, request)?;
        let path = format!(
            "/v1/community/candidate-skins/{}?{INCLUDE_CATEGORY}",
            id.hyphenated()
        );
        let item = self.json_with_limits_timeout::<CandidateSkinItem, _>(
            Method::PUT,
            &path,
            Some(token),
            Some(request),
            MAX_PUBLISH_BODY_BYTES,
            MAX_PUBLISH_RESPONSE_BYTES,
            TRANSFER_TIMEOUT,
        )?;
        validate_item(&item)?;
        if item.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn set_candidate_skin_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct VisibilityRequest {
            visibility: CandidateSkinVisibility,
        }
        let path = format!(
            "/v1/community/candidate-skins/{}?{INCLUDE_CATEGORY}",
            id.hyphenated()
        );
        let item = self.json::<CandidateSkinItem, _>(
            Method::PATCH,
            &path,
            Some(token),
            Some(&VisibilityRequest { visibility }),
        )?;
        validate_item(&item)?;
        if item.id != id || item.visibility != visibility {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn set_candidate_skin_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
        token: &str,
    ) -> Result<CandidateSkinItem, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        #[derive(Serialize)]
        struct CategoryRequest {
            category: CandidateSkinCategory,
        }
        let path = format!(
            "/v1/community/candidate-skins/{}?{INCLUDE_CATEGORY}",
            id.hyphenated()
        );
        let item = self.json::<CandidateSkinItem, _>(
            Method::PATCH,
            &path,
            Some(token),
            Some(&CategoryRequest { category }),
        )?;
        validate_item(&item)?;
        // 回显的分类不一致说明修改没有生效。
        if item.id != id || item.category != Some(category) {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }
}

pub struct BackendCandidateSkinCommunityService<A: AccountApi, S: AccountSessionStorage> {
    api: A,
    session: Arc<BackendAccountSession<A, S>>,
}

impl<A: AccountApi, S: AccountSessionStorage> BackendCandidateSkinCommunityService<A, S> {
    pub fn new(api: A, session: Arc<BackendAccountSession<A, S>>) -> Self {
        Self { api, session }
    }
}

impl<A, S> BackendCandidateSkinCommunityService<A, S>
where
    A: AccountApi + CandidateSkinCommunityApi,
    S: AccountSessionStorage,
{
    pub(crate) fn session_generation(&self) -> Result<Option<u64>, AccountError> {
        if self.session.status()?.is_some() {
            self.session
                .credentials_with_generation(None, None)
                .map(|(_, _, generation)| Some(generation))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn with_session_generation<T, F>(
        &self,
        generation: u64,
        user_id: &str,
        operation: F,
    ) -> Result<T, AccountError>
    where
        F: FnOnce() -> Result<T, AccountError>,
    {
        self.session
            .with_generation(generation, Some(user_id), operation)
    }

    /// One page of published packages, newest first. `mine` lists only the signed-in user's own, and so requires a session. `category` 为 `Some` 时只列出该分类。
    pub fn list(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        category: Option<CandidateSkinCategory>,
    ) -> Result<CandidateSkinPage, AccountError> {
        validate_query(offset, search)?;
        request_with_account_session(&self.api, &self.session, mine, |api, token| {
            api.candidate_skins(offset, search, mine, category, token)
        })
    }

    pub fn detail(&self, id: Uuid) -> Result<CandidateSkinItem, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, false, |api, token| {
            api.candidate_skin(id, token)
        })
    }

    /// The preview of a public package needs no session, but a private one is served to its owner only, so the session goes along when there is one, as for `detail`.
    pub fn preview(&self, id: Uuid) -> Result<CandidateSkinPreview, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, false, |api, token| {
            api.candidate_skin_preview(id, token)
        })
    }

    pub fn publish(
        &self,
        request: &CandidateSkinPublishRequest,
    ) -> Result<CandidateSkinItem, AccountError> {
        validate_publish(request)?;
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.publish_candidate_skin(request, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// Download a package, which the server counts once per user. The echoed id is checked, so the package returned is the one asked for.
    pub fn download(&self, id: Uuid) -> Result<CandidateSkinPackage, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.download_candidate_skin(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn rate(&self, id: Uuid, stars: u8) -> Result<(), AccountError> {
        if id.is_nil() || !(1..=5).contains(&stars) {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.rate_candidate_skin(id, stars, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn unpublish(&self, id: Uuid) -> Result<(), AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.unpublish_candidate_skin(id, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// Every package the signed-in user owns, private ones included, reduced to what sync compares.
    pub fn sync_list(&self) -> Result<Vec<CandidateSkinSyncEntry>, AccountError> {
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.candidate_skin_sync_list(token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn replace(
        &self,
        id: Uuid,
        request: &CandidateSkinReplaceRequest,
    ) -> Result<CandidateSkinItem, AccountError> {
        validate_replace(id, request)?;
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.replace_candidate_skin(id, request, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    pub fn set_visibility(
        &self,
        id: Uuid,
        visibility: CandidateSkinVisibility,
    ) -> Result<CandidateSkinItem, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.set_candidate_skin_visibility(
                id,
                visibility,
                token.ok_or(AccountError::Unauthorized)?,
            )
        })
    }

    /// 修改自己作品的发布分类。
    pub fn set_category(
        &self,
        id: Uuid,
        category: CandidateSkinCategory,
    ) -> Result<CandidateSkinItem, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        request_with_account_session(&self.api, &self.session, true, |api, token| {
            api.set_candidate_skin_category(id, category, token.ok_or(AccountError::Unauthorized)?)
        })
    }

    /// The signed-in user's id, or `None` when signed out.
    pub fn user_id(&self) -> Result<Option<String>, AccountError> {
        self.session.status().map(|user| user.map(|user| user.id))
    }
}

fn validate_query(offset: usize, search: &str) -> Result<(), AccountError> {
    if !valid_query(offset, search) {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_publish(request: &CandidateSkinPublishRequest) -> Result<(), AccountError> {
    if request.id.is_nil() {
        return Err(AccountError::Invalid);
    }
    validate_content(
        &request.name,
        &request.description,
        &request.manifest,
        &request.files,
    )
}

fn validate_replace(id: Uuid, request: &CandidateSkinReplaceRequest) -> Result<(), AccountError> {
    if id.is_nil() {
        return Err(AccountError::Invalid);
    }
    validate_content(
        &request.name,
        &request.description,
        &request.manifest,
        &request.files,
    )
}

fn validate_content(
    name: &str,
    description: &str,
    manifest: &str,
    files: &BTreeMap<String, String>,
) -> Result<(), AccountError> {
    if !valid_name(name)
        || !valid_description(description)
        || manifest.is_empty()
        || manifest.len() > MAX_MANIFEST_BYTES
        || files.is_empty()
        || files.len() > MAX_PACKAGE_FILES
        || files
            .keys()
            .any(|path| !catalog::safe_resource(path, MAX_RESOURCE_PATH_BYTES))
    {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_sync_list(entries: &[CandidateSkinSyncEntry]) -> Result<(), AccountError> {
    let mut ids = HashSet::with_capacity(entries.len());
    if entries.len() > MAX_SYNC_ENTRIES
        || entries.iter().any(|entry| {
            entry.id.is_nil()
                || !ids.insert(entry.id)
                || !valid_package_id(&entry.package_id)
                || !is_sha256_hex(&entry.request_sha256)
                || !crate::text::is_bounded_text(&entry.updated_at, 64)
        })
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn validate_page(page: &CandidateSkinPage) -> Result<(), AccountError> {
    if page.skins.len() > MAXIMUM_PAGE_ITEMS
        || (page.has_more && page.skins.is_empty())
        || page.skins.iter().any(|item| validate_item(item).is_err())
    {
        return Err(AccountError::Unavailable);
    }
    let mut ids = HashSet::with_capacity(page.skins.len());
    if page.skins.iter().any(|item| !ids.insert(item.id)) {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn valid_package_id(id: &str) -> bool {
    catalog::is_external_id(id) && !SERVER_BUILTIN_IDS.contains(&id)
}

fn validate_item(item: &CandidateSkinItem) -> Result<(), AccountError> {
    let license = &item.license;
    if item.id.is_nil()
        || !valid_package_id(&item.package_id)
        || !valid_name(&item.name)
        || !valid_description(&item.description)
        || !valid_author(&item.author)
        || item.version.is_empty()
        || !crate::text::is_bounded_text(&item.version, 32)
        || !crate::text::is_bounded_text(&license.code, 120)
        || (item.visibility == CandidateSkinVisibility::Public && license.assets.trim().is_empty())
        || !crate::text::is_bounded_text(&license.assets, 120)
        || !crate::text::is_bounded_text(&license.source, 500)
        || item.size > MAX_PACKAGE_BYTES as u64
        || item.file_count > MAX_PACKAGE_FILES as u64
        || item.downloads > MAXIMUM_JAVASCRIPT_INTEGER
        || !valid_rating(item.rating_count, item.rating_average, item.my_rating)
        || item.created_at.is_empty()
        || !crate::text::is_bounded_text(&item.created_at, 64)
        || !crate::text::is_bounded_text(&item.updated_at, 64)
        || !(item.request_sha256.is_empty() || is_sha256_hex(&item.request_sha256))
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

fn validate_preview(preview: &CandidateSkinPreview) -> Result<(), AccountError> {
    let content_type = image_content_type(&preview.path);
    if !catalog::safe_resource(&preview.path, MAX_RESOURCE_PATH_BYTES)
        || content_type != Some(preview.content_type.as_str())
        || preview.data.len() > base64_length(MAX_PREVIEW_BYTES)
    {
        return Err(AccountError::Unavailable);
    }
    let bytes = BASE64
        .decode(&preview.data)
        .map_err(|_| AccountError::Unavailable)?;
    if bytes.len() > MAX_PREVIEW_BYTES || !magic_matches(&preview.content_type, &bytes) {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

/// The cheap shape checks on a download; [`install`] checks the contents.
fn validate_package(package: &CandidateSkinPackage) -> Result<(), AccountError> {
    if package.id.is_nil()
        || !valid_package_id(&package.package_id)
        || package.manifest.is_empty()
        || package.manifest.len() > MAX_MANIFEST_BYTES
        || package.files.is_empty()
        || package.files.len() > MAX_PACKAGE_FILES
    {
        return Err(AccountError::Unavailable);
    }
    Ok(())
}

/// The content type a shared image is sent with, by its extension: only PNG and JPEG are accepted.
fn image_content_type(path: &str) -> Option<&'static str> {
    let (_, extension) = path.rsplit_once('.')?;
    match extension.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        _ => None,
    }
}

fn magic_matches(content_type: &str, bytes: &[u8]) -> bool {
    match content_type {
        "image/png" => bytes.starts_with(PNG_MAGIC),
        "image/jpeg" => bytes.starts_with(JPEG_MAGIC),
        _ => false,
    }
}

/// 按 `content_type` 指定的编码完整解码 `bytes`，返回像素数。只看文件头的签名会放过截断或损坏的图，而服务端发布时会完整解码并重新编码，这样的包装进来之后就同步不上去。
///
/// 先只读文件头取尺寸，任一边为 0 或超过 [`MAX_IMAGE_SIDE`] 时返回 `candidate_skin_too_large`（与服务端相同），不做完整解码；解码失败、编码与扩展名不符时返回 `candidate_skin_image_invalid`。
fn check_image(content_type: &str, bytes: &[u8]) -> Result<u64, &'static str> {
    let format = match content_type {
        "image/png" => image::ImageFormat::Png,
        "image/jpeg" => image::ImageFormat::Jpeg,
        _ => return Err(IMAGE_INVALID),
    };
    // 编码由扩展名决定，不按内容猜：名为 .png 的 JPEG 也要拒绝。
    if !magic_matches(content_type, bytes) {
        return Err(IMAGE_INVALID);
    }
    let reader = |bytes| {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_IMAGE_SIDE);
        limits.max_image_height = Some(MAX_IMAGE_SIDE);
        limits.max_alloc = Some(MAX_IMAGE_DECODE_ALLOC);
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        reader.limits(limits);
        reader
    };
    let (width, height) = reader(bytes)
        .into_dimensions()
        .map_err(|error| match error {
            image::ImageError::Limits(_) => TOO_LARGE,
            _ => IMAGE_INVALID,
        })?;
    if width == 0 || height == 0 || width > MAX_IMAGE_SIDE || height > MAX_IMAGE_SIDE {
        return Err(TOO_LARGE);
    }
    if format == image::ImageFormat::Jpeg {
        decode_jpeg_strictly(bytes)?;
    } else {
        reader(bytes).decode().map_err(|_| IMAGE_INVALID)?;
    }
    Ok(u64::from(width) * u64::from(height))
}

/// 用严格模式的 `zune-jpeg` 完整解码一张 JPEG，失败时返回 `candidate_skin_image_invalid`。
///
/// `image` 也用 `zune-jpeg` 解码 JPEG，但固定关掉了严格模式：数据提前结束时用灰色补齐剩下的像素并报告成功，截断在扫描数据中间的 JPEG 因此能“解码成功”，服务端的解码器却会报错。严格模式下这类文件直接解码失败，只缺结尾 EOI 的情况另外检查。每边上限与 [`MAX_IMAGE_SIDE`] 一致；渐进式 JPEG 的扫描段上限与服务端一致，基线 JPEG 每个颜色分量只有一个扫描段，最多 4 个，`zune-jpeg` 不对它计数。
fn decode_jpeg_strictly(bytes: &[u8]) -> Result<(), &'static str> {
    let side = MAX_IMAGE_SIDE as usize;
    let options = zune_jpeg::zune_core::options::DecoderOptions::default()
        .set_strict_mode(true)
        .set_max_width(side)
        .set_max_height(side)
        .jpeg_set_max_scans(MAX_JPEG_SCANS);
    let cursor = zune_jpeg::zune_core::bytestream::ZCursor::new(bytes);
    zune_jpeg::JpegDecoder::new_with_options(cursor, options)
        .decode()
        .map_err(|_| IMAGE_INVALID)?;
    // 严格模式不要求结尾的 EOI：像素数据完整、只缺最后的 `FF D9` 时它照样解码成功，服务端的解码器却会报“意外的文件结尾”。熵编码数据里不会出现 `FF D9` 和 `FF DA`（`0xFF` 后面只跟 `0x00` 或 RSTn），所以最后一个扫描段（`FF DA`）之后必须还有一个 EOI。
    let marker = |code: u8| bytes.windows(2).rposition(|pair| pair == [0xFF, code]);
    match (marker(0xDA), marker(0xD9)) {
        (Some(scan), Some(end)) if end > scan => Ok(()),
        _ => Err(IMAGE_INVALID),
    }
}

/// Length of the padded standard base64 encoding of `bytes` bytes.
fn base64_length(bytes: usize) -> usize {
    bytes.div_ceil(3) * 4
}

/// A package ready to publish, built by [`pack`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedSkin {
    pub package_id: String,
    /// `skin.toml`, verbatim.
    pub manifest: String,
    /// Each referenced image by its package-relative path, in standard base64.
    pub files: BTreeMap<String, String>,
    /// The manifest's license; each part `None` when a private package leaves it out.
    pub license: SkinLicense,
    /// The manifest name cut to the 32 characters a listing name allows, or the package id when that is not a valid listing name.
    pub suggested_name: String,
    /// The manifest description cut to the 280 characters a listing description allows, or `""`.
    pub suggested_description: String,
    /// Total bytes of the images.
    pub size: usize,
    pub file_count: usize,
}

/// The images a package's manifest references: the decoration image (which the catalog already falls back to an image preview for), the background image and the preview. Each must be a safe relative path to a PNG or JPEG, unique even ignoring case, because the server stores keys case-insensitively.
fn referenced_images(summary: &SkinSummary) -> Result<BTreeSet<String>, &'static str> {
    let mut paths = BTreeSet::new();
    for path in [
        summary.decoration_image.as_deref(),
        summary
            .background
            .as_ref()
            .map(|background| background.image.as_str()),
        summary.preview.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if !catalog::safe_resource(path, MAX_RESOURCE_PATH_BYTES) {
            return Err(FILE_PATH);
        }
        if image_content_type(path).is_none() {
            return Err(FILE_TYPE);
        }
        paths.insert(path.to_owned());
    }
    let lowercase: HashSet<String> = paths.iter().map(|path| path.to_ascii_lowercase()).collect();
    if lowercase.len() != paths.len() {
        return Err(FILE_PATH);
    }
    Ok(paths)
}

/// The rules a shared package adds on top of the catalog's: no toolbar stylesheet, and a PNG or JPEG preview. Returns the preview path.
fn shared_preview(summary: &SkinSummary) -> Result<&str, &'static str> {
    if summary.toolbar_stylesheet.is_some() {
        return Err(FILE_TYPE);
    }
    let preview = summary.preview.as_deref().ok_or(PREVIEW_REQUIRED)?;
    if !catalog::is_image(preview) || image_content_type(preview).is_none() {
        return Err(FILE_TYPE);
    }
    Ok(preview)
}

/// 为 `root` 下已安装的包 `id` 生成发布内容：原样的 `skin.toml` 加上它引用的那几张图，不多不少。服务端的规则凡是客户端能检查的都在这里检查，各有各的错误码，服务端会拒收的包在上传前就被拒绝。每张图都按扩展名完整解码一遍，尺寸和整包像素合计也按服务端的上限检查；服务端重新编码之后的大小仍由服务端判断。
pub fn pack(root: &Path, id: &str) -> Result<PackedSkin, &'static str> {
    pack_as(root, id, CandidateSkinVisibility::Public)
}

/// [`pack`] for a package uploaded with `visibility`: a private package may leave out the asset license, which only a public listing has to declare.
pub fn pack_as(
    root: &Path,
    id: &str,
    visibility: CandidateSkinVisibility,
) -> Result<PackedSkin, &'static str> {
    let summary = match catalog::load_package(root, id) {
        Ok(summary) => summary,
        Err(error) if error == IMAGE_DIMENSIONS_TOO_LARGE => return Err(TOO_LARGE),
        Err(_) => return Err(PACKAGE),
    };
    if SERVER_BUILTIN_IDS.contains(&id) {
        return Err(PACKAGE);
    }
    let preview = shared_preview(&summary)?;
    let declared = summary.license.clone().filter(|license| {
        license
            .assets
            .as_deref()
            .is_some_and(|assets| !assets.trim().is_empty())
    });
    let license = match (declared, visibility) {
        (Some(license), _) => license,
        (None, CandidateSkinVisibility::Private) => summary.license.clone().unwrap_or_default(),
        (None, CandidateSkinVisibility::Public) => return Err(LICENSE_REQUIRED),
    };
    // The catalog bounds these by length only, while a listed item carrying a control character in them fails validate_item on every client, so the server refuses such a package and so does pack.
    if [
        Some(&summary.version),
        license.code.as_ref(),
        license.assets.as_ref(),
        license.source.as_ref(),
    ]
    .into_iter()
    .flatten()
    .any(|text| crate::text::has_disallowed_control_with_allowed(text, &[]))
    {
        return Err(PACKAGE);
    }
    let referenced = referenced_images(&summary)?;
    if referenced.len() > MAX_PACKAGE_FILES {
        return Err(TOO_LARGE);
    }
    let directory = root.join(id);
    let mut files = BTreeMap::new();
    let mut size = 0_usize;
    let mut pixels = 0_u64;
    for path in &referenced {
        // read_resource follows links inside the package; a shared image must be a file the package itself holds.
        let metadata = fs::symlink_metadata(directory.join(path)).map_err(|_| PACKAGE)?;
        if !metadata.file_type().is_file() {
            return Err(PACKAGE);
        }
        let limit = if path == preview {
            MAX_PREVIEW_BYTES
        } else {
            MAX_PACKAGE_FILE_BYTES
        };
        if metadata.len() > limit as u64 {
            return Err(TOO_LARGE);
        }
        let resource = catalog::read_resource(root, id, path).map_err(|error| match error {
            catalog::ResourceError::TooLarge => TOO_LARGE,
            _ => PACKAGE,
        })?;
        if image_content_type(path) != Some(resource.content_type) {
            return Err(IMAGE_INVALID);
        }
        if resource.bytes.len() > limit {
            return Err(TOO_LARGE);
        }
        size += resource.bytes.len();
        if size > MAX_PACKAGE_BYTES {
            return Err(TOO_LARGE);
        }
        pixels += check_image(resource.content_type, &resource.bytes)?;
        if pixels > MAX_PACKAGE_PIXELS {
            return Err(TOO_LARGE);
        }
        files.insert(path.clone(), BASE64.encode(&resource.bytes));
    }
    let input = fs::File::open(directory.join(MANIFEST_FILE)).map_err(|_| PACKAGE)?;
    let manifest = crate::bounded_io::read_bounded_file_with(
        input,
        MAX_MANIFEST_BYTES as u64,
        || TOO_LARGE,
        |_| PACKAGE,
    )?;
    let manifest = String::from_utf8(manifest).map_err(|_| PACKAGE)?;
    let suggested: String = summary.name.chars().take(32).collect();
    let suggested = suggested.trim();
    let suggested_name = if valid_name(suggested) {
        suggested.to_owned()
    } else {
        id.chars().take(32).collect()
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
    Ok(PackedSkin {
        package_id: summary.id,
        manifest,
        file_count: files.len(),
        files,
        license,
        suggested_name,
        suggested_description,
        size,
    })
}

/// Give the installed package `id` under `root`, which has no `preview`, the preview image `bytes` (a PNG or JPEG within [`MAX_PREVIEW_BYTES`]) so it can be shared. The image is written under a name the package does not use yet and `preview` is added as the manifest's first key, leaving every other line as the author wrote it. Returns the image's path in the package.
///
/// A decorated package without its own decoration image is refused: the catalog draws its preview in the decoration band, so a preview added to it would change how the skin looks.
pub fn add_preview(root: &Path, id: &str, bytes: &[u8]) -> Result<String, &'static str> {
    let summary = catalog::load_package(root, id).map_err(|_| PACKAGE)?;
    if SERVER_BUILTIN_IDS.contains(&id) || summary.preview.is_some() {
        return Err(PACKAGE);
    }
    if summary.decoration_top_dip > 0.0
        && summary.decoration_width_dip > 0.0
        && summary.decoration_image.is_none()
    {
        return Err(PACKAGE);
    }
    let (extension, content_type) = if magic_matches("image/png", bytes) {
        ("png", "image/png")
    } else if magic_matches("image/jpeg", bytes) {
        ("jpg", "image/jpeg")
    } else {
        return Err(IMAGE_INVALID);
    };
    if bytes.len() > MAX_PREVIEW_BYTES {
        return Err(TOO_LARGE);
    }
    check_image(content_type, bytes)?;
    let directory = root.join(id);
    let manifest_path = directory.join(MANIFEST_FILE);
    let input = fs::File::open(&manifest_path).map_err(|_| PACKAGE)?;
    let original = crate::bounded_io::read_bounded_file_with(
        input,
        MAX_MANIFEST_BYTES as u64,
        || TOO_LARGE,
        |_| PACKAGE,
    )?;
    let text = std::str::from_utf8(&original).map_err(|_| PACKAGE)?;
    let taken: BTreeSet<String> = referenced_images(&summary)?
        .iter()
        .map(|path| path.to_ascii_lowercase())
        .collect();
    let name = (1..100)
        .map(|index| match index {
            1 => format!("preview.{extension}"),
            _ => format!("preview-{index}.{extension}"),
        })
        .find(|name| {
            !taken.contains(name)
                && matches!(
                    fs::symlink_metadata(directory.join(name)),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound
                )
        })
        .ok_or(STORAGE)?;
    // A top-level key is valid TOML only before the first table header, so it goes first, after a byte order mark if the file has one.
    let body = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bom = &text[..text.len() - body.len()];
    let manifest = format!("{bom}preview = \"{name}\"\n{body}");
    if manifest.len() > MAX_MANIFEST_BYTES {
        return Err(TOO_LARGE);
    }
    let image_path = directory.join(&name);
    let mut image = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&image_path)
        .map_err(|_| STORAGE)?;
    let undo_image = || {
        let _ = fs::remove_file(&image_path);
    };
    if image
        .write_all(bytes)
        .and_then(|()| image.sync_all())
        .is_err()
    {
        undo_image();
        return Err(STORAGE);
    }
    let staged = directory.join(".skin.toml.preview");
    if fs::write(&staged, &manifest)
        .and_then(|()| fs::rename(&staged, &manifest_path))
        .is_err()
    {
        let _ = fs::remove_file(&staged);
        undo_image();
        return Err(STORAGE);
    }
    match catalog::load_package(root, id) {
        Ok(updated) if updated.preview.as_deref() == Some(name.as_str()) => Ok(name),
        _ => {
            let _ = fs::write(&manifest_path, &original);
            undo_image();
            Err(PACKAGE)
        }
    }
}

/// Give the installed package `id` under `root`, which declares no asset license, the asset license `assets` (an SPDX id such as `CC-BY-4.0`, or the author's own words), so it can be published. The line goes under the manifest's `[license]` header when it has one, and a new `[license]` table is appended when it has none, leaving every other line as the author wrote it.
///
/// A license the manifest declares some other way (an inline table, dotted keys, an empty `assets`) is refused rather than rewritten: the page then points the author at the file, as it did before.
pub fn add_license(root: &Path, id: &str, assets: &str) -> Result<(), &'static str> {
    let assets = assets.trim();
    if assets.is_empty() || !crate::text::is_bounded_text(assets, 120) {
        return Err(PACKAGE);
    }
    let summary = catalog::load_package(root, id).map_err(|_| PACKAGE)?;
    if SERVER_BUILTIN_IDS.contains(&id)
        || summary
            .license
            .is_some_and(|license| license.assets.is_some())
    {
        return Err(PACKAGE);
    }
    let manifest_path = root.join(id).join(MANIFEST_FILE);
    let input = fs::File::open(&manifest_path).map_err(|_| PACKAGE)?;
    let original = crate::bounded_io::read_bounded_file_with(
        input,
        MAX_MANIFEST_BYTES as u64,
        || TOO_LARGE,
        |_| PACKAGE,
    )?;
    let text = std::str::from_utf8(&original).map_err(|_| PACKAGE)?;
    let line = format!("assets = {}\n", toml::Value::String(assets.to_owned()));
    let manifest = match license_header_end(text) {
        Some(end) if text[..end].ends_with('\n') => {
            format!("{}{line}{}", &text[..end], &text[end..])
        }
        Some(end) => format!("{}\n{line}{}", &text[..end], &text[end..]),
        None => {
            let separator = if text.is_empty() || text.ends_with('\n') {
                ""
            } else {
                "\n"
            };
            format!("{text}{separator}\n[license]\n{line}")
        }
    };
    if manifest.len() > MAX_MANIFEST_BYTES {
        return Err(TOO_LARGE);
    }
    // The header search is textual, so the result is parsed before it is written: a header inside a multi-line string, or a license defined elsewhere as well, leaves the file alone.
    let written = toml::from_str::<toml::Table>(manifest.trim_start_matches('\u{feff}'))
        .ok()
        .and_then(|table| {
            table
                .get("license")?
                .get("assets")?
                .as_str()
                .map(str::to_owned)
        });
    if written.as_deref() != Some(assets) {
        return Err(PACKAGE);
    }
    let staged = root.join(id).join(".skin.toml.license");
    if fs::write(&staged, &manifest)
        .and_then(|()| fs::rename(&staged, &manifest_path))
        .is_err()
    {
        let _ = fs::remove_file(&staged);
        return Err(STORAGE);
    }
    match catalog::load_package(root, id) {
        Ok(updated)
            if updated
                .license
                .as_ref()
                .and_then(|license| license.assets.as_deref())
                == Some(assets) =>
        {
            Ok(())
        }
        _ => {
            let _ = fs::write(&manifest_path, &original);
            Err(PACKAGE)
        }
    }
}

/// The byte offset just past the line holding the manifest's `[license]` header, a trailing comment allowed; `None` when no line is that header.
fn license_header_end(text: &str) -> Option<usize> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        offset += line.len();
        let rest = line.trim().strip_prefix("[license]").map(str::trim_start);
        if rest.is_some_and(|rest| rest.is_empty() || rest.starts_with('#')) {
            return Some(offset);
        }
    }
    None
}

/// Server `candidateRequestDigest`: SHA-256 over the name, description and manifest, each followed by a NUL, then for each file in byte order of its path the path, a NUL, the hex SHA-256 of its original bytes and a newline. It identifies content by what was uploaded, so it survives the server re-encoding every image. `files` are in standard base64, as a request carries them.
pub fn request_digest(
    name: &str,
    description: &str,
    manifest: &str,
    files: &BTreeMap<String, String>,
) -> Result<String, &'static str> {
    let mut hasher = Sha256::new();
    for part in [name, description, manifest] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    for (path, data) in files {
        let bytes = BASE64.decode(data).map_err(|_| IMAGE_INVALID)?;
        let sum = hex::encode(Sha256::digest(&bytes));
        hasher.update(format!("{path}\0{sum}\n").as_bytes());
    }
    Ok(hex::encode(hasher.finalize()))
}

#[derive(Deserialize)]
struct ManifestId {
    id: String,
}

/// Check every key and decode every image of a downloaded package, before anything is written.
fn decode_files(files: &BTreeMap<String, String>) -> Result<Vec<(&str, Vec<u8>)>, &'static str> {
    if files.is_empty() || files.len() > MAX_PACKAGE_FILES {
        return Err(FILE_PATH);
    }
    let mut lowercase = HashSet::with_capacity(files.len());
    for path in files.keys() {
        if !catalog::safe_resource(path, MAX_RESOURCE_PATH_BYTES) || path == MANIFEST_FILE {
            return Err(FILE_PATH);
        }
        if image_content_type(path).is_none() {
            return Err(FILE_TYPE);
        }
        if !lowercase.insert(path.to_ascii_lowercase()) {
            return Err(FILE_PATH);
        }
    }
    let mut decoded = Vec::with_capacity(files.len());
    let mut total = 0_usize;
    let mut pixels = 0_u64;
    for (path, data) in files {
        if data.len() > base64_length(MAX_PACKAGE_FILE_BYTES) {
            return Err(TOO_LARGE);
        }
        let bytes = BASE64.decode(data).map_err(|_| IMAGE_INVALID)?;
        if bytes.len() > MAX_PACKAGE_FILE_BYTES {
            return Err(TOO_LARGE);
        }
        total += bytes.len();
        if total > MAX_PACKAGE_BYTES {
            return Err(TOO_LARGE);
        }
        let content_type = image_content_type(path).ok_or(FILE_TYPE)?;
        pixels += check_image(content_type, &bytes)?;
        if pixels > MAX_PACKAGE_PIXELS {
            return Err(TOO_LARGE);
        }
        decoded.push((path.as_str(), bytes));
    }
    Ok(decoded)
}

fn write_new(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn remove_leftover(path: &Path) -> Result<(), &'static str> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err(STORAGE),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path).map_err(|_| STORAGE)?;
    } else {
        fs::remove_file(path).map_err(|_| STORAGE)?;
    }
    Ok(())
}

/// Install a downloaded package into `root` under its `package_id`, and return that id.
///
/// The manifest id, every key and every image are checked before anything is written. The package is then written to a staging folder in the root, validated there by the same rule `scan` lists skins by plus the sharing rules, and only then swapped in, replacing an existing folder of that name whole. Without `replace` an existing folder is left alone and `candidate_skin_exists` returned, so the page can ask first. The staging folder is removed on every path: dot-prefixed folders are not hidden from `scan`, which would report a leftover as an invalid skin.
pub fn install(
    root: &Path,
    package: &CandidateSkinPackage,
    replace: bool,
) -> Result<String, &'static str> {
    let package_id = package.package_id.as_str();
    if package.manifest.len() > MAX_MANIFEST_BYTES {
        return Err(TOO_LARGE);
    }
    let manifest_id = toml::from_str::<ManifestId>(&package.manifest)
        .map_err(|_| PACKAGE)?
        .id;
    if manifest_id != package_id || !valid_package_id(package_id) {
        return Err(PACKAGE);
    }
    let files = decode_files(&package.files)?;
    // A linked root would install into an unrelated directory; check it before creating it, as folder import does.
    if !crate::storage::create_directory_and_check(root).unwrap_or(false) {
        return Err(STORAGE);
    }
    let target = root.join(package_id);
    // Held from the existence check through the swap, so an overlapping install or folder import can neither clear this one's helpers nor land a folder of the same name after the check.
    let _writes = super::folder_import::lock_skin_root();
    if !replace && fs::symlink_metadata(&target).is_ok() {
        return Err(EXISTS);
    }
    let staging_root = root.join(STAGING_DIRECTORY);
    let result = stage_and_swap(root, &staging_root, package, &files, &target);
    let _ = fs::remove_dir_all(&staging_root);
    result.map(|()| package_id.to_owned())
}

fn stage_and_swap(
    root: &Path,
    staging_root: &Path,
    package: &CandidateSkinPackage,
    files: &[(&str, Vec<u8>)],
    target: &Path,
) -> Result<(), &'static str> {
    let package_id = package.package_id.as_str();
    let backup = root.join(format!(".replaced-{package_id}"));
    remove_leftover(staging_root)?;
    remove_leftover(&backup)?;
    let staging = staging_root.join(package_id);
    fs::create_dir(staging_root).map_err(|_| STORAGE)?;
    fs::create_dir(&staging).map_err(|_| STORAGE)?;
    write_new(&staging.join(MANIFEST_FILE), package.manifest.as_bytes()).map_err(|_| STORAGE)?;
    for (path, bytes) in files {
        // Every segment of `path` passed safe_resource, so the parents created here stay inside the staging folder.
        let destination = staging.join(path);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|_| STORAGE)?;
        }
        write_new(&destination, bytes).map_err(|_| STORAGE)?;
    }
    let summary = catalog::load_package(staging_root, package_id).map_err(|_| PACKAGE)?;
    let preview = shared_preview(&summary).map_err(|_| PACKAGE)?;
    let referenced = referenced_images(&summary).map_err(|_| PACKAGE)?;
    if referenced.len() != files.len() || files.iter().any(|(path, _)| !referenced.contains(*path))
    {
        return Err(PACKAGE);
    }
    if files
        .iter()
        .any(|(path, bytes)| *path == preview && bytes.len() > MAX_PREVIEW_BYTES)
    {
        return Err(TOO_LARGE);
    }
    super::folder_import::replace_directory(&staging, target, &backup)
}

#[cfg(test)]
mod tests;

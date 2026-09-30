//! Sharing candidate-window skin packages through the community library.
//!
//! A shared package is an installed skin folder reduced to what the backend accepts: `skin.toml`, sent verbatim, plus the PNG or JPEG images its manifest references (the preview, the decoration image and the background image), base64-encoded. [`pack`] builds that payload from a folder under the host's skin root and [`install`] writes a downloaded one back as a folder the catalog lists. Both mirror every rule of the server (`internal/account/community_candidate.go`) the client can check, because the transport only reports an HTTP status and the page can then name the exact problem.

use super::catalog::{self, SkinLicense, SkinSummary};
use crate::account::{
    request_with_account_session, AccountApi, AccountError, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession,
};
use crate::cloud::dictionary::percent_encode;
use crate::community::{
    valid_author, valid_description, valid_name, valid_query, valid_rating,
    MAXIMUM_JAVASCRIPT_INTEGER, MAXIMUM_PAGE_ITEMS,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
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
/// Candidate-skin ids the server's curated catalog keeps for itself (`internal/skins/catalog.go`). They are valid folder names on the client, so the catalog rule alone does not refuse them.
pub const SERVER_BUILTIN_IDS: [&str; 4] = ["fluent", "wechat", "graphite", "willow_green"];

const MAX_PUBLISH_RESPONSE_BYTES: usize = 64 * 1024;
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(90);
const MAX_RESOURCE_PATH_BYTES: usize = 256;
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
}

impl CandidateSkinPublishRequest {
    /// The request that publishes `packed` under the listing `name` and `description`.
    pub fn new(id: Uuid, name: String, description: String, packed: PackedSkin) -> Self {
        Self {
            id,
            name,
            description,
            manifest: packed.manifest,
            files: packed.files,
        }
    }
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
        token: Option<&str>,
    ) -> Result<CandidateSkinPage, AccountError>;
    fn candidate_skin(
        &self,
        id: Uuid,
        token: Option<&str>,
    ) -> Result<CandidateSkinItem, AccountError>;
    fn candidate_skin_preview(&self, id: Uuid) -> Result<CandidateSkinPreview, AccountError>;
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
}

impl CandidateSkinCommunityApi for BackendAccountClient {
    fn candidate_skins(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
        token: Option<&str>,
    ) -> Result<CandidateSkinPage, AccountError> {
        validate_query(offset, search)?;
        if mine && token.is_none() {
            return Err(AccountError::Unauthorized);
        }
        let scope = if mine { "&scope=mine" } else { "" };
        let path = format!(
            "/v1/community/candidate-skins?offset={offset}&q={}{scope}",
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
        let path = format!("/v1/community/candidate-skins/{}", id.hyphenated());
        let item = self.json::<CandidateSkinItem, ()>(Method::GET, &path, token, None)?;
        validate_item(&item)?;
        if item.id != id {
            return Err(AccountError::Unavailable);
        }
        Ok(item)
    }

    fn candidate_skin_preview(&self, id: Uuid) -> Result<CandidateSkinPreview, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        let path = format!("/v1/community/candidate-skins/{}/preview", id.hyphenated());
        let preview = self.json_with_limit::<CandidateSkinPreview, ()>(
            Method::GET,
            &path,
            None,
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
            "/v1/community/candidate-skins",
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
    /// One page of published packages, newest first. `mine` lists only the signed-in user's own, and so requires a session.
    pub fn list(
        &self,
        offset: usize,
        search: &str,
        mine: bool,
    ) -> Result<CandidateSkinPage, AccountError> {
        validate_query(offset, search)?;
        request_with_account_session(&self.api, &self.session, mine, |api, token| {
            api.candidate_skins(offset, search, mine, token)
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

    /// The preview is public and carries nothing per viewer, so it is fetched without the session.
    pub fn preview(&self, id: Uuid) -> Result<CandidateSkinPreview, AccountError> {
        if id.is_nil() {
            return Err(AccountError::Invalid);
        }
        self.api.candidate_skin_preview(id)
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
}

fn validate_query(offset: usize, search: &str) -> Result<(), AccountError> {
    if !valid_query(offset, search) {
        return Err(AccountError::Invalid);
    }
    Ok(())
}

fn validate_publish(request: &CandidateSkinPublishRequest) -> Result<(), AccountError> {
    if request.id.is_nil()
        || !valid_name(&request.name)
        || !valid_description(&request.description)
        || request.manifest.is_empty()
        || request.manifest.len() > MAX_MANIFEST_BYTES
        || request.files.is_empty()
        || request.files.len() > MAX_PACKAGE_FILES
        || request
            .files
            .keys()
            .any(|path| !catalog::safe_resource(path, MAX_RESOURCE_PATH_BYTES))
    {
        return Err(AccountError::Invalid);
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
    let mut ids = BTreeSet::new();
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
        || license.assets.trim().is_empty()
        || !crate::text::is_bounded_text(&license.assets, 120)
        || !crate::text::is_bounded_text(&license.source, 500)
        || item.size > MAX_PACKAGE_BYTES as u64
        || item.file_count > MAX_PACKAGE_FILES as u64
        || item.downloads > MAXIMUM_JAVASCRIPT_INTEGER
        || !valid_rating(item.rating_count, item.rating_average, item.my_rating)
        || item.created_at.is_empty()
        || !crate::text::is_bounded_text(&item.created_at, 64)
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
    pub license: SkinLicense,
    /// The manifest name cut to the 32 characters a listing name allows, or the package id when that is not a valid listing name.
    pub suggested_name: String,
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
    let lowercase: BTreeSet<String> = paths.iter().map(|path| path.to_ascii_lowercase()).collect();
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

/// Build the publish payload for the installed package `id` under `root`: `skin.toml` verbatim and exactly the images it references. Every rule the server applies that the client can check is applied here, each with its own error code, so a package the server would refuse is refused before anything is uploaded. Image dimensions and decodability are left to the server, which re-encodes every image.
pub fn pack(root: &Path, id: &str) -> Result<PackedSkin, &'static str> {
    let summary = catalog::load_package(root, id).map_err(|_| PACKAGE)?;
    if SERVER_BUILTIN_IDS.contains(&id) {
        return Err(PACKAGE);
    }
    let preview = shared_preview(&summary)?;
    let license = summary
        .license
        .clone()
        .filter(|license| {
            license
                .assets
                .as_deref()
                .is_some_and(|assets| !assets.trim().is_empty())
        })
        .ok_or(LICENSE_REQUIRED)?;
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
        if image_content_type(path) != Some(resource.content_type)
            || !magic_matches(resource.content_type, &resource.bytes)
        {
            return Err(IMAGE_INVALID);
        }
        if resource.bytes.len() > limit {
            return Err(TOO_LARGE);
        }
        size += resource.bytes.len();
        if size > MAX_PACKAGE_BYTES {
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
    Ok(PackedSkin {
        package_id: summary.id,
        manifest,
        file_count: files.len(),
        files,
        license,
        suggested_name,
        size,
    })
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
    let mut lowercase = BTreeSet::new();
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
        if !magic_matches(content_type, &bytes) {
            return Err(IMAGE_INVALID);
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
    if fs::symlink_metadata(path).is_ok() {
        fs::remove_dir_all(path).map_err(|_| STORAGE)?;
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

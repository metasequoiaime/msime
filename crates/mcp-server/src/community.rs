//! 把已安装的候选窗皮肤发布到社区，并列出、改可见性和删除账号社区库里的皮肤。
//!
//! 账号会话用的是用户在设置应用里登录后保存的那一份 `account-session.json`，位置与格式和各平台设置应用的 `desktop_account` 相同（见 [`session_location`]）。本服务器从不登录、不接触密码，没有会话时让用户去设置应用登录；令牌只在 client-services 里使用，不进工具结果，也不进 stderr 的审计行。会话文件每次调用重新读，用户在设置应用里登录、退出或换账号之后不用重启服务器。
//!
//! 发布走共享的 `candidate_sync::publish`：打包规则、同步状态文件和设置应用的发布对话框是同一套，账号库里已有同一个 `package_id` 的行（设置应用同步上去的私有包，或之前发布过的）就原地替换内容、可见性和分类，所以设置应用和这里不会为同一款本地皮肤各建一行。新行的发布 ID 与设置页一样每次随机生成，去重靠 `package_id` 而不是 ID。

use crate::skins::SKIN_DIRECTORY;
use msime_client_core::edition::Edition;
use msime_client_core::uuid::Uuid;
use msime_client_services::account::{
    AccountApi, AccountError, AccountSessionFileLayout, BackendAccountClient,
    BackendAccountSession, FileAccountSessionStorage,
};
use msime_client_services::skin::candidate_community::{
    self, BackendCandidateSkinCommunityService, CandidateSkinCategory, CandidateSkinCommunityApi,
    CandidateSkinVisibility,
};
use msime_client_services::skin::candidate_sync::{
    self, CandidateSkinPublishError, CandidateSkinSyncRemote,
};
use msime_client_services::skin::catalog;
use rmcp::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// 社区列表标题的字符数上限，与 client-services 的 `valid_name` 相同。
const MAX_NAME_CHARS: usize = 32;
/// 社区列表简介的字符数上限，与 client-services 的 `valid_description` 相同。
const MAX_DESCRIPTION_CHARS: usize = 280;

const SIGN_IN: &str = "no one is signed in: ask the user to sign in to their account in the 水杉输入法 settings app and then try again. This server never signs in or handles passwords itself";

/// 谁能看到社区库里的一款皮肤。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Listed in the community: everyone can see, download and rate it.
    Public,
    /// Kept in the user's own cloud library, visible only to them.
    Private,
}

impl From<Visibility> for CandidateSkinVisibility {
    fn from(value: Visibility) -> Self {
        match value {
            Visibility::Public => Self::Public,
            Visibility::Private => Self::Private,
        }
    }
}

impl From<CandidateSkinVisibility> for Visibility {
    fn from(value: CandidateSkinVisibility) -> Self {
        match value {
            CandidateSkinVisibility::Public => Self::Public,
            CandidateSkinVisibility::Private => Self::Private,
        }
    }
}

/// 社区分类。单独定义而不是直接用 `CandidateSkinCategory`：后者把不认识的值读成 `other`，代理写错分类时应当被拒绝，而不是悄悄归进「其他」。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Nature,
    Guofeng,
    Acg,
    Cute,
    Food,
    Tech,
    Minimal,
    Other,
}

impl From<Category> for CandidateSkinCategory {
    fn from(value: Category) -> Self {
        match value {
            Category::Nature => Self::Nature,
            Category::Guofeng => Self::Guofeng,
            Category::Acg => Self::Acg,
            Category::Cute => Self::Cute,
            Category::Food => Self::Food,
            Category::Tech => Self::Tech,
            Category::Minimal => Self::Minimal,
            Category::Other => Self::Other,
        }
    }
}

impl From<CandidateSkinCategory> for Category {
    fn from(value: CandidateSkinCategory) -> Self {
        match value {
            CandidateSkinCategory::Nature => Self::Nature,
            CandidateSkinCategory::Guofeng => Self::Guofeng,
            CandidateSkinCategory::Acg => Self::Acg,
            CandidateSkinCategory::Cute => Self::Cute,
            CandidateSkinCategory::Food => Self::Food,
            CandidateSkinCategory::Tech => Self::Tech,
            CandidateSkinCategory::Minimal => Self::Minimal,
            CandidateSkinCategory::Other => Self::Other,
        }
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct PublishRequest {
    /// The installed skin to publish: an id list_candidate_skins lists.
    pub package_id: String,
    /// public (everyone can see it; confirm with the user first) or private (only in the user's own cloud library). Required: there is no default.
    pub visibility: Visibility,
    /// nature, guofeng, acg, cute, food, tech, minimal or other. Without it a new listing is other and an existing one keeps its category.
    #[serde(default)]
    pub category: Option<Category>,
    /// The listing's title, 1 to 32 characters. Defaults to the manifest's name.
    #[serde(default)]
    pub name: Option<String>,
    /// The listing's text, at most 280 characters. Defaults to the manifest's description.
    #[serde(default)]
    pub description: Option<String>,
    /// Who may reuse the skin's images, written into skin.toml as [license] assets when the manifest has none: an SPDX id such as CC-BY-4.0 or CC0-1.0, or the user's own words, at most 120 bytes. A public skin needs it; ask the user which to use.
    #[serde(default)]
    pub license_assets: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct PublishedSkin {
    /// The skin's id in the community library, which set_community_skin_visibility and delete_community_skin take.
    pub id: String,
    pub package_id: String,
    /// The listing's title.
    pub name: String,
    pub visibility: Visibility,
    /// The category the server reports, or null from a server that predates categories.
    pub category: Option<Category>,
    /// Whether an entry this account already had for package_id was replaced, rather than a new one created.
    pub replaced: bool,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CommunitySkinList {
    /// Newest change first.
    pub skins: Vec<CommunitySkinView>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct CommunitySkinView {
    /// The skin's id in the community library.
    pub id: String,
    /// The skin id it was made from, the folder it installs into.
    pub package_id: String,
    pub visibility: Visibility,
    /// When its content or visibility last changed, as the server reports it (RFC 3339).
    pub updated_at: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct VisibilityRequest {
    /// The community id list_community_skins or publish_candidate_skin gave.
    pub id: String,
    pub visibility: Visibility,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct VisibilityChanged {
    pub id: String,
    pub package_id: String,
    pub visibility: Visibility,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(crate = "rmcp::schemars")]
#[serde(deny_unknown_fields)]
pub struct CommunitySkinRequest {
    /// The community id list_community_skins or publish_candidate_skin gave.
    pub id: String,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[schemars(crate = "rmcp::schemars")]
pub struct DeletedCommunitySkin {
    pub id: String,
}

/// 用 `api` 和 `directory` 里的会话文件组成社区服务。
pub fn service_at<A>(
    api: A,
    directory: &Path,
    layout: AccountSessionFileLayout,
) -> BackendCandidateSkinCommunityService<A, FileAccountSessionStorage>
where
    A: AccountApi + CandidateSkinCommunityApi + Clone,
{
    let storage = FileAccountSessionStorage::new(directory, layout);
    let session = Arc::new(BackendAccountSession::new(api.clone(), storage));
    BackendCandidateSkinCommunityService::new(api, session)
}

/// 设置应用登录后的社区服务：会话文件在 [`session_location`]，请求经正式的账号服务器。
pub fn service(
    state_dir: &Path,
    edition: &Edition,
) -> Result<
    BackendCandidateSkinCommunityService<BackendAccountClient, FileAccountSessionStorage>,
    String,
> {
    let (directory, layout) = session_location(state_dir, edition, |name| std::env::var_os(name))?;
    let client = BackendAccountClient::new()
        .map_err(|_| "cannot set up the connection to the community server")?;
    Ok(service_at(client, &directory, layout))
}

/// 设置应用保存账号会话的目录和文件格式，各平台与 `desktop_account` 的 `setup` 相同：macOS 是 `~/Library/Application Support/<设置应用的 bundle id>`，与输入法共用，Swift 后端的格式；Windows 是 `%LOCALAPPDATA%\<本版本的用户目录>\account`，Server 也读这一份；Linux 是共享状态目录，也就是偏好所在的 `state_dir`。`env` 是进程环境，传进来以便测试。
pub fn session_location(
    state_dir: &Path,
    edition: &Edition,
    env: impl Fn(&str) -> Option<OsString>,
) -> Result<(PathBuf, AccountSessionFileLayout), String> {
    let absolute = |name: &str| {
        env(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(|| format!("cannot find the account session: {name} is not set"))
    };
    if cfg!(target_os = "macos") {
        let identity = edition
            .macos()
            .ok_or("this edition of the input method has no macOS settings app")?;
        let directory = absolute("HOME")?
            .join("Library/Application Support")
            .join(&identity.settings_bundle_id);
        return Ok((directory, AccountSessionFileLayout::Apple));
    }
    if cfg!(windows) {
        let identity = edition
            .windows()
            .ok_or("this edition of the input method has no Windows settings app")?;
        let directory = absolute("LOCALAPPDATA")?
            .join(&identity.user_data_directory)
            .join("account");
        return Ok((directory, AccountSessionFileLayout::Native));
    }
    Ok((state_dir.to_owned(), AccountSessionFileLayout::Native))
}

fn skin_root(state_dir: &Path) -> PathBuf {
    state_dir.join(SKIN_DIRECTORY)
}

/// 已登录用户的 id；没登录时是让用户去设置应用登录的说明。只读会话文件，不联网。
fn signed_in(remote: &impl CandidateSkinSyncRemote) -> Result<String, String> {
    remote
        .user_id()
        .map_err(account_problem)?
        .ok_or_else(|| SIGN_IN.to_owned())
}

fn community_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id.trim())
        .ok()
        .filter(|id| !id.is_nil())
        .ok_or_else(|| format!("{id:?} is not a community id; list_community_skins gives the ids"))
}

/// 发布一款已安装的皮肤。先确认已登录，再按需写素材授权、按分享规则打包检查，都过了才联网。
pub fn publish(
    state_dir: &Path,
    remote: &impl CandidateSkinSyncRemote,
    request: &PublishRequest,
) -> Result<PublishedSkin, String> {
    let id = request.package_id.as_str();
    if !catalog::is_external_id(id) {
        return Err(format!("{id:?} is not a skin the user installed: built-in themes, msime-windows built-in looks, canvas and default cannot be published"));
    }
    let root = skin_root(state_dir);
    if std::fs::symlink_metadata(root.join(id)).is_err() {
        return Err(format!(
            "no skin {id} is installed; list_candidate_skins shows the installed skins"
        ));
    }
    // 没登录时什么都不改，包括清单里的授权。
    signed_in(remote)?;
    if let Some(assets) = request.license_assets.as_deref() {
        ensure_license(&root, id, assets)?;
    }
    let visibility = CandidateSkinVisibility::from(request.visibility);
    // 先在本机按分享规则打一次包：被拒时给出具体原因，也拿到默认的标题和简介。
    let packed = candidate_community::pack_as(&root, id, visibility)
        .map_err(|code| package_problem(id, code))?;
    let name = match request.name.as_deref().map(str::trim) {
        Some(name) => name.to_owned(),
        None => packed.suggested_name.clone(),
    };
    if name.is_empty() || name.chars().count() > MAX_NAME_CHARS {
        return Err(format!("name must be 1 to {MAX_NAME_CHARS} characters"));
    }
    let description = match request.description.as_deref().map(str::trim) {
        Some(description) => description.to_owned(),
        None => packed.suggested_description.clone(),
    };
    if description.chars().count() > MAX_DESCRIPTION_CHARS {
        return Err(format!(
            "description must be at most {MAX_DESCRIPTION_CHARS} characters"
        ));
    }
    let publication = Uuid::new_v4();
    let item = candidate_sync::publish(
        &root,
        &candidate_sync::state_path(&root),
        remote,
        id,
        publication,
        name,
        description,
        visibility,
        request.category.map(Into::into),
    )
    .map_err(|error| match error {
        CandidateSkinPublishError::Package(code) => package_problem(id, code),
        CandidateSkinPublishError::Account(error) => account_problem(error),
    })?;
    Ok(PublishedSkin {
        // 原地替换时返回的是库里原来那一行，ID 不是这次新生成的。
        replaced: item.id != publication,
        id: item.id.hyphenated().to_string(),
        package_id: item.package_id,
        name: item.name,
        visibility: item.visibility.into(),
        category: item.category.map(Into::into),
    })
}

/// 账号社区库里的全部皮肤，私有的也在内。
pub fn list(remote: &impl CandidateSkinSyncRemote) -> Result<CommunitySkinList, String> {
    signed_in(remote)?;
    let rows = remote.sync_list().map_err(account_problem)?;
    Ok(CommunitySkinList {
        skins: rows
            .into_iter()
            .map(|row| CommunitySkinView {
                id: row.id.hyphenated().to_string(),
                package_id: row.package_id,
                visibility: row.visibility.into(),
                updated_at: row.updated_at,
            })
            .collect(),
    })
}

pub fn set_visibility(
    remote: &impl CandidateSkinSyncRemote,
    request: &VisibilityRequest,
) -> Result<VisibilityChanged, String> {
    let id = community_id(&request.id)?;
    signed_in(remote)?;
    let item = remote
        .set_visibility(id, request.visibility.into())
        .map_err(|error| match (error, request.visibility) {
            // 转为公开时服务端要求已上传的清单带素材授权，缺了回 400。
            (AccountError::Invalid, Visibility::Public) => "the community refused to make it public: the uploaded skin.toml must declare [license] assets. Publish the installed skin again with publish_candidate_skin, visibility public and license_assets, which uploads the license as well".to_owned(),
            (error, _) => account_problem(error),
        })?;
    Ok(VisibilityChanged {
        id: item.id.hyphenated().to_string(),
        package_id: item.package_id,
        visibility: item.visibility.into(),
    })
}

/// 从账号社区库删掉一行。同步状态里对应的记录一并忘掉，本机那款皮肤留着，设置应用下一次同步把它当成新的私有包再传上去。
pub fn delete(
    state_dir: &Path,
    remote: &impl CandidateSkinSyncRemote,
    request: &CommunitySkinRequest,
) -> Result<DeletedCommunitySkin, String> {
    let id = community_id(&request.id)?;
    signed_in(remote)?;
    candidate_sync::unpublish(
        &candidate_sync::state_path(&skin_root(state_dir)),
        remote,
        id,
    )
    .map_err(account_problem)?;
    Ok(DeletedCommunitySkin {
        id: id.hyphenated().to_string(),
    })
}

/// 清单没有素材授权时写进去；已有同样的授权时不动；已有别的授权时拒绝，不替用户改掉。
fn ensure_license(root: &Path, id: &str, assets: &str) -> Result<(), String> {
    let assets = assets.trim();
    let declared = catalog::load_package(root, id)
        .map_err(|reason| format!("{id} cannot be read as a skin ({reason}); list_candidate_skins lists it under issues with the reason"))?
        .license
        .and_then(|license| license.assets);
    match declared {
        Some(existing) if existing.trim() == assets => Ok(()),
        Some(existing) if !existing.trim().is_empty() => Err(format!("{id}'s skin.toml already declares [license] assets = {existing:?}; leave license_assets out to keep it, or change it in the manifest with get_candidate_skin and create_candidate_skin (replace: true)")),
        Some(_) => Err(format!("{id}'s skin.toml has an empty [license] assets; write the license there with get_candidate_skin and create_candidate_skin (replace: true)")),
        None => candidate_community::add_license(root, id, assets).map_err(|code| match code {
            "storage" => "skin.toml could not be written".to_owned(),
            "candidate_skin_too_large" => "skin.toml would grow past 64 KiB with the license".to_owned(),
            _ => format!("license_assets must be 1 to 120 bytes of text on one line, and {id}'s skin.toml must declare its license, if at all, as a plain [license] table; otherwise write assets there with get_candidate_skin and create_candidate_skin (replace: true)"),
        }),
    }
}

/// 打包被拒的原因，说成代理能照着改的话。
fn package_problem(id: &str, code: &str) -> String {
    match code {
        "candidate_skin_license_required" => "a public skin must say who may reuse its images in skin.toml's [license] assets: ask the user and pass license_assets, such as CC-BY-4.0 (reuse with credit) or CC0-1.0 (no conditions), or publish it as private".into(),
        "candidate_skin_preview_required" => format!("{id} has no preview: add a PNG or JPEG preview of at most 256 KiB to its manifest and images with get_candidate_skin and create_candidate_skin (replace: true)"),
        "candidate_skin_file_type" => "only PNG, JPEG, APNG, GIF and WebP images can be shared, the preview must be PNG or JPEG, and a skin with a toolbar_stylesheet cannot be shared".into(),
        "candidate_skin_file_path" => "an image path in skin.toml is not a plain relative path inside the skin folder, or two differ only in case".into(),
        "candidate_skin_too_large" => "too large to share: each image at most 1 MiB and 2048 pixels on each side, the preview at most 256 KiB, all images at most 2 MiB and 8,000,000 pixels together (each animation frame counted as a whole image), at most 37 images and 120 frames".into(),
        "candidate_skin_image_invalid" => "an image cannot be decoded as the format its extension names; make it again with code and reinstall the skin with create_candidate_skin (replace: true)".into(),
        "storage" => "the skin folder or the sync state could not be read or written".into(),
        _ => format!("{id} cannot be shared: its skin.toml or images do not pass the sharing rules (list_candidate_skins shows any issue), a version or [license] text contains a control character, or an image is not a regular file inside the skin folder"),
    }
}

/// 账号或社区服务器的错误，说成用户听得懂、代理知道下一步的话。
fn account_problem(error: AccountError) -> String {
    match error {
        AccountError::Unauthorized => SIGN_IN.into(),
        AccountError::Cancelled => "the user signed out or switched accounts in the settings app while this ran; try again".into(),
        AccountError::Forbidden => "the community server does not allow this account to do that".into(),
        AccountError::NotFound => "this account has no such skin in its community library; list_community_skins lists the ones it has".into(),
        AccountError::Conflict => "the community library is full: an account keeps at most 100 skins, at most 20 of them public. Make one private or delete one first".into(),
        AccountError::RateLimited => "the community allows at most 10 public publishes an hour and 60 private uploads an hour per account; try again later".into(),
        AccountError::Invalid => "the community server refused the request: the skin may use something it does not accept yet (more than 3 images, APNG, GIF or WebP files, or base canvas or wechat_scroll), or the name or description may not be accepted".into(),
        AccountError::BlockedContent => "the community does not allow some words in the name, description or skin.toml; change them and try again".into(),
        AccountError::ScreeningUnavailable => "the community's content check is unavailable right now; try again later".into(),
        AccountError::Banned => "this account is banned from the community".into(),
        AccountError::Storage => "the account session or the skin sync state on this computer could not be read or written".into(),
        AccountError::Unavailable => "the community server cannot be reached right now or gave an unexpected answer; try again later".into(),
    }
}

#[cfg(test)]
mod tests;

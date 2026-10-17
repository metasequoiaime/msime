//! 原生设置窗口的社区操作：与共享应用（Tauri）在 Windows 上的社区命令一一对应——候选窗皮肤社区（`desktop_candidate_skin_community.rs`）、插件社区（`desktop_plugin_community.rs`）、社区词包与回复模板（`community_resources.rs`）和举报（`desktop_community_report.rs`）——另有移动端的键盘皮肤社区（Tauri 的 `mobile_community.rs`）。服务经 [`super::account`] 里同一个账号会话对象认证，用同一份 `account-session.json`；发布和安装都在这里对着宿主给的皮肤根目录或插件根目录做，规则、错误码和返回形状都是 client-services 的，与共享应用相同。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use super::account::{anonymous_session_for, session_for, Session};
use super::plugins::plugin_response;
use super::session_store::{session_source, SessionStorage, Source};
use super::with_bounded_bytes;
use crate::*;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use key_sound::PluginRoots;
use msime_client_core::uuid::Uuid;
use msime_client_services::account::{AccountError, BackendAccountClient};
use msime_client_services::community::report::{
    BackendCommunityReportService, CommunityReport, CommunityReportKind,
};
use msime_client_services::community::resource::{
    BackendCommunityResourceService, CommunityResourceContent, CommunityResourceKind,
    CommunityResourceScope,
};
use msime_client_services::plugins::community::{
    self as plugin_community, BackendCommunityPluginService,
};
use msime_client_services::plugins::{PluginFailure, PluginKind};
use msime_client_services::skin::candidate_community::{
    self, BackendCandidateSkinCommunityService, CandidateSkinCategory, CandidateSkinVisibility,
};
use msime_client_services::skin::candidate_sync::{self, CandidateSkinPublishError};
use msime_client_services::skin::catalog;
use msime_client_services::skin::category::SkinCategory;
use msime_client_services::skin::community::BackendCommunitySkinService;

/// 请求的上限：发布词包时请求里带着整份词条，加上皮肤预览图的 base64，4 MiB 足够；目录各最多 16 KiB。
const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
/// 发布键盘皮肤时设计里可能带着照片背景，入口处放宽到与 `msime_client_community_skin_install` 相同的 9,000,000 字节；解析出操作后，除 `keyboard_skin_publish` 外仍按 [`MAX_REQUEST_BYTES`] 拒绝。
const MAX_KEYBOARD_SKIN_PUBLISH_BYTES: usize = 9_000_000;
const MAX_DIRECTORY_BYTES: usize = 16 * 1024;

type Storage = SessionStorage;

/// 一个账号目录（或一个注入的会话存储）上的五个社区服务，前四个与共享应用在 `desktop_account::manage` 里建的相同，键盘皮肤与移动端的 `MobileCommunityState` 相同，都经同一个会话对象认证。
pub(super) struct Services {
    candidate_skins: BackendCandidateSkinCommunityService<BackendAccountClient, Storage>,
    plugins: BackendCommunityPluginService<BackendAccountClient, Storage>,
    resources: BackendCommunityResourceService<BackendAccountClient, Storage>,
    report: BackendCommunityReportService<BackendAccountClient, Storage>,
    keyboard_skins: BackendCommunitySkinService<BackendAccountClient, Storage>,
}

/// 每个账号目录一组服务，第一次用到时建立。只在取对象时持锁，请求本身在锁外进行。
static COMMUNITY_SERVICES: OnceLock<Mutex<HashMap<PathBuf, Arc<Services>>>> = OnceLock::new();

fn services_for(directory: &Path, legacy: Option<&Path>) -> Result<Arc<Services>, AccountError> {
    let services = COMMUNITY_SERVICES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut services = services.lock().map_err(|_| AccountError::Unavailable)?;
    if let Some(existing) = services.get(directory) {
        return Ok(Arc::clone(existing));
    }
    let session: Arc<Session> = session_for(directory, legacy)?;
    let created = Arc::new(Services::new(BackendAccountClient::new()?, session));
    services.insert(directory.to_path_buf(), Arc::clone(&created));
    Ok(created)
}

impl Services {
    /// 经 `session` 认证的五个服务，共用 `client`。
    pub(super) fn new(client: BackendAccountClient, session: Arc<Session>) -> Self {
        Self {
            candidate_skins: BackendCandidateSkinCommunityService::new(
                client.clone(),
                Arc::clone(&session),
            ),
            plugins: BackendCommunityPluginService::new(client.clone(), Arc::clone(&session)),
            resources: BackendCommunityResourceService::new(client.clone(), Arc::clone(&session)),
            report: BackendCommunityReportService::new(client.clone(), Arc::clone(&session)),
            keyboard_skins: BackendCommunitySkinService::new(client, session),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommunityRequest {
    /// 账号目录，与 `msime_client_account` 的 `directory` 相同；与 `session_store` 恰好给一个。
    #[serde(default)]
    directory: Option<String>,
    /// 注入的会话存储，与 `msime_client_account` 的 `session_store` 相同。
    #[serde(default)]
    session_store: Option<u64>,
    /// 旧版本放会话的目录，与 `msime_client_account` 的 `legacy_directory` 相同。
    #[serde(default)]
    legacy_directory: Option<String>,
    /// 候选窗皮肤根目录；在本机发布、安装、补图、补许可和同步皮肤时必需。
    #[serde(default)]
    skins_directory: Option<String>,
    /// 放 `plugins/` 的状态根目录（与 `msime_client_plugins` 的 `state_root` 相同）；在本机打包、发布和安装插件时必需。
    #[serde(default)]
    state_root: Option<String>,
    action: CommunityAction,
}

/// Tauri 的社区命令，名字去掉 `_community` 换成前缀：`candidate_skin_*`、`plugin_*`、`resource_*` 和 `report`。参数与命令相同，改成 snake_case；皮肤预览图从字节数组改成 base64。
#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum CommunityAction {
    CandidateSkinList {
        offset: usize,
        search: String,
        mine: bool,
        #[serde(default)]
        category: Option<CandidateSkinCategory>,
    },
    CandidateSkinDetail {
        id: String,
    },
    CandidateSkinPreview {
        id: String,
    },
    CandidateSkinInstall {
        id: String,
        replace: bool,
    },
    CandidateSkinPackPreview {
        skin_id: String,
        #[serde(default)]
        visibility: Option<CandidateSkinVisibility>,
    },
    CandidateSkinAddPreview {
        skin_id: String,
        image: String,
    },
    CandidateSkinAddLicense {
        skin_id: String,
        assets: String,
    },
    CandidateSkinPublish {
        skin_id: String,
        id: String,
        name: String,
        description: String,
        #[serde(default)]
        visibility: Option<CandidateSkinVisibility>,
        #[serde(default)]
        category: Option<CandidateSkinCategory>,
    },
    CandidateSkinRate {
        id: String,
        stars: u8,
    },
    CandidateSkinSetVisibility {
        id: String,
        visibility: CandidateSkinVisibility,
    },
    CandidateSkinSetCategory {
        id: String,
        category: CandidateSkinCategory,
    },
    CandidateSkinUnpublish {
        id: String,
    },
    CandidateSkinSync,
    PluginList {
        offset: usize,
        search: String,
        #[serde(default)]
        kind: Option<PluginKind>,
        #[serde(default)]
        mine: Option<bool>,
    },
    PluginDetail {
        id: String,
    },
    PluginPackPreview {
        kind: PluginKind,
        plugin_id: String,
    },
    PluginPublish {
        kind: PluginKind,
        plugin_id: String,
        id: String,
        name: String,
        description: String,
    },
    PluginInstall {
        id: String,
        kind: PluginKind,
        plugin_id: String,
    },
    PluginRate {
        id: String,
        stars: u8,
    },
    PluginDelete {
        id: String,
    },
    ResourceList {
        kind: CommunityResourceKind,
        scope: String,
        search: String,
        offset: usize,
    },
    ResourceDetail {
        id: String,
    },
    ResourcePublish {
        id: String,
        kind: CommunityResourceKind,
        name: String,
        description: String,
        content: CommunityResourceContent,
        revision: u32,
    },
    ResourceApply {
        id: String,
        resource_revision: u32,
    },
    ResourceSave {
        id: String,
        saved: bool,
    },
    ResourceRate {
        id: String,
        stars: u8,
    },
    ResourceUnpublish {
        id: String,
    },
    Report {
        kind: CommunityReportKind,
        id: String,
        reason: String,
        #[serde(default)]
        detail: Option<String>,
    },
    KeyboardSkinList {
        offset: usize,
        search: String,
        #[serde(default)]
        mine: Option<bool>,
        #[serde(default)]
        category: Option<SkinCategory>,
    },
    KeyboardSkinDetail {
        id: String,
    },
    KeyboardSkinDownload {
        id: String,
    },
    KeyboardSkinInstall {
        id: String,
        name: String,
    },
    KeyboardSkinRate {
        id: String,
        stars: u8,
    },
    KeyboardSkinPublish {
        id: String,
        name: String,
        description: String,
        design: msime_client_core::preferences::TouchKeyboardSkinDesign,
        #[serde(default)]
        category: Option<SkinCategory>,
    },
    KeyboardSkinSetCategory {
        id: String,
        category: SkinCategory,
    },
    KeyboardSkinUnpublish {
        id: String,
    },
}

fn invalid() -> PluginFailure {
    PluginFailure::code("community_invalid")
}

fn account(error: AccountError) -> PluginFailure {
    PluginFailure::code(error.community_code())
}

fn id(value: &str) -> Result<Uuid, PluginFailure> {
    Uuid::parse_str(value).map_err(|_| invalid())
}

fn value<T: Serialize>(value: T) -> Result<Value, PluginFailure> {
    serde_json::to_value(value).map_err(|_| PluginFailure::code("storage"))
}

fn absolute_directory(value: &str) -> Result<&Path, PluginFailure> {
    let path = Path::new(value);
    if value.len() > MAX_DIRECTORY_BYTES || !path.is_absolute() {
        return Err(invalid());
    }
    Ok(path)
}

/// 宿主给的可选目录：给了就必须是绝对路径；用到却没给时是 `community_invalid`。
fn optional_directory(value: Option<&str>) -> Result<Option<&Path>, PluginFailure> {
    value.map(absolute_directory).transpose()
}

/// 重新扫描皮肤根目录，形状与共享应用的 `SkinCatalogResponse` 相同：`{directory, packages, issues}`。
fn catalog_response(root: &Path) -> Result<Value, PluginFailure> {
    let mut response = value(catalog::scan(root))?;
    response["directory"] = Value::String(root.to_string_lossy().into_owned());
    Ok(response)
}

/// 一次操作。皮肤根目录、插件根目录和状态根目录只在用到的操作里要求。`anonymous` 给出本机匿名账号的会话，只有键盘皮肤的下载、安装和举报会调用它：这三项登录账号优先、没有登录时用匿名账号（#8201 的决定 13）。
fn run(
    services: &Services,
    anonymous: impl FnOnce() -> Result<Arc<Session>, AccountError>,
    skins: Option<&Path>,
    plugins: Option<&Path>,
    state_root: Option<&Path>,
    action: CommunityAction,
) -> Result<Value, PluginFailure> {
    let skins = || skins.ok_or_else(invalid);
    let plugins = || plugins.ok_or_else(invalid);
    let keyboard = &services.keyboard_skins;
    let package = |code: &'static str| PluginFailure::code(code);
    let candidate = &services.candidate_skins;
    match action {
        CommunityAction::CandidateSkinList {
            offset,
            search,
            mine,
            category,
        } => value(
            candidate
                .list(offset, &search, mine, category)
                .map_err(account)?,
        ),
        CommunityAction::CandidateSkinDetail { id: item } => {
            value(candidate.detail(id(&item)?).map_err(account)?)
        }
        CommunityAction::CandidateSkinPreview { id: item } => {
            let preview = candidate.preview(id(&item)?).map_err(account)?;
            Ok(json!({ "dataUrl": preview.data_url() }))
        }
        CommunityAction::CandidateSkinInstall { id: item, replace } => {
            let item = id(&item)?;
            let root = skins()?;
            let downloaded = candidate.download(item).map_err(account)?;
            candidate_sync::install_downloaded(root, &downloaded, replace).map_err(package)?;
            catalog_response(root)
        }
        CommunityAction::CandidateSkinPackPreview {
            skin_id,
            visibility,
        } => value(
            candidate_community::pack_preview(skins()?, &skin_id, visibility.unwrap_or_default())
                .map_err(package)?,
        ),
        CommunityAction::CandidateSkinAddPreview { skin_id, image } => {
            let root = skins()?;
            let bytes = BASE64.decode(image.as_bytes()).map_err(|_| invalid())?;
            candidate_community::add_preview(root, &skin_id, &bytes).map_err(package)?;
            catalog_response(root)
        }
        CommunityAction::CandidateSkinAddLicense { skin_id, assets } => {
            let root = skins()?;
            candidate_community::add_license(root, &skin_id, &assets).map_err(package)?;
            catalog_response(root)
        }
        CommunityAction::CandidateSkinPublish {
            skin_id,
            id: item,
            name,
            description,
            visibility,
            category,
        } => {
            let item = id(&item)?;
            let root = skins()?;
            value(
                candidate_sync::publish(
                    root,
                    &candidate_sync::state_path(root),
                    candidate,
                    &skin_id,
                    item,
                    name,
                    description,
                    visibility.unwrap_or_default(),
                    category,
                )
                .map_err(|error| match error {
                    CandidateSkinPublishError::Package(code) => package(code),
                    CandidateSkinPublishError::Account(error) => account(error),
                })?,
            )
        }
        CommunityAction::CandidateSkinRate { id: item, stars } => {
            candidate.rate(id(&item)?, stars).map_err(account)?;
            Ok(json!({ "stars": stars }))
        }
        CommunityAction::CandidateSkinSetVisibility {
            id: item,
            visibility,
        } => value(
            candidate
                .set_visibility(id(&item)?, visibility)
                .map_err(account)?,
        ),
        CommunityAction::CandidateSkinSetCategory { id: item, category } => value(
            candidate
                .set_category(id(&item)?, category)
                .map_err(account)?,
        ),
        CommunityAction::CandidateSkinUnpublish { id: item } => {
            let item = id(&item)?;
            candidate_sync::unpublish(&candidate_sync::state_path(skins()?), candidate, item)
                .map_err(account)?;
            Ok(json!({ "deleted": true }))
        }
        CommunityAction::CandidateSkinSync => {
            let root = skins()?;
            value(
                candidate_sync::sync_candidate_skins(
                    root,
                    &candidate_sync::state_path(root),
                    candidate,
                )
                .map_err(account)?,
            )
        }
        CommunityAction::PluginList {
            offset,
            search,
            kind,
            mine,
        } => value(
            services
                .plugins
                .list(offset, &search, kind, mine.unwrap_or(false))
                .map_err(account)?,
        ),
        CommunityAction::PluginDetail { id: item } => {
            value(services.plugins.detail(id(&item)?).map_err(account)?)
        }
        CommunityAction::PluginPackPreview { kind, plugin_id } => value(
            plugin_community::pack_preview(plugins()?, kind, &plugin_id)?,
        ),
        CommunityAction::PluginPublish {
            kind,
            plugin_id,
            id: item,
            name,
            description,
        } => {
            let item = id(&item)?;
            value(services.plugins.publish_installed(
                plugins()?,
                kind,
                &plugin_id,
                item,
                name,
                description,
            )?)
        }
        CommunityAction::PluginInstall {
            id: item,
            kind,
            plugin_id,
        } => {
            let item = id(&item)?;
            value(
                services
                    .plugins
                    .install_publication(plugins()?, item, kind, &plugin_id)?,
            )
        }
        CommunityAction::PluginRate { id: item, stars } => {
            services.plugins.rate(id(&item)?, stars).map_err(account)?;
            Ok(json!({ "stars": stars }))
        }
        CommunityAction::PluginDelete { id: item } => {
            services.plugins.delete(id(&item)?).map_err(account)?;
            Ok(json!({ "deleted": true }))
        }
        CommunityAction::ResourceList {
            kind,
            scope,
            search,
            offset,
        } => {
            let scope = CommunityResourceScope::from_name(&scope).ok_or_else(invalid)?;
            value(
                services
                    .resources
                    .list(kind, scope, &search, offset)
                    .map_err(account)?,
            )
        }
        CommunityAction::ResourceDetail { id: item } => {
            value(services.resources.detail(id(&item)?).map_err(account)?)
        }
        CommunityAction::ResourcePublish {
            id: item,
            kind,
            name,
            description,
            content,
            revision,
        } => value(
            services
                .resources
                .publish(id(&item)?, kind, &name, &description, &content, revision)
                .map_err(account)?,
        ),
        CommunityAction::ResourceApply {
            id: item,
            resource_revision,
        } => value(
            services
                .resources
                .apply(id(&item)?, resource_revision)
                .map_err(account)?,
        ),
        CommunityAction::ResourceSave { id: item, saved } => {
            services
                .resources
                .save(id(&item)?, saved)
                .map_err(account)?;
            Ok(Value::Null)
        }
        CommunityAction::ResourceRate { id: item, stars } => {
            services
                .resources
                .rate(id(&item)?, stars)
                .map_err(account)?;
            Ok(Value::Null)
        }
        CommunityAction::ResourceUnpublish { id: item } => {
            services.resources.delete(id(&item)?).map_err(account)?;
            Ok(Value::Null)
        }
        CommunityAction::Report {
            kind,
            id: item,
            reason,
            detail,
        } => {
            let report =
                CommunityReport::from_labels(kind, &item, &reason, detail.as_deref().unwrap_or(""))
                    .map_err(account)?;
            let anonymous = anonymous().map_err(account)?;
            services
                .report
                .report_with_anonymous(&anonymous, &report)
                .map_err(account)?;
            Ok(Value::Null)
        }
        CommunityAction::KeyboardSkinList {
            offset,
            search,
            mine,
            category,
        } => value(
            keyboard
                .list(offset, &search, mine.unwrap_or(false), category)
                .map_err(account)?,
        ),
        CommunityAction::KeyboardSkinDetail { id: item } => {
            value(keyboard.detail(id(&item)?).map_err(account)?)
        }
        CommunityAction::KeyboardSkinDownload { id: item } => {
            let item = id(&item)?;
            let anonymous = anonymous().map_err(account)?;
            value(
                keyboard
                    .download_with_anonymous(&anonymous, item)
                    .map_err(account)?,
            )
        }
        CommunityAction::KeyboardSkinInstall { id: item, name } => {
            let item = id(&item)?;
            let root = state_root.ok_or_else(invalid)?;
            let anonymous = anonymous().map_err(account)?;
            let design = keyboard
                .download_with_anonymous(&anonymous, item)
                .map_err(account)?;
            super::host::install_downloaded_keyboard_skin(root, item, &name, design)
                .map_err(PluginFailure::code)
        }
        CommunityAction::KeyboardSkinRate { id: item, stars } => {
            keyboard.rate(id(&item)?, stars).map_err(account)?;
            Ok(Value::Null)
        }
        CommunityAction::KeyboardSkinPublish {
            id: item,
            name,
            description,
            design,
            category,
        } => {
            keyboard
                .publish(id(&item)?, &name, &description, &design, category)
                .map_err(account)?;
            Ok(Value::Null)
        }
        CommunityAction::KeyboardSkinSetCategory { id: item, category } => value(
            keyboard
                .set_category(id(&item)?, category)
                .map_err(account)?,
        ),
        CommunityAction::KeyboardSkinUnpublish { id: item } => {
            keyboard.unpublish(id(&item)?).map_err(account)?;
            Ok(Value::Null)
        }
    }
}

/// 原生设置窗口的一次社区操作。请求、各操作的返回值和错误码见头文件。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_community(request: *const u8, length: usize) -> *mut c_char {
    plugin_response(|| {
        // SAFETY: forwarded from the documented caller contract; the length is bounded first.
        let request: CommunityRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_KEYBOARD_SKIN_PUBLISH_BYTES,
                "",
                |bytes| serde_json::from_slice(bytes).map_err(|_| String::new()),
            )
        }
        .map_err(|_| invalid())?;
        // 只有发布键盘皮肤可以超过 4 MiB，其余操作的上限与以前相同。
        if length > MAX_REQUEST_BYTES
            && !matches!(request.action, CommunityAction::KeyboardSkinPublish { .. })
        {
            return Err(invalid());
        }
        let source = session_source(
            request.directory.as_deref(),
            request.legacy_directory.is_some(),
            request.session_store,
        )
        .ok_or_else(invalid)?;
        let directory = match &source {
            Source::Directory(directory) => Some(absolute_directory(directory)?),
            Source::Store(_) => None,
        };
        let legacy = optional_directory(request.legacy_directory.as_deref())?;
        let skins = optional_directory(request.skins_directory.as_deref())?;
        let state_root = optional_directory(request.state_root.as_deref())?;
        // `<state_root>/plugins`, the root `msime_client_plugins` lists and imports into.
        let plugins = PluginRoots::new(request.state_root.as_deref(), None, None, "").installed;
        match source {
            Source::Store(entry) => run(
                &entry.community,
                || Ok(Arc::clone(&entry.anonymous)),
                skins,
                plugins.as_deref(),
                state_root,
                request.action,
            ),
            Source::Directory(_) => {
                let directory = directory.ok_or_else(invalid)?;
                let services = services_for(directory, legacy).map_err(account)?;
                run(
                    &services,
                    || anonymous_session_for(directory),
                    skins,
                    plugins.as_deref(),
                    state_root,
                    request.action,
                )
            }
        }
    })
}

#[cfg(test)]
mod tests;

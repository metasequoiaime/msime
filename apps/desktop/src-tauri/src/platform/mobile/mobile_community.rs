//! Community skin, community resource and AI skin commands, written once for iOS and Android.
//!
//! Both targets used to carry their own copy of these commands, identical apart from the account storage type the services authenticate through. The services live in [`MobileCommunityState`], which each target's setup manages next to its own account state; the session they share is the target's account session.

use msime_client_core::account::{AccountError, BackendAccountClient, BackendAccountSession};
use msime_client_core::community::report::{
    BackendCommunityReportService, CommunityReport, CommunityReportKind, CommunityReportReason,
};
use msime_client_core::community::resource::{
    BackendCommunityResourceService, CommunityResource, CommunityResourceApplication,
    CommunityResourceContent, CommunityResourceKind, CommunityResourcePage,
    CommunityResourcePublication, CommunityResourceScope,
};
use msime_client_core::community::resource_library::{
    CommunityResourceLibraryError, CommunityResourceLibraryStore,
};
use msime_client_core::preferences::TouchKeyboardSkinDesign;
use msime_client_core::skin::ai::{AiSkinError, AiSkinProposal, BackendAiSkinService};
use msime_client_core::skin::category::SkinCategory;
use msime_client_core::skin::community::{
    BackendCommunitySkinService, CommunitySkin, CommunitySkinPage,
};
use msime_client_core::skin::custom_library::{
    CustomSkinLibraryError, CustomSkinLibraryStore, SavedTouchKeyboardSkin,
};
use msime_client_core::skin::keyboard_trial::{
    KeyboardSkinTrial, KeyboardSkinTrialError, KeyboardSkinTrialStore,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::{Emitter, State, Wry};

use super::mobile_ai_skin_requests::AiSkinRequests;
use super::MobileStorage;

type Session = BackendAccountSession<BackendAccountClient, MobileStorage>;
type CommunityService = BackendCommunitySkinService<BackendAccountClient, MobileStorage>;
type CommunityResourceService =
    BackendCommunityResourceService<BackendAccountClient, MobileStorage>;
type AiSkinService = BackendAiSkinService<BackendAccountClient, MobileStorage>;
type CommunityReportService = BackendCommunityReportService<BackendAccountClient, MobileStorage>;

pub(crate) struct MobileCommunityState {
    community: Arc<CommunityService>,
    resources: Arc<CommunityResourceService>,
    ai_skin: Arc<AiSkinService>,
    reports: Arc<CommunityReportService>,
    ai_skin_requests: AiSkinRequests,
}

impl MobileCommunityState {
    /// Builds the four services over the account session. `client` is the one the session was built with; the resource, AI skin and report services each get a client of their own.
    pub(crate) fn new(
        client: BackendAccountClient,
        session: &Arc<Session>,
    ) -> Result<Self, AccountError> {
        let community = Arc::new(BackendCommunitySkinService::new(
            client,
            Arc::clone(session),
        ));
        let resources = Arc::new(BackendCommunityResourceService::new(
            BackendAccountClient::new()?,
            Arc::clone(session),
        ));
        let ai_skin = Arc::new(BackendAiSkinService::new(
            BackendAccountClient::new()?,
            Arc::clone(session),
        ));
        let reports = Arc::new(BackendCommunityReportService::new(
            BackendAccountClient::new()?,
            Arc::clone(session),
        ));
        Ok(Self {
            community,
            resources,
            ai_skin,
            reports,
            ai_skin_requests: AiSkinRequests::default(),
        })
    }
}

fn community_error(error: AccountError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            AccountError::Invalid => "community_invalid",
            AccountError::Unauthorized => "community_unauthorized",
            AccountError::Forbidden => "community_forbidden",
            AccountError::NotFound => "community_not_found",
            AccountError::RateLimited => "community_rate_limited",
            AccountError::Cancelled => "community_cancelled",
            AccountError::Storage => "community_storage",
            AccountError::Conflict => "community_conflict",
            AccountError::Unavailable => "community_unavailable",
            AccountError::BlockedContent => "community_blocked_content",
            AccountError::ScreeningUnavailable => "community_screening_unavailable",
            AccountError::Banned => "community_account_banned",
        },
    }
}

fn ai_skin_error(error: AiSkinError) -> crate::CommandError {
    let code = match error {
        AiSkinError::Cancelled => "ai_skin_cancelled",
        AiSkinError::InvalidResponse => "ai_skin_invalid",
        AiSkinError::Account(error) => error.code(),
    };
    crate::CommandError { code }
}

fn valid_ai_skin_request_id(value: &str) -> bool {
    msime_client_core::is_bounded_ascii_identifier(value, 96)
}

fn community_id(value: &str) -> Result<uuid::Uuid, crate::CommandError> {
    uuid::Uuid::parse_str(value).map_err(|_| crate::CommandError {
        code: "community_invalid",
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AiSkinProgress {
    request_id: String,
    completed: usize,
}

#[tauri::command]
pub async fn ai_skin_generate(
    app: tauri::AppHandle<Wry>,
    state: State<'_, MobileCommunityState>,
    request_id: String,
    prompt: String,
) -> Result<Vec<AiSkinProposal>, crate::CommandError> {
    if !valid_ai_skin_request_id(&request_id) {
        return Err(crate::CommandError {
            code: "ai_skin_invalid",
        });
    }
    let cancelled = state
        .ai_skin_requests
        .begin(&request_id)
        .map_err(|code| crate::CommandError { code })?;
    let service = Arc::clone(&state.ai_skin);
    let progress_app = app.clone();
    let progress_request_id = request_id.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        service.generate(&prompt, &cancelled, move |completed| {
            let _ = progress_app.emit(
                "ai-skin-progress",
                AiSkinProgress {
                    request_id: progress_request_id.clone(),
                    completed,
                },
            );
        })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "ai_skin_unavailable",
    })?
    .map_err(ai_skin_error);
    state.ai_skin_requests.finish(&request_id);
    result
}

#[tauri::command]
pub async fn ai_skin_cancel(
    state: State<'_, MobileCommunityState>,
    request_id: String,
) -> Result<(), crate::CommandError> {
    if !valid_ai_skin_request_id(&request_id) {
        return Err(crate::CommandError {
            code: "ai_skin_invalid",
        });
    }
    state
        .ai_skin_requests
        .cancel(&request_id)
        .map_err(|code| crate::CommandError { code })
}

async fn service_call<T, S, F>(service: Arc<S>, operation: F) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    S: Send + Sync + 'static,
    F: FnOnce(&S) -> Result<T, AccountError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || operation(&service))
        .await
        .map_err(|_| crate::CommandError {
            code: "community_unavailable",
        })?
        .map_err(community_error)
}

fn custom_skin_error(error: CustomSkinLibraryError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            CustomSkinLibraryError::Full => "community_skin_library_full",
            CustomSkinLibraryError::InvalidName => "community_skin_invalid_name",
            CustomSkinLibraryError::DuplicateName => "community_skin_duplicate_name",
            CustomSkinLibraryError::NotFound => "community_skin_not_found",
            CustomSkinLibraryError::Json(_) | CustomSkinLibraryError::Invalid => {
                "community_skin_library_format"
            }
            CustomSkinLibraryError::Io(_) => "community_storage",
        },
    }
}

fn trial_error(error: KeyboardSkinTrialError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            KeyboardSkinTrialError::Io(_) | KeyboardSkinTrialError::Preferences(_) => {
                "community_storage"
            }
            KeyboardSkinTrialError::Json(_) | KeyboardSkinTrialError::Invalid => {
                "community_trial_format"
            }
        },
    }
}

async fn storage_call<T, E, F, M>(operation: F, map_error: M) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    E: Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
    M: FnOnce(E) -> crate::CommandError + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|_| crate::CommandError {
            code: "community_storage",
        })?
        .map_err(map_error)
}

fn resource_library_error(error: CommunityResourceLibraryError) -> crate::CommandError {
    crate::CommandError {
        code: match error {
            CommunityResourceLibraryError::Io(_) => "community_storage",
            CommunityResourceLibraryError::Json(_) | CommunityResourceLibraryError::Invalid => {
                "community_resource_library_format"
            }
        },
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommunitySkinDownloadResponse {
    skin: SavedTouchKeyboardSkin,
    trial: KeyboardSkinTrial,
}

#[tauri::command]
pub async fn community_skin_list(
    state: State<'_, MobileCommunityState>,
    offset: usize,
    search: String,
    mine: Option<bool>,
    // 不传或为 null 时列出全部分类。
    category: Option<SkinCategory>,
) -> Result<CommunitySkinPage, crate::CommandError> {
    service_call(Arc::clone(&state.community), move |service| {
        service.list(offset, &search, mine.unwrap_or(false), category)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_detail(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<CommunitySkin, crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.community), move |service| {
        service.detail(id)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_download(
    state: State<'_, MobileCommunityState>,
    library: State<'_, CustomSkinLibraryStore>,
    trials: State<'_, KeyboardSkinTrialStore>,
    id: String,
    name: String,
) -> Result<CommunitySkinDownloadResponse, crate::CommandError> {
    let id = community_id(&id)?;
    let service = Arc::clone(&state.community);
    let library = library.inner().clone();
    let trials = trials.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let design = service.download(id).map_err(community_error)?;
        let (trial, _) = trials.begin(&name, design.clone()).map_err(trial_error)?;
        let skin = match library.import_download(id, &name, design) {
            Ok(skin) => skin,
            Err(error) => {
                let _ = trials.finish(trial.id, false);
                return Err(custom_skin_error(error));
            }
        };
        Ok(CommunitySkinDownloadResponse { skin, trial })
    })
    .await
    .map_err(|_| crate::CommandError {
        code: "community_unavailable",
    })?
}

#[tauri::command]
pub async fn community_skin_rate(
    state: State<'_, MobileCommunityState>,
    id: String,
    stars: u8,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.community), move |service| {
        service.rate(id, stars)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_publish(
    state: State<'_, MobileCommunityState>,
    id: String,
    name: String,
    description: String,
    design: TouchKeyboardSkinDesign,
    // 不传时不发送分类，由服务端归入默认分类。
    category: Option<SkinCategory>,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.community), move |service| {
        service.publish(id, &name, &description, &design, category)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_set_category(
    state: State<'_, MobileCommunityState>,
    id: String,
    category: SkinCategory,
) -> Result<CommunitySkin, crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.community), move |service| {
        service.set_category(id, category)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_unpublish(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.community), move |service| {
        service.unpublish(id)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_finish_trial(
    trials: State<'_, KeyboardSkinTrialStore>,
    id: String,
    keep: bool,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    let trials = trials.inner().clone();
    storage_call(move || trials.finish(id, keep).map(|_| ()), trial_error).await
}

fn resource_scope(value: &str) -> Result<CommunityResourceScope, crate::CommandError> {
    match value {
        "" => Ok(CommunityResourceScope::All),
        "mine" => Ok(CommunityResourceScope::Mine),
        "saved" => Ok(CommunityResourceScope::Saved),
        _ => Err(crate::CommandError {
            code: "community_invalid",
        }),
    }
}

#[tauri::command]
pub async fn community_resource_list(
    state: State<'_, MobileCommunityState>,
    kind: CommunityResourceKind,
    scope: String,
    search: String,
    offset: usize,
) -> Result<CommunityResourcePage, crate::CommandError> {
    let scope = resource_scope(&scope)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.list(kind, scope, &search, offset)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_detail(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<CommunityResource, crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.detail(id)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_publish(
    state: State<'_, MobileCommunityState>,
    id: String,
    kind: CommunityResourceKind,
    name: String,
    description: String,
    content: CommunityResourceContent,
    revision: u32,
) -> Result<CommunityResourcePublication, crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.publish(id, kind, &name, &description, &content, revision)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_apply(
    state: State<'_, MobileCommunityState>,
    id: String,
    resource_revision: u32,
) -> Result<CommunityResourceApplication, crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.apply(id, resource_revision)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_save(
    state: State<'_, MobileCommunityState>,
    id: String,
    saved: bool,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.save(id, saved)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_rate(
    state: State<'_, MobileCommunityState>,
    id: String,
    stars: u8,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.rate(id, stars)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_unpublish(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    service_call(Arc::clone(&state.resources), move |service| {
        service.delete(id)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_store_reply(
    library: State<'_, CommunityResourceLibraryStore>,
    item: CommunityResource,
) -> Result<(), crate::CommandError> {
    let library = library.inner().clone();
    storage_call(move || library.save_reply(item), resource_library_error).await
}

#[tauri::command]
pub async fn community_resource_remove_reply(
    library: State<'_, CommunityResourceLibraryStore>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    let library = library.inner().clone();
    storage_call(move || library.remove(id), resource_library_error).await
}

/// Files one report on another user's skin, dictionary or reply template. `reason` is the label of one of [`CommunityReportReason::ALL`]; anything else, a malformed id or a detail past the limit is `community_invalid`.
#[tauri::command]
pub async fn community_report(
    state: State<'_, MobileCommunityState>,
    kind: CommunityReportKind,
    id: String,
    reason: String,
    detail: Option<String>,
) -> Result<(), crate::CommandError> {
    let item_id = community_id(&id)?;
    let reason = CommunityReportReason::from_label(&reason).ok_or(crate::CommandError {
        code: "community_invalid",
    })?;
    service_call(Arc::clone(&state.reports), move |service| {
        let report = CommunityReport::new(kind, item_id, reason, detail.as_deref().unwrap_or(""))?;
        service.report(&report)
    })
    .await
}

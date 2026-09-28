//! Community skin, community resource and AI skin commands, written once for iOS and Android.
//!
//! Both targets used to carry their own copy of these commands, identical apart from the account storage type the services authenticate through. The services live in [`MobileCommunityState`], which each target's setup manages next to its own account state; the session they share is the target's account session.

use msime_client_core::account::{AccountError, BackendAccountClient, BackendAccountSession};
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
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, State, Wry};

use super::MobileStorage;

type Session = BackendAccountSession<BackendAccountClient, MobileStorage>;
type CommunityService = BackendCommunitySkinService<BackendAccountClient, MobileStorage>;
type CommunityResourceService =
    BackendCommunityResourceService<BackendAccountClient, MobileStorage>;
type AiSkinService = BackendAiSkinService<BackendAccountClient, MobileStorage>;

pub(crate) struct MobileCommunityState {
    community: Arc<CommunityService>,
    resources: Arc<CommunityResourceService>,
    ai_skin: Arc<AiSkinService>,
    ai_skin_requests: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

impl MobileCommunityState {
    /// Builds the three services over the account session. `client` is the one the session was built with; the resource and AI skin services each get a client of their own.
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
        Ok(Self {
            community,
            resources,
            ai_skin,
            ai_skin_requests: Arc::new(Mutex::new(HashMap::new())),
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
    (1..=96).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
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
    let cancelled = Arc::new(AtomicBool::new(false));
    {
        let mut requests = state
            .ai_skin_requests
            .lock()
            .map_err(|_| crate::CommandError {
                code: "ai_skin_unavailable",
            })?;
        if requests
            .insert(request_id.clone(), Arc::clone(&cancelled))
            .is_some()
        {
            return Err(crate::CommandError {
                code: "ai_skin_busy",
            });
        }
    }
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
    if let Ok(mut requests) = state.ai_skin_requests.lock() {
        requests.remove(&request_id);
    }
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
    let requests = state
        .ai_skin_requests
        .lock()
        .map_err(|_| crate::CommandError {
            code: "ai_skin_unavailable",
        })?;
    if let Some(cancelled) = requests.get(&request_id) {
        cancelled.store(true, Ordering::Release);
    }
    Ok(())
}

async fn community_call<T, F>(
    state: State<'_, MobileCommunityState>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    F: FnOnce(&CommunityService) -> Result<T, AccountError> + Send + 'static,
{
    let service = Arc::clone(&state.community);
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
) -> Result<CommunitySkinPage, crate::CommandError> {
    community_call(state, move |service| service.list(offset, &search)).await
}

#[tauri::command]
pub async fn community_skin_detail(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<CommunitySkin, crate::CommandError> {
    let id = community_id(&id)?;
    community_call(state, move |service| service.detail(id)).await
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
    community_call(state, move |service| service.rate(id, stars)).await
}

#[tauri::command]
pub async fn community_skin_publish(
    state: State<'_, MobileCommunityState>,
    id: String,
    name: String,
    description: String,
    design: TouchKeyboardSkinDesign,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    community_call(state, move |service| {
        service.publish(id, &name, &description, &design)
    })
    .await
}

#[tauri::command]
pub async fn community_skin_unpublish(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    community_call(state, move |service| service.unpublish(id)).await
}

#[tauri::command]
pub async fn community_skin_finish_trial(
    trials: State<'_, KeyboardSkinTrialStore>,
    id: String,
    keep: bool,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    let trials = trials.inner().clone();
    tauri::async_runtime::spawn_blocking(move || trials.finish(id, keep).map(|_| ()))
        .await
        .map_err(|_| crate::CommandError {
            code: "community_storage",
        })?
        .map_err(trial_error)
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

async fn resource_call<T, F>(
    state: State<'_, MobileCommunityState>,
    operation: F,
) -> Result<T, crate::CommandError>
where
    T: Send + 'static,
    F: FnOnce(&CommunityResourceService) -> Result<T, AccountError> + Send + 'static,
{
    let service = Arc::clone(&state.resources);
    tauri::async_runtime::spawn_blocking(move || operation(&service))
        .await
        .map_err(|_| crate::CommandError {
            code: "community_unavailable",
        })?
        .map_err(community_error)
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
    resource_call(state, move |service| {
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
    resource_call(state, move |service| service.detail(id)).await
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
    resource_call(state, move |service| {
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
    resource_call(state, move |service| service.apply(id, resource_revision)).await
}

#[tauri::command]
pub async fn community_resource_save(
    state: State<'_, MobileCommunityState>,
    id: String,
    saved: bool,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    resource_call(state, move |service| service.save(id, saved)).await
}

#[tauri::command]
pub async fn community_resource_rate(
    state: State<'_, MobileCommunityState>,
    id: String,
    stars: u8,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    resource_call(state, move |service| service.rate(id, stars)).await
}

#[tauri::command]
pub async fn community_resource_unpublish(
    state: State<'_, MobileCommunityState>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    resource_call(state, move |service| service.delete(id)).await
}

#[tauri::command]
pub async fn community_resource_store_reply(
    library: State<'_, CommunityResourceLibraryStore>,
    item: CommunityResource,
) -> Result<(), crate::CommandError> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.save_reply(item))
        .await
        .map_err(|_| crate::CommandError {
            code: "community_storage",
        })?
        .map_err(resource_library_error)
}

#[tauri::command]
pub async fn community_resource_remove_reply(
    library: State<'_, CommunityResourceLibraryStore>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.remove(id))
        .await
        .map_err(|_| crate::CommandError {
            code: "community_storage",
        })?
        .map_err(resource_library_error)
}

//! 社区词包与回复模板：浏览、收藏、评分、发布、删除自己的发布，以及把词包导入账号云端词库。iOS、Android 与 Windows 共用这些命令。
//!
//! 服务经本平台的账号会话认证，各平台建账号状态时一并登记 [`CommunityResourceState`]。把回复模板存进本机「高情商回复」键盘只有移动端有，那两个命令留在 `mobile_community`。

use super::account_helpers::{community_id, community_service_call};
use msime_client_core::account::{AccountError, BackendAccountClient, BackendAccountSession};
use msime_client_core::community::resource::{
    BackendCommunityResourceService, CommunityResource, CommunityResourceApplication,
    CommunityResourceContent, CommunityResourceKind, CommunityResourcePage,
    CommunityResourcePublication, CommunityResourceScope,
};
use std::sync::Arc;
use tauri::State;

/// 本平台账号会话的存储类型，与各平台 `AccountState` 持有的会话一致。
#[cfg(any(target_os = "ios", target_os = "android"))]
type Storage = super::mobile::MobileStorage;
#[cfg(target_os = "windows")]
type Storage = super::desktop::desktop_account::Storage;

type Service = BackendCommunityResourceService<BackendAccountClient, Storage>;

pub(crate) struct CommunityResourceState {
    service: Arc<Service>,
}

impl CommunityResourceState {
    /// 社区资源服务用自己的 HTTP 客户端，经同一个账号会话认证。
    pub(crate) fn new(
        session: &Arc<BackendAccountSession<BackendAccountClient, Storage>>,
    ) -> Result<Self, AccountError> {
        Ok(Self {
            service: Arc::new(BackendCommunityResourceService::new(
                BackendAccountClient::new()?,
                Arc::clone(session),
            )),
        })
    }
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
    state: State<'_, CommunityResourceState>,
    kind: CommunityResourceKind,
    scope: String,
    search: String,
    offset: usize,
) -> Result<CommunityResourcePage, crate::CommandError> {
    let scope = resource_scope(&scope)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.list(kind, scope, &search, offset)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_detail(
    state: State<'_, CommunityResourceState>,
    id: String,
) -> Result<CommunityResource, crate::CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.detail(id)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_publish(
    state: State<'_, CommunityResourceState>,
    id: String,
    kind: CommunityResourceKind,
    name: String,
    description: String,
    content: CommunityResourceContent,
    revision: u32,
) -> Result<CommunityResourcePublication, crate::CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.publish(id, kind, &name, &description, &content, revision)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_apply(
    state: State<'_, CommunityResourceState>,
    id: String,
    resource_revision: u32,
) -> Result<CommunityResourceApplication, crate::CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.apply(id, resource_revision)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_save(
    state: State<'_, CommunityResourceState>,
    id: String,
    saved: bool,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.save(id, saved)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_rate(
    state: State<'_, CommunityResourceState>,
    id: String,
    stars: u8,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.rate(id, stars)
    })
    .await
}

#[tauri::command]
pub async fn community_resource_unpublish(
    state: State<'_, CommunityResourceState>,
    id: String,
) -> Result<(), crate::CommandError> {
    let id = community_id(&id)?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.delete(id)
    })
    .await
}

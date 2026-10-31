//! 屏蔽社区作者，给 macOS、Windows 和 Linux 的设置应用用。
//!
//! 页面发作品的类别和 id（屏蔽）、不带参数（列出已屏蔽的作者）或屏蔽记录的 id（取消屏蔽），三项都交给 [`BackendCommunityBlockService`]，与其他社区服务用同一个账号会话。和举报不同，屏蔽只认登录的水杉账号，不退回本机匿名账号：没有登录时答 `community_unauthorized`，页面据此请用户登录。屏蔽之后服务端不再在这个账号的社区列表里列出那位作者的作品，页面只需重读列表。

use crate::platform::account_helpers::community_service_call;
use crate::platform::desktop::desktop_account::Storage;
use crate::CommandError;
use msime_client_services::account::BackendAccountClient;
use msime_client_services::community::block::{BackendCommunityBlockService, CommunityBlock};
use msime_client_services::community::report::CommunityReportKind;
use std::sync::Arc;
use tauri::State;

type CommunityBlockService = BackendCommunityBlockService<BackendAccountClient, Storage>;

pub(crate) struct CommunityBlockState {
    pub(crate) service: Arc<CommunityBlockService>,
}

/// 屏蔽 `kind` 类作品 `id` 的作者。格式不对的 id 和自己的作品答 `community_invalid`，达到屏蔽上限答 `community_conflict`。
async fn block(
    service: Arc<CommunityBlockService>,
    kind: CommunityReportKind,
    id: String,
) -> Result<CommunityBlock, CommandError> {
    community_service_call(service, move |service| service.block(kind, &id)).await
}

/// 登录账号屏蔽的作者，最近的在前。
async fn list(service: Arc<CommunityBlockService>) -> Result<Vec<CommunityBlock>, CommandError> {
    community_service_call(service, |service| service.list()).await
}

/// 取消屏蔽记录 `id`；已经取消过的也按成功处理。
async fn unblock(service: Arc<CommunityBlockService>, id: String) -> Result<(), CommandError> {
    community_service_call(service, move |service| service.unblock(&id)).await
}

#[tauri::command]
pub async fn community_block(
    state: State<'_, CommunityBlockState>,
    kind: CommunityReportKind,
    id: String,
) -> Result<CommunityBlock, CommandError> {
    block(Arc::clone(&state.service), kind, id).await
}

#[tauri::command]
pub async fn community_block_list(
    state: State<'_, CommunityBlockState>,
) -> Result<Vec<CommunityBlock>, CommandError> {
    list(Arc::clone(&state.service)).await
}

#[tauri::command]
pub async fn community_unblock(
    state: State<'_, CommunityBlockState>,
    id: String,
) -> Result<(), CommandError> {
    unblock(Arc::clone(&state.service), id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_services::account::{
        AccountSessionFileLayout, BackendAccountSession, FileAccountSessionStorage,
    };

    const ITEM: &str = "10000000-0000-4000-8000-000000000001";

    /// 一个没有登录会话的目录：服务只读得到「没有人登录」，请求不会发出去。
    fn signed_out(directory: &std::path::Path) -> Arc<CommunityBlockService> {
        let client = BackendAccountClient::new().unwrap();
        let storage = FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native);
        let session = Arc::new(BackendAccountSession::new(client.clone(), storage));
        Arc::new(BackendCommunityBlockService::new(client, session))
    }

    #[test]
    fn every_operation_asks_for_a_signed_in_account() {
        let directory = tempfile::tempdir().unwrap();
        let service = signed_out(directory.path());
        let code = |result: Result<(), CommandError>| result.err().map(|error| error.code);

        let blocked = tauri::async_runtime::block_on(block(
            Arc::clone(&service),
            CommunityReportKind::Skins,
            ITEM.to_string(),
        ));
        assert_eq!(code(blocked.map(|_| ())), Some("community_unauthorized"));
        let listed = tauri::async_runtime::block_on(list(Arc::clone(&service)));
        assert_eq!(code(listed.map(|_| ())), Some("community_unauthorized"));
        let unblocked = tauri::async_runtime::block_on(unblock(service, ITEM.to_string()));
        assert_eq!(code(unblocked), Some("community_unauthorized"));
        // 没有登录时不在会话目录里写任何东西。
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn a_malformed_id_is_refused_before_the_session_is_read() {
        let directory = tempfile::tempdir().unwrap();
        let service = signed_out(directory.path());

        let blocked = tauri::async_runtime::block_on(block(
            Arc::clone(&service),
            CommunityReportKind::Plugins,
            "not-a-uuid".to_string(),
        ));
        assert_eq!(
            blocked.err().map(|error| error.code),
            Some("community_invalid")
        );
        let unblocked = tauri::async_runtime::block_on(unblock(
            service,
            "00000000-0000-0000-0000-000000000000".to_string(),
        ));
        assert_eq!(
            unblocked.err().map(|error| error.code),
            Some("community_invalid")
        );
    }

    #[test]
    fn the_page_names_kinds_as_the_server_does() {
        // 页面发的 kind 字符串原样反序列化成服务端的类别。
        for (name, kind) in [
            ("skins", CommunityReportKind::Skins),
            ("candidate-skins", CommunityReportKind::CandidateSkins),
            ("plugins", CommunityReportKind::Plugins),
            ("dictionaries", CommunityReportKind::Dictionaries),
            ("replies", CommunityReportKind::Replies),
        ] {
            assert_eq!(
                serde_json::from_value::<CommunityReportKind>(serde_json::json!(name)).unwrap(),
                kind
            );
        }
    }
}

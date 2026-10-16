//! Reporting another user's community item to the moderators, for the macOS, Windows and Linux shells.
//!
//! The page sends the kind, the item id, one of the fixed reasons by its label and an optional detail; the report is filed with the account session the other community services use, so the device's anonymous account counts.

use crate::platform::account_helpers::community_service_call;
use crate::platform::desktop::desktop_account::Storage;
use crate::CommandError;
use msime_client_services::account::BackendAccountClient;
use msime_client_services::community::report::{
    BackendCommunityReportService, CommunityReport, CommunityReportKind,
};
use std::sync::Arc;
use tauri::State;

pub(crate) struct CommunityReportState {
    pub(crate) service: Arc<BackendCommunityReportService<BackendAccountClient, Storage>>,
}

/// 提交一条举报。`reason` 必须是 [`msime_client_services::community::report::CommunityReportReason::ALL`] 中某一项的标签；其他值、格式不对的 id 或超出长度上限的详情都返回 `community_invalid`（见 [`CommunityReport::from_labels`]）。
#[tauri::command]
pub async fn community_report(
    state: State<'_, CommunityReportState>,
    kind: CommunityReportKind,
    id: String,
    reason: String,
    detail: Option<String>,
) -> Result<(), CommandError> {
    let report = CommunityReport::from_labels(kind, &id, &reason, detail.as_deref().unwrap_or(""))
        .map_err(|error| CommandError {
            code: error.community_code(),
        })?;
    community_service_call(Arc::clone(&state.service), move |service| {
        service.report(&report)
    })
    .await
}

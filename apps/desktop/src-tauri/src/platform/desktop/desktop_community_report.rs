//! Reporting another user's community item to the moderators, for the macOS, Windows and Linux shells.
//!
//! The page sends the kind, the item id, one of the fixed reasons by its label and an optional detail; the report is filed with the account session the other community services use, so the device's anonymous account counts.

use crate::platform::account_helpers::{community_id, community_service_call};
use crate::platform::desktop::desktop_account::Storage;
use crate::CommandError;
use msime_client_core::account::BackendAccountClient;
use msime_client_core::community::report::{
    BackendCommunityReportService, CommunityReport, CommunityReportKind, CommunityReportReason,
};
use std::sync::Arc;
use tauri::State;

pub(crate) struct CommunityReportState {
    pub(crate) service: Arc<BackendCommunityReportService<BackendAccountClient, Storage>>,
}

/// Files one report. `reason` is the label of one of [`CommunityReportReason::ALL`]; anything else, a malformed id or a detail past the limit is `community_invalid`.
#[tauri::command]
pub async fn community_report(
    state: State<'_, CommunityReportState>,
    kind: CommunityReportKind,
    id: String,
    reason: String,
    detail: Option<String>,
) -> Result<(), CommandError> {
    let item_id = community_id(&id)?;
    let reason = CommunityReportReason::from_label(&reason).ok_or(CommandError {
        code: "community_invalid",
    })?;
    community_service_call(Arc::clone(&state.service), move |service| {
        let report = CommunityReport::new(kind, item_id, reason, detail.as_deref().unwrap_or(""))?;
        service.report(&report)
    })
    .await
}

//! Reporting another user's community item to the moderators, for the macOS, Windows and Linux shells.
//!
//! The page sends the kind, the item id, one of the fixed reasons by its label and an optional detail; the report is filed with the account session the other community services use, so the device's anonymous account counts.

use crate::platform::account_helpers::community_service_call;
use crate::platform::desktop::desktop_account::Storage;
use crate::CommandError;
use msime_client_core::account::BackendAccountClient;
use msime_client_core::community::report::{
    BackendCommunityReportService, CommunityReport, CommunityReportKind,
};
use std::sync::Arc;
use tauri::State;

pub(crate) struct CommunityReportState {
    pub(crate) service: Arc<BackendCommunityReportService<BackendAccountClient, Storage>>,
}

/// Files one report. `reason` is the label of one of [`msime_client_core::community::report::CommunityReportReason::ALL`]; anything else, a malformed id or a detail past the limit is `community_invalid` ([`CommunityReport::from_labels`]).
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

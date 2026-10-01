//! Community moderation decisions for hosts that send their community requests through their own HTTP stack (iOS, Android, HarmonyOS): the fixed report reasons, the validated report request, and how a refused upload is told to the user.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. The requests themselves stay with the host, because only the surrounding platform's HTTPS stack and session can send them; what is decided here is what every host must decide the same way.

use super::with_bounded_bytes;
use crate::*;
use msime_client_core::account::AccountError;
use msime_client_core::community::report::{
    CommunityReport, CommunityReportKind, CommunityReportReason,
};
use msime_client_core::uuid::Uuid;

const MAX_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum ModerationRequest {
    /// The report reasons in the order a report dialog lists them.
    Reasons,
    /// A report to validate and turn into the request the host sends.
    Report {
        kind: CommunityReportKind,
        item_id: String,
        reason: String,
        #[serde(default)]
        detail: String,
    },
    /// A failed response the host received from a community request.
    Error {
        status: u16,
        #[serde(default)]
        body: String,
    },
}

/// Community moderation helpers for hosts that perform community HTTP themselves. Operations: `reasons`, `report` and `error`; see the header for each request and value.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_community_moderation(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: forwarded from the documented caller contract.
        let request: ModerationRequest = unsafe {
            with_bounded_bytes(
                request,
                length,
                MAX_REQUEST_BYTES,
                "invalid request buffer",
                |bytes| {
                    serde_json::from_slice(bytes).map_err(|_| "invalid request document".to_owned())
                },
            )?
        };
        match request {
            ModerationRequest::Reasons => Ok(json!(CommunityReportReason::ALL
                .iter()
                .map(|reason| reason.label())
                .collect::<Vec<_>>())),
            ModerationRequest::Report {
                kind,
                item_id,
                reason,
                detail,
            } => {
                let item_id = Uuid::parse_str(&item_id).map_err(|_| "community_invalid")?;
                let reason =
                    CommunityReportReason::from_label(&reason).ok_or("community_invalid")?;
                let report = CommunityReport::new(kind, item_id, reason, &detail)
                    .map_err(|_| "community_invalid")?;
                Ok(json!({
                    "method": "POST",
                    "path": "/v1/community/reports",
                    "body": report,
                }))
            }
            ModerationRequest::Error { status, body } => {
                let error = AccountError::from_http_response(status, body.as_bytes());
                Ok(json!({
                    "code": error.code(),
                    "message": error.moderation_message(),
                    "retry": matches!(
                        error,
                        AccountError::ScreeningUnavailable
                            | AccountError::RateLimited
                            | AccountError::Unavailable
                    ),
                }))
            }
        }
    })
}

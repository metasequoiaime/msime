//! 数据入口只适配宿主选项与脱敏结果，轮次、认证和维护租约仍归共享服务。

use super::*;
use msime_client_core::cloud::learning_e2e::SealError;
use msime_client_services::account::learning_sync::round::{
    DownloadOutcome, DownloadReport, RoundError, RoundReport,
};
use msime_client_services::account::learning_sync::{state::StateError, PendingError};
use msime_engine::LearningSyncReadError;

pub(super) use crate::learning_context::Context;

/// 固定错误码；绝不序列化底层错误的 Display、原始正文或密钥串。
pub(super) fn failure(error: RoundError) -> Failure {
    let code = match error {
        RoundError::Account(error) => error.code().to_owned(),
        RoundError::Learning(error) => error.code().to_owned(),
        RoundError::Http {
            status,
            retry_after_seconds,
        } => {
            return Failure {
                code: "learning_http".to_owned(),
                retry_after_seconds,
                http_status: Some(status),
            }
        }
        RoundError::Busy => "learning_busy".to_owned(),
        RoundError::NotReady => "learning_waiting_approval".to_owned(),
        RoundError::KeyChanged => "learning_key_rotated".to_owned(),
        RoundError::InvalidMetadata => "learning_invalid_metadata".to_owned(),
        RoundError::Local => "learning_local_unavailable".to_owned(),
        RoundError::State(error) => match error {
            StateError::Unavailable => "learning_state_unavailable",
            StateError::Invalid => "learning_state_invalid",
            StateError::Rollback => "learning_rollback",
            StateError::NotVerified => "learning_not_verified",
            StateError::Exhausted => "learning_generation_exhausted",
        }
        .to_owned(),
        RoundError::Pending(error) => match error {
            PendingError::Unavailable => "learning_pending_unavailable",
            PendingError::Invalid => "learning_pending_invalid",
            PendingError::Full => "learning_pending_full",
        }
        .to_owned(),
        RoundError::Stream(LearningSyncReadError::TooLarge) => "learning_too_large".to_owned(),
        RoundError::Stream(_) => "learning_stream_invalid".to_owned(),
        RoundError::Seal(error) => match error {
            SealError::NotReady => "learning_waiting_approval",
            SealError::InvalidGeneration => "learning_invalid_generation",
            SealError::TooLarge => "learning_too_large",
            SealError::RandomUnavailable => "learning_random_unavailable",
        }
        .to_owned(),
    };
    code.into()
}

pub(super) fn round_report(report: RoundReport) -> Value {
    let mut value = json!({ "uploaded": report.uploaded, "upload_unchanged": report.upload_unchanged,
        "dropped_weights": report.dropped_weights,
        "upload_error": report.upload_error.map(|error| failure(error).json()),
        "downloads": downloads(report.downloads), "pending": downloads(report.pending) });
    if report.upload_paused {
        value["upload_paused"] = json!(true);
    }
    value
}

pub(super) fn downloads(reports: Vec<DownloadReport>) -> Vec<Value> {
    reports
        .into_iter()
        .map(|report| {
            let outcome = match report.outcome {
                DownloadOutcome::Applied => "applied",
                DownloadOutcome::AlreadyApplied => "already_applied",
                DownloadOutcome::Undecryptable => "undecryptable",
                DownloadOutcome::Rollback => "rollback",
                DownloadOutcome::Pending => "pending",
                DownloadOutcome::MergeFailed => "merge_failed",
                DownloadOutcome::Abandoned => "abandoned",
                DownloadOutcome::Http { .. } => "http",
            };
            let mut value = json!({ "device_id": report.device_id, "generation": report.generation,
            "outcome": outcome, "dictionary_mismatch": report.dictionary_mismatch });
            if let DownloadOutcome::Http {
                status,
                retry_after_seconds,
            } = report.outcome
            {
                value["http_status"] = json!(status);
                if let Some(seconds) = retry_after_seconds {
                    value["retry_after_seconds"] = json!(seconds);
                }
            }
            value
        })
        .collect()
}

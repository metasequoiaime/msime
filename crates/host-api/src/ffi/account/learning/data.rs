//! 数据入口只适配宿主选项与脱敏结果，轮次、认证和维护租约仍归共享服务。

use super::*;
use msime_client_core::cloud::learning_e2e::SealError;
use msime_client_services::account::learning_sync::round::{
    DownloadOutcome, DownloadReport, RoundContext, RoundError, RoundReport,
};
use msime_client_services::account::learning_sync::{state::StateError, PendingError};
use msime_engine::{LearningSyncReadError, RuntimePaths};

pub(super) struct Context {
    options: crate::EngineOptions,
    paths: RuntimePaths,
    state: PathBuf,
    content_id: String,
    main_dictionary: bool,
}

impl Context {
    pub(super) fn parse(mut document: Value) -> Result<Self, Failure> {
        let invalid = || Failure::from("learning_invalid_options".to_owned());
        // 与词库管理入口相同，Linux 宿主附带的候选皮肤目录不属于 Engine 选项。
        if let Some(object) = document.as_object_mut() {
            object.remove("candidate_skin_catalog");
        }
        let options = HostOptions::from_document(document).ok_or_else(invalid)?;
        if options.api_version != 1 || options.preferences.validate().is_err() {
            return Err(invalid());
        }
        if options.read_only_state == Some(true) {
            return Err("learning_read_only".to_owned().into());
        }
        let state = options
            .preferences_directory
            .as_deref()
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or_else(invalid)?;
        // 查询这个会话的资源清单，而非后台最新选择；旧会话可能仍持有上一代。
        let specification = crate::dictionary_updates::specification_for_options(&options)
            .map_err(|_| invalid())?;
        let content_id = specification.generation().map_err(|_| invalid())?;
        let paths = RuntimePaths {
            resources: PathBuf::from(&options.resources),
            user_data: PathBuf::from(&options.user_data),
            cache: PathBuf::from(&options.cache),
            dictionaries: PathBuf::from(&options.dictionaries),
        };
        paths.validate().map_err(|_| invalid())?;
        // 不能让旧运行时选项按新词库标识上传，也不从自由目录名猜 content_id。
        if paths.dictionaries != paths.user_data.join("dictionaries").join(&content_id) {
            return Err(invalid());
        }
        let main_dictionary = engine_schemes(options.edition()).reads_main_dictionary();
        Ok(Self {
            options: options.into_engine_options(),
            paths,
            state,
            content_id,
            main_dictionary,
        })
    }

    pub(super) fn reset(&self) -> Result<Value, RoundError> {
        crate::dictionary::reset::under_maintenance(&self.options, Some(&self.state))
            .map_err(|_| RoundError::Local)
    }

    pub(super) fn borrowed(&self) -> RoundContext<'_> {
        RoundContext {
            paths: &self.paths,
            state_directory: &self.state,
            content_id: &self.content_id,
            main_dictionary: self.main_dictionary,
        }
    }
}

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
    json!({ "uploaded": report.uploaded, "upload_unchanged": report.upload_unchanged,
        "dropped_weights": report.dropped_weights,
        "upload_error": report.upload_error.map(|error| failure(error).json()),
        "downloads": downloads(report.downloads), "pending": downloads(report.pending) })
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

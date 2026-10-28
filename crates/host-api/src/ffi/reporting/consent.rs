//! 持久同意的正常路径适配；故障回调只使用已经固定的本地记录头。

use super::*;
use msime_client_core::preferences::UsageReportingConsent;
use msime_client_services::telemetry::{Delivery, TelemetrySender};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsentRequest {
    directory: String,
    /// 首次宿主准备前可指定包内版本，避免初始化成 full 的偏好默认值。
    #[serde(default)]
    edition: Option<String>,
}

/// 确保持续同意历史。请求 `{directory,edition?}` 是偏好目录；读写失败保持错误，不清理已有报告。
/// 成功返回 `{enabled:false}` 或 `{enabled:true,epoch,since_unix_ms}`；不联网。
/// # Safety
/// `request` 指向 `length` 个可读 UTF-8 JSON 字节。
#[no_mangle]
pub unsafe extern "C" fn msime_client_telemetry_consent(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 调用方遵守文档中的缓冲区约定。
        let request: ConsentRequest = unsafe { document(request, length, MAX_REQUEST_BYTES)? };
        let directory = absolute_directory(&request.directory)?;
        let store = match request.edition.as_deref() {
            Some(id) => PreferencesStore::for_edition(
                directory,
                msime_client_core::edition::Edition::by_id(id).ok_or("unknown edition")?,
            ),
            None => PreferencesStore::new(directory),
        };
        match store
            .usage_reporting_consent()
            .map_err(|error| error.to_string())?
        {
            Some(consent) => Ok(
                json!({"enabled": true, "epoch": consent.epoch, "since_unix_ms": consent.since_unix_ms}),
            ),
            None => Ok(json!({"enabled": false})),
        }
    })
}

pub(super) fn with_epoch<T>(
    preferences_directory: &str,
    epoch: &str,
    store: &TelemetryStore,
    operation: impl FnOnce(&TelemetryStore, UsageReportingConsent) -> Result<T, String>,
) -> Result<Option<T>, String> {
    let preferences = PreferencesStore::new(absolute_directory(preferences_directory)?);
    preferences
        .with_usage_reporting_consent(|enabled, current| {
            let Some(current) = current
                .filter(|current| enabled && current.epoch.to_string().eq_ignore_ascii_case(epoch))
            else {
                return Ok(None);
            };
            let bound = store
                .activate_consent_epoch(current.epoch)
                .map_err(|error| error.to_string())?;
            operation(&bound, current).map(Some)
        })
        .map_err(|error| error.to_string())?
}

/// 每条发送前检查；检查结束后释放偏好锁，网络阻塞不能延迟撤回。
pub(super) struct EpochSender<'a, S> {
    pub preferences: PreferencesStore,
    pub epoch: &'a str,
    pub sender: &'a S,
}

impl<S: TelemetrySender> TelemetrySender for EpochSender<'_, S> {
    fn send(&self, body: &[u8]) -> Delivery {
        let allowed = self
            .preferences
            .with_usage_reporting_consent(|enabled, current| {
                enabled
                    && current.is_some_and(|current| {
                        current.epoch.to_string().eq_ignore_ascii_case(self.epoch)
                    })
            });
        if !matches!(allowed, Ok(true)) {
            return Delivery::Failed;
        }
        self.sender.send(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Barrier};

    struct Paused {
        entered: Arc<Barrier>,
        release: Arc<Barrier>,
        calls: Arc<AtomicUsize>,
    }

    impl TelemetrySender for Paused {
        fn send(&self, _body: &[u8]) -> Delivery {
            if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                self.entered.wait();
                self.release.wait();
            }
            Delivery::Accepted
        }
    }

    #[test]
    fn telemetry_epoch_sender_releases_preferences_while_network_is_blocked() {
        let root = tempfile::tempdir().unwrap();
        let preference_path = root.path().join("preferences");
        let preferences = PreferencesStore::new(&preference_path);
        let consent = preferences.usage_reporting_consent().unwrap().unwrap();
        let epoch = consent.epoch.to_string();
        let store = TelemetryStore::new(root.path().join("telemetry"));
        let bound = with_epoch(
            preference_path.to_str().unwrap(),
            &epoch,
            &store,
            |store, _| Ok(store.clone()),
        )
        .unwrap()
        .unwrap();
        let session = bound
            .begin_session(
                &TelemetryApp::new("ios", "0.1.0").unwrap(),
                SystemTime::now(),
            )
            .unwrap();
        bound.end_session_by_id(&session.session_id).unwrap();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let calls = Arc::new(AtomicUsize::new(0));
        let sender = Paused {
            entered: entered.clone(),
            release: release.clone(),
            calls: calls.clone(),
        };
        let worker_preferences = preference_path.clone();
        let worker = std::thread::spawn(move || {
            let sender = EpochSender {
                preferences: PreferencesStore::new(worker_preferences),
                epoch: &epoch,
                sender: &sender,
            };
            bound.flush(&sender, SystemTime::now())
        });
        entered.wait();
        let (written, observed) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            let preferences = PreferencesStore::new(preference_path);
            let current = preferences.load().unwrap();
            let mut disabled = current.preferences;
            disabled.usage_reporting = false;
            preferences.save(current.revision, disabled).unwrap();
            written.send(()).unwrap();
        });
        // 无论是否超时都释放网络，让错误实现也能结束并报告失败，避免测试无限等待。
        let completed = observed.recv_timeout(std::time::Duration::from_secs(2));
        release.wait();
        writer.join().unwrap();
        let report = worker.join().unwrap().unwrap();
        assert!(completed.is_ok(), "撤回不得等待网络发送");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(report.sent, 1);
        assert_eq!(report.remaining, 1);
    }
}

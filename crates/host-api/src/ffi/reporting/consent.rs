//! 持久授权的正常路径适配；故障回调只使用已经固定的本地记录头。

use super::*;
use msime_client_core::preferences::{
    LoadedUsageReportingConsent, ReportingAuthority, ReportingAuthorization, UsageReportingConsent,
};
use msime_client_services::telemetry::{Delivery, TelemetrySender};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Publication {
    policy_allowed: bool,
    policy_identity: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConsentRequest {
    #[serde(default)]
    directory: Option<String>,
    /// 首次宿主准备前指定包内版本，避免初始化成 full 的偏好默认值。
    #[serde(default)]
    edition: Option<String>,
    #[serde(default)]
    authority_directory: Option<String>,
    /// 仅权威设置进程发布；接收沙箱省略本字段，以只读方式核对。
    #[serde(default)]
    publication: Option<Publication>,
}

fn authority_value(status: ReportingAuthorization) -> Value {
    match status {
        ReportingAuthorization::Enabled(consent) => {
            json!({"enabled":true, "epoch":consent.epoch, "since_unix_ms":consent.since_unix_ms})
        }
        ReportingAuthorization::Disabled => json!({"enabled":false}),
        ReportingAuthorization::Suspended => json!({"enabled":false, "suspended":true}),
    }
}

fn preferences_store(
    directory: Option<&str>,
    edition: Option<&str>,
) -> Result<PreferencesStore, String> {
    let directory = absolute_directory(directory.ok_or("directory is required")?)?;
    match edition {
        Some(id) => Ok(PreferencesStore::for_edition(
            directory,
            msime_client_core::edition::Edition::by_id(id).ok_or("unknown edition")?,
        )),
        None => Ok(PreferencesStore::new(directory)),
    }
}

/// 读取偏好授权 `{directory,edition?}` 或只读共享授权 `{authority_directory}`。
/// 权威设置进程发布时另带源 `directory` 与 `publication:{policy_allowed,policy_identity}`。
/// 错误不清理已有报告；暂停返回 `{enabled:false,suspended:true}`，不能当作撤回。
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
        if let Some(directory) = request.authority_directory.as_deref() {
            let authority = ReportingAuthority::new(absolute_directory(directory)?);
            return match request.publication {
                Some(publication) => {
                    let source = preferences_store(
                        request.directory.as_deref(),
                        request.edition.as_deref(),
                    )?;
                    authority
                        .publish_from(
                            &source,
                            publication.policy_allowed,
                            &publication.policy_identity,
                        )
                        .map(authority_value)
                        .map_err(|error| error.to_string())
                }
                None if request.directory.is_none() && request.edition.is_none() => authority
                    .with_current(authority_value)
                    .map_err(|error| error.to_string()),
                None => {
                    Err("authority reads cannot include a preferences directory or edition".into())
                }
            };
        }
        if request.publication.is_some() {
            return Err("publication requires authority_directory".into());
        }
        let store = preferences_store(request.directory.as_deref(), request.edition.as_deref())?;
        match polled_consent(&store, request.edition)? {
            Some(consent) => Ok(authority_value(ReportingAuthorization::Enabled(consent))),
            None => Ok(authority_value(ReportingAuthorization::Disabled)),
        }
    })
}

type ConsentCache = HashMap<(PathBuf, Option<String>), LoadedUsageReportingConsent>;

/// 观察线程每隔几百毫秒轮询一次授权；按 `(directory, edition)` 在进程内记住上一次读到的文档字节，文件没变时不再解析偏好。核对仍在偏好锁内逐字节进行，撤回不会被缓存挡住。
fn polled_consent(
    store: &PreferencesStore,
    edition: Option<String>,
) -> Result<Option<UsageReportingConsent>, String> {
    static CACHE: OnceLock<Mutex<ConsentCache>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (store.directory().to_path_buf(), edition);
    let previous = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&key);
    let loaded = store
        .usage_reporting_consent_reusing(previous)
        .map_err(|error| error.to_string())?;
    let consent = loaded.consent;
    cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(key, loaded);
    Ok(consent)
}

pub(super) enum ConsentSource {
    Preferences(PreferencesStore),
    Authority(ReportingAuthority),
}

impl ConsentSource {
    pub fn new(preferences: Option<&str>, authority: Option<&str>) -> Result<Self, String> {
        match (preferences, authority) {
            (Some(directory), None) => Ok(Self::Preferences(PreferencesStore::new(
                absolute_directory(directory)?,
            ))),
            (None, Some(directory)) => Ok(Self::Authority(ReportingAuthority::new(
                absolute_directory(directory)?,
            ))),
            _ => Err("exactly one consent source is required".into()),
        }
    }

    /// 条件清理在来源锁内执行，绕过普通操作的代次校验但不绕过关闭校验。
    pub fn clear_if_disabled(&self, store: &TelemetryStore) -> Result<bool, String> {
        match self {
            Self::Preferences(preferences) => clear_if_consent_disabled(preferences, store),
            Self::Authority(authority) => {
                msime_client_services::telemetry::clear_if_authority_disabled(authority, store)
            }
        }
    }

    /// 暂停和读错误保持错误；只有明确关闭才把 None 交给条件清理。
    pub fn with_current<T>(
        &self,
        operation: impl FnOnce(Option<UsageReportingConsent>) -> Result<T, String>,
    ) -> Result<T, String> {
        match self {
            Self::Preferences(store) => store
                .with_usage_reporting_consent(|enabled, current| {
                    if enabled && current.is_none() {
                        return Err("reporting consent is not initialized".into());
                    }
                    operation(if enabled { current } else { None })
                })
                .map_err(|error| error.to_string())?,
            Self::Authority(authority) => authority
                .with_current(|status| match status {
                    ReportingAuthorization::Enabled(consent) => operation(Some(consent)),
                    ReportingAuthorization::Disabled => operation(None),
                    ReportingAuthorization::Suspended => {
                        Err("reporting authority is suspended".into())
                    }
                })
                .map_err(|error| error.to_string())?,
        }
    }
}

pub(super) fn with_epoch<T>(
    source: &ConsentSource,
    epoch: &str,
    store: &TelemetryStore,
    operation: impl FnOnce(&TelemetryStore, UsageReportingConsent) -> Result<T, String>,
) -> Result<Option<T>, String> {
    source.with_current(|current| {
        let Some(current) =
            current.filter(|current| current.epoch.to_string().eq_ignore_ascii_case(epoch))
        else {
            return Ok(None);
        };
        let bound = store
            .activate_consent_epoch(current.epoch)
            .map_err(|error| error.to_string())?;
        operation(&bound, current).map(Some)
    })
}

/// 每条发送前核对授权；检查结束后释放读锁，网络阻塞不能延迟撤回。
pub(super) struct EpochSender<'a, S> {
    pub source: ConsentSource,
    pub epoch: &'a str,
    pub sender: &'a S,
}

impl<S: TelemetrySender> TelemetrySender for EpochSender<'_, S> {
    fn send(&self, body: &[u8]) -> Delivery {
        let allowed = self.source.with_current(|current| {
            Ok(current
                .is_some_and(|current| current.epoch.to_string().eq_ignore_ascii_case(self.epoch)))
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
            &ConsentSource::Preferences(PreferencesStore::new(&preference_path)),
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
                source: ConsentSource::Preferences(PreferencesStore::new(worker_preferences)),
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
    #[test]
    fn telemetry_authority_sender_releases_read_lock_while_network_is_blocked() {
        let root = tempfile::tempdir().unwrap();
        let preference_path = root.path().join("preferences");
        let preferences = PreferencesStore::new(&preference_path);
        let authority = ReportingAuthority::new(root.path().join("authority"));
        let ReportingAuthorization::Enabled(consent) = authority
            .publish_from(&preferences, true, "policy")
            .unwrap()
        else {
            panic!("not enabled");
        };
        let epoch = consent.epoch.to_string();
        let source = ConsentSource::Authority(authority.clone());
        let store = TelemetryStore::new(root.path().join("telemetry"));
        let bound = with_epoch(&source, &epoch, &store, |store, _| Ok(store.clone()))
            .unwrap()
            .unwrap();
        let session = bound
            .begin_session(
                &TelemetryApp::new("harmony", "0.1.0").unwrap(),
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
        let worker = std::thread::spawn(move || {
            let sender = EpochSender {
                source,
                epoch: &epoch,
                sender: &sender,
            };
            bound.flush(&sender, SystemTime::now())
        });
        entered.wait();
        let (written, observed) = mpsc::channel();
        let writer = std::thread::spawn(move || {
            authority
                .publish_from(&preferences, false, "policy")
                .unwrap();
            authority
                .publish_from(&preferences, true, "policy")
                .unwrap();
            written.send(()).unwrap();
        });
        let completed = observed.recv_timeout(std::time::Duration::from_secs(2));
        release.wait();
        writer.join().unwrap();
        let report = worker.join().unwrap().unwrap();
        assert!(completed.is_ok(), "撤回不得等待网络读锁");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(report.sent, 1);
        assert_eq!(report.remaining, 1);
    }
}

//! 所有桌面正常入口共享的持久授权适配，偏好锁先于队列锁。
use super::{Delivery, TelemetrySender, TelemetryStore};
use msime_client_core::preferences::{
    PreferencesStore, ReportingAuthority, ReportingAuthorization, UsageReportingConsent,
};

/// 在偏好锁内核对持久代次并执行本地操作；操作不得联网或反向取得偏好锁。
pub fn with_consent_epoch<T>(
    preferences: &PreferencesStore,
    epoch: &str,
    store: &TelemetryStore,
    operation: impl FnOnce(&TelemetryStore, UsageReportingConsent) -> Result<T, String>,
) -> Result<Option<T>, String> {
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
pub struct ConsentEpochSender<'a, S> {
    pub preferences: PreferencesStore,
    pub epoch: &'a str,
    pub sender: &'a S,
}

impl<S: TelemetrySender> TelemetrySender for ConsentEpochSender<'_, S> {
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

/// 明确关闭后才清理；核对与清理在同一偏好锁内，旧回调不能删除重开后的队列。
pub fn clear_if_consent_disabled(
    preferences: &PreferencesStore,
    store: &TelemetryStore,
) -> Result<bool, String> {
    preferences
        .with_usage_reporting_consent(|enabled, _| {
            if enabled {
                return Ok(false);
            }
            clear_authorized(store)
        })
        .map_err(|error| error.to_string())?
}

/// 只读授权明确关闭后才清理；暂停与读错误保留队列，核对期间保持授权读锁。
pub fn clear_if_authority_disabled(
    authority: &ReportingAuthority,
    store: &TelemetryStore,
) -> Result<bool, String> {
    authority
        .with_current(|current| match current {
            ReportingAuthorization::Enabled(_) => Ok(false),
            ReportingAuthorization::Disabled => clear_authorized(store),
            ReportingAuthorization::Suspended => Err("reporting authority is suspended".into()),
        })
        .map_err(|error| error.to_string())?
}

fn clear_authorized(store: &TelemetryStore) -> Result<bool, String> {
    let current = TelemetryStore::new(store.directory());
    let _lock = current.lock().map_err(|error| error.to_string())?;
    current.clear_locked().map_err(|error| error.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::telemetry::{TelemetryApp, TelemetryStore};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc, Barrier};
    use std::time::SystemTime;

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
        let bound = with_consent_epoch(&preferences, &epoch, &store, |store, _| Ok(store.clone()))
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
            let sender = ConsentEpochSender {
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

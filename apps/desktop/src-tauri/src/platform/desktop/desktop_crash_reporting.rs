use msime_client_core::preferences::{PreferencesStore, UsageReportingConsent};
use msime_client_services::telemetry::{
    backend_sender, Delivery, FlushReport, SessionStart, TelemetryApp, TelemetryError,
    TelemetrySender, TelemetryStore,
};
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

#[allow(unsafe_code)]
unsafe extern "C" {
    fn msime_native_crash_install();
    fn msime_native_crash_arm(path: *const std::ffi::c_char, header: *const std::ffi::c_char);
    fn msime_native_crash_disarm();
}

struct Reporter {
    preferences: Arc<PreferencesStore>,
    store: TelemetryStore,
    app: TelemetryApp,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    capture: Option<(UsageReportingConsent, SessionStart)>,
    last_flush: Option<Instant>,
    suspended: bool,
    cleared: bool,
}

static REPORTER: OnceLock<Arc<Reporter>> = OnceLock::new();

/// 与原生输入法共用遥测目录和安装 id；设置与面板不额外计为一次输入会话。
fn directory() -> Option<PathBuf> {
    let edition = crate::package_edition();
    #[cfg(target_os = "macos")]
    {
        let root =
            PathBuf::from(std::env::var_os("HOME")?).join("Library/Application Support/MSIME");
        Some(
            if edition.id == "full" {
                root
            } else {
                root.join(&edition.id)
            }
            .join("telemetry"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        Some(
            PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
                .join(&edition.windows()?.user_data_directory),
        )
    }
    #[cfg(target_os = "linux")]
    {
        let root = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| Some(PathBuf::from(std::env::var_os("HOME")?).join(".local/state")))?;
        let name = if edition.id == "full" {
            "msime".to_owned()
        } else {
            format!("msime-{}", edition.id)
        };
        Some(root.join(name))
    }
}

#[allow(unsafe_code)]
pub(crate) fn start(app: &tauri::AppHandle, preferences: Arc<PreferencesStore>) {
    let Some(directory) = directory().filter(|path| path.is_absolute()) else {
        return;
    };
    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    let Ok(identity) = TelemetryApp::new(platform, &app.package_info().version.to_string())
        .and_then(|identity| identity.with_component("desktop"))
    else {
        return;
    };
    let reporter = Arc::new(Reporter {
        preferences,
        store: TelemetryStore::new(directory),
        app: identity,
        state: Mutex::new(State::default()),
    });
    if REPORTER.set(reporter).is_err() {
        return;
    }
    // 处理器只在主线程安装一次，读取同意之前保持未启用；不将可恢复的 Rust panic 当作进程崩溃。
    unsafe {
        msime_native_crash_install();
    }
    refresh();
}

impl State {
    #[allow(unsafe_code)]
    fn suspend(&mut self) {
        unsafe { msime_native_crash_disarm() };
        self.suspended = true;
    }

    #[allow(unsafe_code)]
    fn arm(&mut self) -> Option<()> {
        let (_, capture) = self.capture.as_ref()?;
        let path = CString::new(capture.crash_record_path.to_str()?).ok()?;
        let header = CString::new(capture.crash_record_header.as_str()).ok()?;
        // C 端立即复制字符串；恢复同代次采集时继续使用原快照。
        unsafe { msime_native_crash_arm(path.as_ptr(), header.as_ptr()) };
        self.suspended = false;
        Some(())
    }
}

impl Reporter {
    fn refresh_capture(&self, state: &mut State) -> Option<UsageReportingConsent> {
        let consent = match self.preferences.usage_reporting_consent() {
            Ok(Some(consent)) => consent,
            Ok(None) => {
                state.suspend();
                state.capture = None;
                state.last_flush = None;
                if !state.cleared {
                    // 关闭观察可能迟到，只有偏好锁内仍关闭才清理。
                    state.cleared = matches!(
                        self.preferences.with_usage_reporting_consent(|enabled, _| {
                            !enabled && self.store.clear().is_ok()
                        }),
                        Ok(true)
                    );
                }
                return None;
            }
            Err(_) => {
                // 暂时不可读只停采；恢复同一持续授权时保留身份与发送间隔。
                state.suspend();
                return None;
            }
        };
        state.cleared = false;
        if state.capture.as_ref().map(|(old, _)| old.epoch) != Some(consent.epoch) {
            state.suspend();
            state.capture = None;
            state.last_flush = None;
            let capture = self
                .preferences
                .with_usage_reporting_consent(|enabled, current| {
                    if !enabled || current.map(|current| current.epoch) != Some(consent.epoch) {
                        return Ok(None);
                    }
                    self.store
                        .activate_consent_epoch(consent.epoch)?
                        .prepare_crash_capture(&self.app, SystemTime::now())
                        .map(Some)
                })
                .ok()?
                .ok()??;
            state.capture = Some((consent, capture));
        }
        if state.suspended {
            state.arm()?;
        }
        Some(consent)
    }

    fn flush(
        &self,
        store: &TelemetryStore,
        consent: UsageReportingConsent,
        sender: &impl TelemetrySender,
    ) -> Option<FlushReport> {
        // 本地准备按偏好、队列顺序加锁，发送期间不持偏好锁。
        let store = self
            .preferences
            .with_usage_reporting_consent(|enabled, current| {
                if !enabled || current.map(|current| current.epoch) != Some(consent.epoch) {
                    return Ok::<_, TelemetryError>(None);
                }
                let bound = store.activate_consent_epoch(consent.epoch)?;
                bound.record_active(&self.app, SystemTime::now())?;
                Ok(Some(bound))
            })
            .ok()?
            .ok()??;
        store
            .flush(
                &EpochSender {
                    preferences: &self.preferences,
                    consent,
                    sender,
                },
                SystemTime::now(),
            )
            .ok()
    }
}

struct EpochSender<'a, S> {
    preferences: &'a PreferencesStore,
    consent: UsageReportingConsent,
    sender: &'a S,
}

impl<S: TelemetrySender> TelemetrySender for EpochSender<'_, S> {
    fn send(&self, body: &[u8]) -> Delivery {
        let allowed = self
            .preferences
            .with_usage_reporting_consent(|enabled, current| {
                enabled && current.map(|current| current.epoch) == Some(self.consent.epoch)
            });
        if !matches!(allowed, Ok(true)) {
            return Delivery::Failed;
        }
        self.sender.send(body)
    }
}

/// 设置保存和既有偏好监视器都调用它；不新建轮询线程。
pub(crate) fn refresh() {
    let Some(reporter) = REPORTER.get() else {
        return;
    };
    let Ok(mut state) = reporter.state.lock() else {
        return;
    };
    let Some(consent) = reporter.refresh_capture(&mut state) else {
        return;
    };
    if state
        .last_flush
        .is_some_and(|last| last.elapsed() < Duration::from_secs(30 * 60))
    {
        return;
    }
    state.last_flush = Some(Instant::now());
    let reporter = Arc::clone(reporter);
    let _ = std::thread::Builder::new()
        .name("msime-desktop-telemetry".to_owned())
        .spawn(move || {
            if let Ok(sender) = backend_sender() {
                let _ = reporter.flush(&reporter.store, consent, &sender);
                #[cfg(target_os = "linux")]
                {
                    // Fcitx5 的会话目录与 IBus 分开，补收时也绑定同一实际授权。
                    if let Some(root) = directory() {
                        let _ = reporter.flush(
                            &TelemetryStore::new(root.join("fcitx5")),
                            consent,
                            &sender,
                        );
                    }
                }
            }
        });
}

#[cfg(all(test, unix))]
#[path = "desktop_crash_reporting_tests.rs"]
mod tests;

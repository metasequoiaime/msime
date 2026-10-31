use msime_client_core::preferences::{PreferencesStore, UsageReportingConsent};
use msime_client_services::telemetry::{
    backend_sender, clear_if_consent_disabled, with_consent_epoch, ConsentEpochSender, FlushReport,
    SessionStart, TelemetryApp, TelemetrySender, TelemetryStore,
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
        let support = msime_host_macos::telemetry_support_directory()?;
        let root = support.join("MSIME");
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
                        clear_if_consent_disabled(&self.preferences, &self.store),
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
            let capture = with_consent_epoch(
                &self.preferences,
                &consent.epoch.to_string(),
                &self.store,
                |store, _| {
                    store
                        .prepare_crash_capture(&self.app, SystemTime::now())
                        .map_err(|error| error.to_string())
                },
            )
            .ok()??;
            state.capture = Some((consent, capture));
        }
        if state.suspended {
            self.arm_capture(state, consent)?;
        }
        Some(consent)
    }

    fn arm_capture(&self, state: &mut State, consent: UsageReportingConsent) -> Option<()> {
        // 慢准备或恢复结束后，再在持久偏好锁内发布；旧快照不能重新启用采集。
        let armed = with_consent_epoch(
            &self.preferences,
            &consent.epoch.to_string(),
            &self.store,
            |_, _| Ok(state.arm()),
        );
        if matches!(armed, Ok(Some(Some(())))) {
            Some(())
        } else {
            state.suspend();
            None
        }
    }

    fn flush(
        &self,
        store: &TelemetryStore,
        consent: UsageReportingConsent,
        sender: &impl TelemetrySender,
        record_active: bool,
    ) -> Option<FlushReport> {
        // 代收不能初始化从未使用的框架目录，或为另一套输入法增加日活。
        if !record_active && !store.directory().is_dir() {
            return None;
        }
        // 本地准备按偏好、队列顺序加锁，发送期间不持偏好锁。
        let epoch = consent.epoch.to_string();
        let store = with_consent_epoch(&self.preferences, &epoch, store, |bound, _| {
            if record_active {
                bound.record_active(&self.app, SystemTime::now())
            } else {
                bound.collect_crashes(&self.app)
            }
            .map_err(|error| error.to_string())?;
            Ok(bound.clone())
        })
        .ok()??;
        store
            .flush(
                &ConsentEpochSender {
                    preferences: PreferencesStore::new(self.preferences.directory()),
                    epoch: &epoch,
                    sender,
                },
                SystemTime::now(),
            )
            .ok()
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
                let _ = reporter.flush(&reporter.store, consent, &sender, true);
                #[cfg(target_os = "linux")]
                {
                    // Fcitx5 的会话目录与 IBus 分开，只代收既有目录，不为未使用的框架计日活。
                    if let Some(root) = directory() {
                        let _ = reporter.flush(
                            &TelemetryStore::new(root.join("fcitx5")),
                            consent,
                            &sender,
                            false,
                        );
                    }
                }
            }
        });
}

#[cfg(all(test, unix))]
#[path = "desktop_crash_reporting_tests.rs"]
mod tests;

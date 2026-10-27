use msime_client_core::preferences::PreferencesStore;
use msime_client_services::account::BackendAccountClient;
use msime_client_services::telemetry::{SessionStart, TelemetryApp, TelemetryStore};
use std::ffi::CString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

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
    capture: Option<SessionStart>,
    last_flush: Option<Instant>,
    consent: Option<bool>,
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

/// 设置保存和既有偏好监视器都调用它；不新建轮询线程。
pub(crate) fn refresh() {
    let Some(reporter) = REPORTER.get() else {
        return;
    };
    let consent = reporter
        .preferences
        .load()
        .ok()
        .map(|snapshot| snapshot.preferences.usage_reporting);
    let Ok(mut state) = reporter.state.lock() else {
        return;
    };
    if consent != Some(true) {
        // 配置不可读时也停用采集，只有显式关闭才删除已有记录。
        unsafe {
            msime_native_crash_disarm();
        }
        state.capture = None;
        state.last_flush = None;
        if consent == Some(false) && state.consent != Some(false) {
            let _ = reporter.store.clear();
        }
        state.consent = consent;
        return;
    }
    state.consent = consent;
    if state.capture.is_none() {
        let Ok(capture) = reporter
            .store
            .prepare_crash_capture(&reporter.app, SystemTime::now())
        else {
            return;
        };
        let (Some(path), Ok(header)) = (
            capture.crash_record_path.to_str(),
            CString::new(capture.crash_record_header.as_str()),
        ) else {
            return;
        };
        let Ok(path) = CString::new(path) else {
            return;
        };
        // C 端立即复制这两个字符串，不保留 Rust 的借用。
        unsafe {
            msime_native_crash_arm(path.as_ptr(), header.as_ptr());
        }
        state.capture = Some(capture);
    }
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
            if reporter
                .preferences
                .load()
                .is_ok_and(|snapshot| snapshot.preferences.usage_reporting)
            {
                if let Ok(sender) = BackendAccountClient::new() {
                    let _ = reporter
                        .store
                        .flush_with(&reporter.app, &sender, SystemTime::now());
                    #[cfg(target_os = "linux")]
                    {
                        // Fcitx5 的会话目录与 IBus 分开，设置应用也补收它的记录。
                        if let Some(root) = directory() {
                            let _ = TelemetryStore::new(root.join("fcitx5")).flush_with(
                                &reporter.app,
                                &sender,
                                SystemTime::now(),
                            );
                        }
                    }
                }
            }
        });
}

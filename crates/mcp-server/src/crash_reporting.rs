//! MCP 仅在本地捕获自身致命故障，由输入法宿主补报；不记录调用参数或输入会话。

use crate::config::Config;
use msime_client_core::edition::Edition;
use msime_client_core::preferences::{Preferences, PreferencesStore, UsageReportingConsent};
use msime_client_services::telemetry::{
    clear_if_consent_disabled, with_consent_epoch, SessionStart, TelemetryApp, TelemetryStore,
};
use std::cell::Cell;
use std::ffi::{CString, OsString};
use std::panic::{catch_unwind, resume_unwind, UnwindSafe};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::{self, Sender};
use std::sync::Once;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

#[allow(unsafe_code)]
unsafe extern "C" {
    fn msime_native_crash_install();
    fn msime_native_crash_arm(path: *const std::ffi::c_char, header: *const std::ffi::c_char);
    fn msime_native_crash_disarm();
    fn msime_native_crash_record(summary: *const std::ffi::c_char);
}

#[derive(Clone, Copy)]
struct PanicSite {
    file: [u8; 128],
    length: usize,
    line: u32,
    column: u32,
}

thread_local! {
    static PANIC_SITE: Cell<Option<PanicSite>> = const { Cell::new(None) };
}

fn remember_panic_site() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // 钩子仅在本线程的固定缓冲区保留源码位置，不碰 payload，也不写任何崩溃文件。
            let site = info.location().map(|location| {
                let name = location.file().rsplit(['/', '\\']).next().unwrap_or("");
                let length = name.len().min(128);
                let mut file = [0; 128];
                file[..length].copy_from_slice(&name.as_bytes()[..length]);
                PanicSite {
                    file,
                    length,
                    line: location.line(),
                    column: location.column(),
                }
            });
            let _ = PANIC_SITE.try_with(|slot| slot.set(site));
            previous(info);
        }));
    });
}

/// 只认领越过主入口的 panic，工具内部捕获或 Tokio 任务处理的 panic 不算进程崩溃。
#[allow(unsafe_code)]
pub(crate) fn run_main(run: impl FnOnce() -> ExitCode + UnwindSafe) -> ExitCode {
    match catch_unwind(run) {
        Ok(code) => code,
        Err(payload) => {
            // panic 内容可能包含工具参数，只写固定类型及源码文件名；随后保留原有退出语义。
            let site = PANIC_SITE.try_with(Cell::get).ok().flatten();
            let summary = site.map(|site| {
                CString::new(format!(
                    "Unhandled Rust panic at {}:{}:{}",
                    String::from_utf8_lossy(&site.file[..site.length]),
                    site.line,
                    site.column
                ))
            });
            let summary = summary.as_ref().and_then(|value| value.as_deref().ok());
            unsafe {
                msime_native_crash_record(summary.unwrap_or(c"Unhandled Rust panic").as_ptr())
            };
            resume_unwind(payload)
        }
    }
}

pub(crate) struct Reporter {
    stop: Sender<()>,
    worker: Option<JoinHandle<()>>,
}

#[derive(Default)]
struct Capture {
    current: Option<CaptureSnapshot>,
    disabled_target: Option<(PathBuf, PathBuf)>,
    suspended: bool,
}

struct CaptureSnapshot {
    directory: PathBuf,
    preferences_directory: PathBuf,
    consent: UsageReportingConsent,
    start: SessionStart,
}

struct WatcherPanicGuard;

impl Drop for WatcherPanicGuard {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if std::thread::panicking() {
            // 观察线程意外退出后无法继续检查同意，不能留下永久启用的记录器。
            unsafe { msime_native_crash_disarm() };
        }
    }
}

#[allow(unsafe_code)]
pub(crate) fn start(config: &Config) -> Option<Reporter> {
    // 主线程安装信号备用栈，仍未取得同意时记录器保持停用。帮助和版本命令不经过这里。
    unsafe { msime_native_crash_install() };
    remember_panic_site();
    let mut capture = Capture::default();
    capture.refresh(config);
    let config = config.clone();
    let (stop, receiver) = mpsc::channel();
    let worker = std::thread::Builder::new()
        .name("msime-mcp-crash-consent".into())
        .spawn(move || {
            let _panic_guard = WatcherPanicGuard;
            while receiver.recv_timeout(Duration::from_millis(500))
                == Err(mpsc::RecvTimeoutError::Timeout)
            {
                capture.refresh(&config);
            }
        });
    match worker {
        Ok(worker) => Some(Reporter {
            stop,
            worker: Some(worker),
        }),
        Err(_) => {
            // 无法持续观察同意时停用，不能把启动时的同意永久缓存下来。
            unsafe { msime_native_crash_disarm() };
            None
        }
    }
}

impl Drop for Reporter {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        // 主入口外的 catch_unwind 还需要写致命 panic，普通退出则解除采集。
        if !std::thread::panicking() {
            unsafe { msime_native_crash_disarm() };
        }
    }
}

impl Capture {
    #[allow(unsafe_code)]
    fn suspend(&mut self) {
        unsafe { msime_native_crash_disarm() };
        self.suspended = true;
    }

    fn disable(&mut self, directory: PathBuf, preferences: &PreferencesStore) {
        self.suspend();
        self.current = None;
        let target = (directory, preferences.directory().to_owned());
        if self.disabled_target.as_ref() == Some(&target) {
            return;
        }
        // 关闭观察可能迟到；只在偏好锁内仍明确关闭时清理，不能删除已重开的新队列。
        let cleared = clear_if_consent_disabled(preferences, &TelemetryStore::new(&target.0));
        if matches!(cleared, Ok(true)) {
            self.disabled_target = Some(target);
        }
    }

    #[allow(unsafe_code)]
    fn arm(&mut self) {
        let Some(snapshot) = &self.current else {
            return;
        };
        let (Some(path), Ok(header)) = (
            snapshot.start.crash_record_path.to_str(),
            CString::new(snapshot.start.crash_record_header.as_str()),
        ) else {
            return;
        };
        let Ok(path) = CString::new(path) else {
            return;
        };
        // C 端立即复制字符串；故障现场只使用这份预备身份，不读偏好或联网。
        unsafe { msime_native_crash_arm(path.as_ptr(), header.as_ptr()) };
        self.suspended = false;
    }

    fn refresh(&mut self, config: &Config) {
        let target = config.read_host_options().ok().and_then(|document| {
            let directory = directory(config.edition(&document).ok()?, &|name| {
                std::env::var_os(name)
            })?;
            let enabled = match document.pointer("/preferences/usage_reporting") {
                Some(value) => value.as_bool()?,
                None => Preferences::default().usage_reporting,
            };
            Some((directory, config.state_dir(&document).ok()?, enabled))
        });
        let Some((directory, preferences_directory, enabled)) = target else {
            // 临时不可读仅停采；同一持续授权恢复后沿用原记录身份。
            self.suspend();
            return;
        };
        let preferences = PreferencesStore::new(&preferences_directory);
        if !enabled {
            self.disable(directory, &preferences);
            return;
        }
        let consent = match preferences.usage_reporting_consent() {
            Ok(Some(consent)) => consent,
            Ok(None) => {
                self.disable(directory, &preferences);
                return;
            }
            Err(_) => {
                self.suspend();
                return;
            }
        };
        self.disabled_target = None;
        if self.current.as_ref().is_some_and(|snapshot| {
            snapshot.directory == directory
                && snapshot.preferences_directory == preferences_directory
                && snapshot.consent.epoch == consent.epoch
        }) {
            if self.suspended {
                self.arm_authorized(&preferences);
            }
            return;
        }
        self.suspend();
        self.current = None;
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };
        let Ok(app) = TelemetryApp::new(platform, env!("MSIME_APP_VERSION"))
            .and_then(|app| app.with_component("mcp"))
        else {
            return;
        };
        // 偏好先于队列加锁，检查与准备同处一次事务；旧准备不能激活已经撤回的代次。
        let Ok(Some(start)) = with_consent_epoch(
            &preferences,
            &consent.epoch.to_string(),
            &TelemetryStore::new(&directory),
            |store, _| {
                store
                    .prepare_crash_capture(&app, SystemTime::now())
                    .map_err(|error| error.to_string())
            },
        ) else {
            return;
        };
        self.current = Some(CaptureSnapshot {
            directory,
            preferences_directory,
            consent,
            start,
        });
        self.arm_authorized(&preferences);
    }

    fn arm_authorized(&mut self, preferences: &PreferencesStore) {
        let Some(snapshot) = self.current.as_ref() else {
            return;
        };
        let directory = snapshot.directory.clone();
        let epoch = snapshot.consent.epoch.to_string();
        // 准备回包与暂停恢复都可能迟到，最终发布必须再次核对持久授权。
        let armed = with_consent_epoch(
            preferences,
            &epoch,
            &TelemetryStore::new(directory),
            |_, _| {
                self.arm();
                Ok(!self.suspended)
            },
        );
        if !matches!(armed, Ok(Some(true))) {
            self.suspend();
        }
    }
}

/// 与原生宿主按版本共用匿名安装身份和队列，不跟随可搬迁的词库目录。
fn directory(edition: &Edition, env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    #[cfg(not(target_os = "macos"))]
    let absolute = |name| {
        env(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    #[cfg(target_os = "macos")]
    {
        let _ = env;
        let root = msime_host_macos::telemetry_support_directory()?.join("MSIME");
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
        Some(absolute("LOCALAPPDATA")?.join(&edition.windows()?.user_data_directory))
    }
    #[cfg(target_os = "linux")]
    {
        let root =
            absolute("XDG_STATE_HOME").or_else(|| Some(absolute("HOME")?.join(".local/state")))?;
        Some(root.join(if edition.id == "full" {
            "msime".to_owned()
        } else {
            format!("msime-{}", edition.id)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_services::telemetry::{
        CrashContext, TelemetryKind, CRASH_DIRECTORY, SESSION_FILE,
    };
    use serde_json::json;
    use std::path::Path;
    use std::process::Command;

    fn set_consent(root: &Path, enabled: bool) {
        let store = PreferencesStore::new(root);
        let mut snapshot = store.load().unwrap();
        snapshot.preferences.usage_reporting = enabled;
        store.save(snapshot.revision, snapshot.preferences).unwrap();
    }

    #[cfg(not(target_os = "macos"))]
    fn test_environment(root: &Path, name: &str) -> Option<OsString> {
        match name {
            "HOME" | "LOCALAPPDATA" | "XDG_STATE_HOME" => Some(root.as_os_str().to_owned()),
            _ => None,
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn absent_or_relative_environment_does_not_select_a_directory() {
        assert!(directory(Edition::full(), &|_| None).is_none());
        assert!(directory(Edition::full(), &|_| Some("relative".into())).is_none());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "由 macos_directory_uses_the_native_system_home 在独立进程中运行"]
    fn macos_directory_child() {
        let root = PathBuf::from(std::env::var_os("CFFIXED_USER_HOME").unwrap());
        for edition in [Edition::full(), Edition::by_id("pinyin").unwrap()] {
            let mut expected = root.join("Library/Application Support/MSIME");
            if edition.id != "full" {
                expected = expected.join(&edition.id);
            }
            assert!(
                directory(edition, &|name| std::env::var_os(name))
                    == Some(expected.join("telemetry")),
                "MCP 与原生宿主必须使用同一个系统 Application Support 目录"
            );
        }
        assert!(!root.join("Library").exists(), "解析目录不应创建队列");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_directory_uses_the_native_system_home() {
        let root = tempfile::tempdir().unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "crash_reporting::tests::macos_directory_child",
                "--ignored",
                "--nocapture",
            ])
            .env("CFFIXED_USER_HOME", root.path())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// 手动推进实际采集器，使中间撤回必定发生在两次观察之间；最后仍由真实进程 panic 落盘。
    #[allow(unsafe_code)]
    fn controlled_capture(config: &Config, root: &Path, mode: &str) -> Capture {
        unsafe { msime_native_crash_install() };
        remember_panic_site();
        let mut capture = Capture::default();
        capture.refresh(config);
        let first = capture.current.as_ref().unwrap().start.clone();
        match mode {
            "prepared-off" | "prepared-cycle" => {
                capture.suspend();
                set_consent(root, false);
                if mode == "prepared-cycle" {
                    set_consent(root, true);
                }
                capture.arm_authorized(&PreferencesStore::new(root));
                assert!(capture.suspended);
                assert_eq!(capture.current.as_ref().unwrap().start, first);
            }
            "epoch-cycle" => {
                set_consent(root, false);
                set_consent(root, true);
                capture.refresh(config);
                let current = capture.current.as_ref().unwrap();
                assert_ne!(current.start.session_id, first.session_id);
                assert_eq!(
                    CrashContext::from_header(&current.start.crash_record_header)
                        .unwrap()
                        .consent_epoch,
                    Some(
                        PreferencesStore::new(root)
                            .usage_reporting_consent()
                            .unwrap()
                            .unwrap()
                            .epoch
                    )
                );
            }
            "read-recovery" => {
                let path = root.join("preferences.json");
                let saved = std::fs::read(&path).unwrap();
                std::fs::write(&path, b"{").unwrap();
                capture.refresh(config);
                assert!(capture.suspended);
                assert_eq!(capture.current.as_ref().unwrap().start, first);
                std::fs::write(&path, saved).unwrap();
                capture.refresh(config);
                assert!(!capture.suspended);
                assert_eq!(capture.current.as_ref().unwrap().start, first);
            }
            "stale-late" => {
                set_consent(root, false);
                set_consent(root, true);
                let preferences = PreferencesStore::new(root);
                let consent = preferences.usage_reporting_consent().unwrap().unwrap();
                // 模拟另一个已观察到重开的宿主激活新队列；旧快照此时才真正发生致命故障。
                preferences
                    .with_usage_reporting_consent(|enabled, current| {
                        assert!(enabled);
                        assert_eq!(current.unwrap().epoch, consent.epoch);
                        TelemetryStore::new(&capture.current.as_ref().unwrap().directory)
                            .activate_consent_epoch(consent.epoch)
                            .unwrap()
                            .prepare_crash_capture(
                                &TelemetryApp::new("linux", "99.0.0").unwrap(),
                                SystemTime::now(),
                            )
                            .unwrap();
                    })
                    .unwrap();
            }
            _ => unreachable!(),
        }
        capture
    }

    /// 只能在独立测试进程里安装全局处理器，防止并行测试污染宿主和其它用例。
    #[test]
    #[ignore = "由 crash_capture_lifecycle 使用合成目录启动"]
    fn child_process() {
        let root = PathBuf::from(std::env::var_os("MSIME_TEST_CRASH_ROOT").unwrap());
        let mode = std::env::var("MSIME_TEST_CRASH_MODE").unwrap();
        let config = Config {
            options: root.join("runtime-options.json"),
            state_dir: Some(root.clone()),
            allow_write: false,
            allow_dictionary_read: false,
        };
        run_main(|| {
            let controlled = matches!(
                mode.as_str(),
                "epoch-cycle" | "read-recovery" | "stale-late" | "prepared-off" | "prepared-cycle"
            );
            let _capture = controlled.then(|| controlled_capture(&config, &root, &mode));
            let _reporter =
                (!controlled).then(|| start(&config).expect("测试进程必须能观察同意开关"));
            match mode.as_str() {
                "disable" | "enable" => {
                    set_consent(&root, mode == "enable");
                    std::thread::sleep(Duration::from_millis(1600));
                }
                "rotate" => {
                    set_consent(&root, false);
                    set_consent(&root, true);
                    let epoch = PreferencesStore::new(&root)
                        .usage_reporting_consent()
                        .unwrap()
                        .unwrap()
                        .epoch;
                    let telemetry =
                        directory(Edition::full(), &|name| std::env::var_os(name)).unwrap();
                    let deadline = std::time::Instant::now() + Duration::from_secs(5);
                    loop {
                        let observed = std::fs::read(telemetry.join("telemetry-state.json"))
                            .ok()
                            .and_then(|bytes| {
                                serde_json::from_slice::<serde_json::Value>(&bytes).ok()
                            });
                        if observed
                            .as_ref()
                            .and_then(|value| value["consent_epoch"].as_str())
                            == Some(epoch.to_string().as_str())
                        {
                            break;
                        }
                        assert!(
                            std::time::Instant::now() < deadline,
                            "MCP 未观察到新的授权代次"
                        );
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                "unreadable" => {
                    std::fs::write(root.join("preferences.json"), b"{").unwrap();
                    std::thread::sleep(Duration::from_millis(1600));
                }
                "recovered" => {
                    let _ = catch_unwind(|| panic!("synthetic-private-argument"));
                    return ExitCode::SUCCESS;
                }
                #[cfg(unix)]
                "native" => std::process::abort(),
                _ => {}
            }
            panic!("synthetic-private-argument")
        });
    }

    #[test]
    fn crash_capture_lifecycle() {
        let modes = [
            ("panic", true, true),
            ("inline-off", true, true),
            ("inline-on", false, false),
            ("rotate", true, true),
            ("off", false, false),
            ("disable", true, false),
            ("enable", false, true),
            ("unreadable", true, false),
            ("recovered", true, false),
            ("epoch-cycle", true, true),
            ("read-recovery", true, true),
            ("stale-late", true, true),
            ("prepared-off", true, false),
            ("prepared-cycle", true, false),
            ("embedded-off", false, false),
            #[cfg(unix)]
            ("native", true, true),
        ];
        for (mode, enabled, records) in modes {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path();
            std::fs::write(
                root.join("runtime-options.json"),
                serde_json::to_vec(&json!({"preferences_directory": root,
                    "preferences": {"usage_reporting": if matches!(mode, "inline-off" | "embedded-off") { false } else if mode == "inline-on" { true } else { enabled }}}))
                .unwrap(),
            )
            .unwrap();
            if mode != "embedded-off" {
                set_consent(root, enabled);
            }
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "crash_reporting::tests::child_process",
                    "--ignored",
                    "--nocapture",
                ])
                .env("MSIME_TEST_CRASH_ROOT", root)
                .env("MSIME_TEST_CRASH_MODE", mode)
                .env("CFFIXED_USER_HOME", root)
                .env("LOCALAPPDATA", root)
                .env("XDG_STATE_HOME", root)
                .output()
                .unwrap();
            assert_eq!(output.status.success(), mode == "recovered", "{mode}");
            #[cfg(target_os = "macos")]
            let directory = root.join("Library/Application Support/MSIME/telemetry");
            #[cfg(not(target_os = "macos"))]
            let directory =
                directory(Edition::full(), &|name| test_environment(root, name)).unwrap();
            assert!(!directory.join(SESSION_FILE).exists(), "{mode}");
            let files: Vec<_> = std::fs::read_dir(directory.join(CRASH_DIRECTORY))
                .into_iter()
                .flatten()
                .flatten()
                .collect();
            assert_eq!(files.len(), usize::from(records), "{mode}");
            if mode == "embedded-off" {
                assert!(
                    !root.join("preferences.json").exists(),
                    "不能将旧配置的明确关闭改为默认开启"
                );
            }
            if records {
                let record = std::fs::read_to_string(files[0].path()).unwrap();
                assert!(record.starts_with("MSIME-CRASH/1 "), "{mode}");
                assert!(record.contains("\"component\":\"mcp\""), "{mode}");
                assert!(!record.contains("synthetic-private-argument"), "{mode}");
                if mode != "native" {
                    assert!(
                        record.contains("Unhandled Rust panic at crash_reporting.rs:"),
                        "{mode}"
                    );
                }
                let consent = PreferencesStore::new(root)
                    .usage_reporting_consent()
                    .unwrap()
                    .unwrap();
                let context = CrashContext::from_header(record.lines().next().unwrap()).unwrap();
                assert_eq!(
                    context.consent_epoch == Some(consent.epoch),
                    mode != "stale-late",
                    "{mode}"
                );
                let preferences = PreferencesStore::new(root);
                let store = with_consent_epoch(
                    &preferences,
                    &consent.epoch.to_string(),
                    &TelemetryStore::new(&directory),
                    |store, _| Ok(store.clone()),
                )
                .unwrap()
                .unwrap();
                let app = TelemetryApp::new("linux", "99.0.0").unwrap();
                store.record_active(&app, SystemTime::now()).unwrap();
                let events = store.queued().unwrap();
                let crashes: Vec<_> = events
                    .iter()
                    .filter(|event| event.kind == TelemetryKind::Crash)
                    .collect();
                assert_eq!(crashes.len(), usize::from(mode != "stale-late"), "{mode}");
                if let Some(crash) = crashes.first() {
                    assert_eq!(crash.version, env!("MSIME_APP_VERSION"), "{mode}");
                }
                assert!(
                    events
                        .iter()
                        .all(|event| !event.message.contains(&consent.epoch.to_string())),
                    "{mode}"
                );
                assert!(events.iter().all(|event| !matches!(
                    event.kind,
                    TelemetryKind::Session | TelemetryKind::SessionCrash
                )));
                assert!(!files[0].path().exists(), "{mode}");
            }
        }
    }
}

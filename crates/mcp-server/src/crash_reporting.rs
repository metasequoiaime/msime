//! MCP 仅在本地捕获自身致命故障，由输入法宿主补报；不记录调用参数或输入会话。

use crate::config::Config;
use msime_client_core::edition::Edition;
use msime_client_core::preferences::Preferences;
use msime_client_services::telemetry::{SessionStart, TelemetryApp, TelemetryStore};
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
    current: Option<(PathBuf, SessionStart)>,
    disabled_directory: Option<PathBuf>,
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
    fn refresh(&mut self, config: &Config) {
        let target = config.read_host_options().ok().and_then(|document| {
            let directory = directory(config.edition(&document).ok()?, &|name| {
                std::env::var_os(name)
            })?;
            let enabled = match document.pointer("/preferences/usage_reporting") {
                Some(value) => value.as_bool()?,
                None => Preferences::default().usage_reporting,
            };
            Some((directory, enabled))
        });
        let Some((directory, true)) = target.as_ref() else {
            unsafe { msime_native_crash_disarm() };
            self.current = None;
            if let Some((directory, false)) = target {
                if self.disabled_directory.as_ref() != Some(&directory)
                    && TelemetryStore::new(&directory).clear().is_ok()
                {
                    self.disabled_directory = Some(directory);
                }
            }
            return;
        };
        self.disabled_directory = None;
        if self
            .current
            .as_ref()
            .is_some_and(|(old, _)| old == directory)
        {
            return;
        }
        unsafe { msime_native_crash_disarm() };
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
        let Ok(capture) =
            TelemetryStore::new(directory).prepare_crash_capture(&app, SystemTime::now())
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
        // C 端立即复制字符串；记录路径不进入协议输出，补报继续由宿主持有。
        unsafe { msime_native_crash_arm(path.as_ptr(), header.as_ptr()) };
        self.current = Some((directory.clone(), capture));
    }
}

/// 与原生宿主按版本共用匿名安装身份和队列，不跟随可搬迁的词库目录。
fn directory(edition: &Edition, env: &impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let absolute = |name| {
        env(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    #[cfg(target_os = "macos")]
    {
        let root = absolute("HOME")?.join("Library/Application Support/MSIME");
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
    use msime_client_core::preferences::PreferencesStore;
    use msime_client_services::telemetry::{TelemetryKind, CRASH_DIRECTORY, SESSION_FILE};
    use serde_json::json;
    use std::path::Path;
    use std::process::Command;

    fn set_consent(root: &Path, enabled: bool) {
        let store = PreferencesStore::new(root);
        let mut snapshot = store.load().unwrap();
        snapshot.preferences.usage_reporting = enabled;
        store.save(snapshot.revision, snapshot.preferences).unwrap();
    }

    fn test_environment(root: &Path, name: &str) -> Option<OsString> {
        match name {
            "HOME" | "LOCALAPPDATA" | "XDG_STATE_HOME" => Some(root.as_os_str().to_owned()),
            _ => None,
        }
    }

    #[test]
    fn absent_or_relative_environment_does_not_select_a_directory() {
        assert!(directory(Edition::full(), &|_| None).is_none());
        assert!(directory(Edition::full(), &|_| Some("relative".into())).is_none());
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
            let _reporter = start(&config).expect("测试进程必须能观察同意开关");
            match mode.as_str() {
                "disable" | "enable" => {
                    set_consent(&root, mode == "enable");
                    std::thread::sleep(Duration::from_millis(1600));
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
            ("off", false, false),
            ("disable", true, false),
            ("enable", false, true),
            ("unreadable", true, false),
            ("recovered", true, false),
            #[cfg(unix)]
            ("native", true, true),
        ];
        for (mode, enabled, records) in modes {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary.path();
            std::fs::write(
                root.join("runtime-options.json"),
                serde_json::to_vec(&json!({"preferences_directory": root})).unwrap(),
            )
            .unwrap();
            set_consent(root, enabled);
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "crash_reporting::tests::child_process",
                    "--ignored",
                    "--nocapture",
                ])
                .env("MSIME_TEST_CRASH_ROOT", root)
                .env("MSIME_TEST_CRASH_MODE", mode)
                .env("HOME", root)
                .env("LOCALAPPDATA", root)
                .env("XDG_STATE_HOME", root)
                .output()
                .unwrap();
            assert_eq!(output.status.success(), mode == "recovered", "{mode}");
            let directory =
                directory(Edition::full(), &|name| test_environment(root, name)).unwrap();
            assert!(!directory.join(SESSION_FILE).exists(), "{mode}");
            let files: Vec<_> = std::fs::read_dir(directory.join(CRASH_DIRECTORY))
                .into_iter()
                .flatten()
                .flatten()
                .collect();
            assert_eq!(files.len(), usize::from(records), "{mode}");
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
                let store = TelemetryStore::new(&directory);
                let app = TelemetryApp::new("linux", "99.0.0").unwrap();
                store.record_active(&app, SystemTime::now()).unwrap();
                let events = store.queued().unwrap();
                let crashes: Vec<_> = events
                    .iter()
                    .filter(|event| event.kind == TelemetryKind::Crash)
                    .collect();
                assert_eq!(crashes.len(), 1, "{mode}");
                assert_eq!(crashes[0].version, env!("MSIME_APP_VERSION"), "{mode}");
                assert!(events.iter().all(|event| !matches!(
                    event.kind,
                    TelemetryKind::Session | TelemetryKind::SessionCrash
                )));
                assert!(!files[0].path().exists(), "{mode}");
            }
        }
    }
}

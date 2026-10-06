//! First-run preparation from the settings window.
//!
//! The packaged `msime-linux-setup` script owns the whole preparation: it verifies the dictionary lock, optionally downloads what is missing, runs `msime-linux-prepare`, enables the user units and adds the input method to the running Fcitx5 or IBus input method list, falling back to printing the manual steps. This module only locates that script, runs it for the state directory this window already reads, and streams its output to the page, so the terminal and the graphical paths cannot drift apart.
use serde::Serialize;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, State};

use crate::RuntimeOptionsState;

/// 本安装包所属版本的首次配置命令名（full 是 `msime-linux-setup`，其他版本是 `msime-linux-<id>-setup`）：设置应用只配置自己的版本。
fn setup_program_name() -> String {
    msime_client_core::edition::Edition::linux_package_identity_or_full().setup_program()
}
/// The first download is about 170 MB; a stalled mirror must still end the run rather than leave the page busy forever.
const SETUP_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const MAX_LINE_BYTES: usize = 2048;
const MAX_LINES: usize = 2000;
/// 安装包的 postinst 会在首次配置之前为每个登录用户注册匿名账号，所以状态目录在首次配置时通常已经存在、里面只有这两份文件。`msime-linux-setup` 接受只含这些文件的目录（脚本里的 `ANONYMOUS_ACCOUNT_FILES`），这里的清单必须与它一致。
const ANONYMOUS_ACCOUNT_FILES: [&str; 2] = ["anonymous-account.json", "anonymous-session.json"];
pub const SETUP_OUTPUT_EVENT: &str = "linux-setup-output";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxSetupStatus {
    prepared: bool,
    state_directory: Option<String>,
    /// 目录已存在、没有 runtime options，且不只有安装流程创建的匿名账号文件；脚本拒绝覆盖这样的目录。
    directory_occupied: bool,
    setup_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SetupLine {
    text: String,
    error: bool,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct LinuxSetupError {
    code: &'static str,
}

impl LinuxSetupError {
    const fn new(code: &'static str) -> Self {
        Self { code }
    }
}

#[derive(Default)]
pub struct LinuxSetupState(Arc<AtomicBool>);

/// The fixed locator every Linux frontend reads: `$XDG_CONFIG_HOME/msime-client/runtime-options.json`. A relative XDG value is ignored, as the specification requires.
///
/// 目录名随本安装包所属的版本（full 是 `msime-client`），与同一版本的宿主和脚本读的是同一份。
pub fn user_runtime_options() -> Option<PathBuf> {
    super::config_home(
        std::env::var_os("XDG_CONFIG_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
    .map(|directory| {
        directory
            .join(
                &msime_client_core::edition::Edition::linux_package_identity_or_full()
                    .client_directory,
            )
            .join("runtime-options.json")
    })
}

fn find_program(executable_dir: Option<&Path>, search_path: Option<&OsStr>) -> Option<PathBuf> {
    // The installer puts the script beside the desktop binary; prefer that copy so a second installation on PATH cannot prepare against another prefix's lock.
    let name = setup_program_name();
    executable_dir
        .map(|directory| directory.join(&name))
        .into_iter()
        .chain(
            search_path
                .map(|value| std::env::split_paths(value).collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .filter(|directory| directory.is_absolute())
                .map(|directory| directory.join(&name)),
        )
        .find(|path| path.is_file())
}

fn setup_program() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok();
    find_program(
        executable.as_deref().and_then(Path::parent),
        std::env::var_os("PATH").as_deref(),
    )
}

/// 与 `msime-linux-prepare` 的 `installer_account_state` 同一判定（它比脚本的 `anonymous_account_state` 多查属主和权限，以更严的为准）：属于当前用户、不对组和其他用户开放的非空目录，不是符号链接，其中每一项都是同样属主和权限的匿名账号普通文件。
fn only_anonymous_account(directory: &Path) -> bool {
    let private = |metadata: &fs::Metadata| {
        metadata.uid() == rustix::process::geteuid().as_raw() && metadata.mode() & 0o077 == 0
    };
    if !fs::symlink_metadata(directory)
        .is_ok_and(|metadata| metadata.is_dir() && private(&metadata))
    {
        return false;
    }
    let Ok(entries) =
        fs::read_dir(directory).and_then(|entries| entries.collect::<Result<Vec<_>, _>>())
    else {
        return false;
    };
    !entries.is_empty()
        && entries.iter().all(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| ANONYMOUS_ACCOUNT_FILES.contains(&name))
                && fs::symlink_metadata(entry.path())
                    .is_ok_and(|metadata| metadata.is_file() && private(&metadata))
        })
}

fn status_for(options: Option<&Path>, program: Option<&Path>) -> LinuxSetupStatus {
    let prepared = options.is_some_and(|path| {
        crate::shared::atomic_file::check_directory_ancestors(
            path.parent().unwrap_or_else(|| Path::new(".")),
        )
        .is_ok()
            && fs::symlink_metadata(path)
                .map(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
                .unwrap_or(false)
    });
    let directory = options.and_then(Path::parent);
    LinuxSetupStatus {
        prepared,
        state_directory: directory.map(|path| path.to_string_lossy().into_owned()),
        directory_occupied: !prepared
            && directory.is_some_and(|path| path.exists() && !only_anonymous_account(path)),
        setup_available: program.is_some(),
    }
}

fn setup_arguments(
    state_directory: &Path,
    download: bool,
    cloud_candidates: bool,
) -> Vec<OsString> {
    let mut arguments = vec![OsString::from("--state"), state_directory.into()];
    if download {
        arguments.push("--download".into());
    }
    if !cloud_candidates {
        arguments.push("--no-cloud-candidates".into());
    }
    arguments
}

/// Split a stream into lossy, bounded lines. An overlong line is cut rather than buffered without limit.
fn read_lines(stream: impl Read, mut emit: impl FnMut(String)) {
    let mut reader = BufReader::new(stream);
    let mut buffer = Vec::with_capacity(MAX_LINE_BYTES);
    loop {
        buffer.clear();
        let mut limited = (&mut reader).take(MAX_LINE_BYTES as u64);
        match limited.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                while buffer
                    .last()
                    .is_some_and(|byte| *byte == b'\n' || *byte == b'\r')
                {
                    buffer.pop();
                }
                emit(String::from_utf8_lossy(&buffer).into_owned());
            }
        }
    }
}

fn run_setup(
    program: &Path,
    arguments: &[OsString],
    timeout: Duration,
    emit: impl Fn(SetupLine) + Send + Sync + 'static,
) -> Result<(), LinuxSetupError> {
    let mut child = Command::new(program)
        .args(arguments)
        // Keep the setup script and anything it launches in a private process
        // group so a child that inherits stdout/stderr cannot outlive the
        // setup request or hold the reader threads open.
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // A piped Python stdout is block-buffered; without this the page would see nothing until the script ends.
        .env("PYTHONUNBUFFERED", "1")
        .spawn()
        .map_err(|_| LinuxSetupError::new("setup_unavailable"))?;
    let emit = Arc::new(emit);
    let emitted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let readers: Vec<_> = [
        child
            .stdout
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|stream| Box::new(stream) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .enumerate()
    .filter_map(|(index, stream)| stream.map(|stream| (index == 1, stream)))
    .map(|(error, stream)| {
        let emit = Arc::clone(&emit);
        let emitted = Arc::clone(&emitted);
        std::thread::spawn(move || {
            read_lines(stream, |text| {
                if emitted.fetch_add(1, Ordering::Relaxed) < MAX_LINES {
                    emit(SetupLine { text, error });
                }
            })
        })
    })
    .collect();
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    if let Some(pid) = rustix::process::Pid::from_raw(child.id() as _) {
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
    for reader in readers {
        let _ = reader.join();
    }
    match status {
        Some(status) if status.success() => Ok(()),
        Some(_) => Err(LinuxSetupError::new("setup_failed")),
        None => Err(LinuxSetupError::new("setup_timeout")),
    }
}

#[tauri::command]
pub fn linux_setup_status(options: State<'_, RuntimeOptionsState>) -> LinuxSetupStatus {
    status_for(options.path.as_deref(), setup_program().as_deref())
}

#[tauri::command]
pub async fn run_linux_setup(
    app: AppHandle,
    options: State<'_, RuntimeOptionsState>,
    running: State<'_, LinuxSetupState>,
    download: bool,
    cloud_candidates: bool,
) -> Result<LinuxSetupStatus, LinuxSetupError> {
    let program = setup_program().ok_or(LinuxSetupError::new("setup_unavailable"))?;
    let options_path = options
        .path
        .clone()
        .ok_or(LinuxSetupError::new("setup_unavailable"))?;
    let status = status_for(Some(&options_path), Some(&program));
    if status.prepared {
        return Ok(status);
    }
    if status.directory_occupied {
        return Err(LinuxSetupError::new("setup_directory_exists"));
    }
    let state_directory = options_path
        .parent()
        .ok_or(LinuxSetupError::new("setup_unavailable"))?
        .to_path_buf();
    if running.0.swap(true, Ordering::AcqRel) {
        return Err(LinuxSetupError::new("setup_running"));
    }
    let flag = Arc::clone(&running.0);
    let arguments = setup_arguments(&state_directory, download, cloud_candidates);
    let emitter = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = run_setup(&program, &arguments, SETUP_TIMEOUT, move |line| {
            let _ = emitter.emit(SETUP_OUTPUT_EVENT, line);
        });
        flag.store(false, Ordering::Release);
        result
    })
    .await
    .map_err(|_| LinuxSetupError::new("setup_failed"))?;
    result?;
    Ok(status_for(Some(&options_path), setup_program().as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Mutex;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "msime-linux-setup-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn script(directory: &Path, body: &str) -> PathBuf {
        let path = directory.join(setup_program_name());
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    #[test]
    fn the_copy_beside_the_binary_wins_over_path() {
        let root = scratch("find");
        let installed = root.join("bin");
        let other = root.join("other");
        std::fs::create_dir_all(&installed).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let on_path = script(&other, "true");
        let search = std::env::join_paths([PathBuf::from("relative"), other.clone()]).unwrap();
        assert_eq!(find_program(Some(&installed), Some(&search)), Some(on_path));
        let beside = script(&installed, "true");
        assert_eq!(find_program(Some(&installed), Some(&search)), Some(beside));
        assert_eq!(find_program(None, None), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn status_distinguishes_missing_prepared_and_occupied_state() {
        let root = scratch("status");
        let options = root.join("msime-client/runtime-options.json");
        let program = Path::new("/usr/bin/msime-linux-setup");
        let missing = status_for(Some(&options), None);
        assert!(!missing.prepared && !missing.directory_occupied && !missing.setup_available);
        std::fs::create_dir_all(options.parent().unwrap()).unwrap();
        let occupied = status_for(Some(&options), Some(program));
        assert!(!occupied.prepared && occupied.directory_occupied && occupied.setup_available);
        std::fs::write(&options, "{}").unwrap();
        let prepared = status_for(Some(&options), Some(program));
        assert!(prepared.prepared && !prepared.directory_occupied);
        assert_eq!(
            prepared.state_directory.as_deref(),
            Some(root.join("msime-client").to_str().unwrap())
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn status_lets_setup_continue_over_the_installer_anonymous_account() {
        let root = scratch("anonymous");
        let directory = root.join("msime-client");
        let options = directory.join("runtime-options.json");
        let program = Path::new("/usr/bin/msime-linux-setup");
        let mode = |path: &Path, mode| {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap()
        };
        std::fs::create_dir_all(&directory).unwrap();
        mode(&directory, 0o700);
        for name in ANONYMOUS_ACCOUNT_FILES {
            std::fs::write(directory.join(name), "{}").unwrap();
            mode(&directory.join(name), 0o600);
        }
        let status = status_for(Some(&options), Some(program));
        assert!(!status.prepared && !status.directory_occupied);

        // `msime-linux-prepare` 拒绝其他用户可读的状态，页面也不能在这种目录上提供配置。
        mode(&directory.join("anonymous-session.json"), 0o644);
        assert!(status_for(Some(&options), Some(program)).directory_occupied);
        mode(&directory.join("anonymous-session.json"), 0o600);
        mode(&directory, 0o755);
        assert!(status_for(Some(&options), Some(program)).directory_occupied);
        mode(&directory, 0o700);

        std::fs::write(directory.join("preferences.json"), "{}").unwrap();
        assert!(status_for(Some(&options), Some(program)).directory_occupied);
        std::fs::remove_file(directory.join("preferences.json")).unwrap();

        std::fs::create_dir(directory.join("anonymous-session.json.d")).unwrap();
        assert!(status_for(Some(&options), Some(program)).directory_occupied);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn status_does_not_treat_a_symlinked_locator_as_prepared() {
        use std::os::unix::fs::symlink;

        let root = scratch("symlink-status");
        let options = root.join("msime-client/runtime-options.json");
        std::fs::create_dir_all(options.parent().unwrap()).unwrap();
        let outside = root.join("outside.json");
        std::fs::write(&outside, b"{}").unwrap();
        symlink(&outside, &options).unwrap();

        let status = status_for(Some(&options), None);
        assert!(!status.prepared);
        assert!(status.directory_occupied);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_home_follows_xdg_and_ignores_relative_values() {
        let xdg = OsStr::new("/xdg");
        let home = OsStr::new("/home/user");
        assert_eq!(
            super::super::config_home(Some(xdg), Some(home)),
            Some(PathBuf::from("/xdg"))
        );
        assert_eq!(
            super::super::config_home(Some(OsStr::new("relative")), Some(home)),
            Some(PathBuf::from("/home/user/.config"))
        );
        assert_eq!(
            super::super::config_home(None, Some(OsStr::new("relative"))),
            None
        );
    }

    #[test]
    fn download_is_only_requested_when_the_user_allowed_it() {
        let state = Path::new("/home/user/.config/msime-client");
        assert_eq!(
            setup_arguments(state, false, true),
            ["--state", "/home/user/.config/msime-client"]
        );
        assert_eq!(
            setup_arguments(state, true, true),
            ["--state", "/home/user/.config/msime-client", "--download"]
        );
    }

    #[test]
    fn a_declined_cloud_candidate_choice_reaches_the_setup_script() {
        let state = Path::new("/home/user/.config/msime-client");
        assert_eq!(
            setup_arguments(state, false, false),
            [
                "--state",
                "/home/user/.config/msime-client",
                "--no-cloud-candidates"
            ]
        );
    }

    #[test]
    fn long_lines_are_cut_and_line_endings_removed() {
        let long = "x".repeat(MAX_LINE_BYTES + 10);
        let input = format!("first\r\n{long}\nlast");
        let mut lines = Vec::new();
        read_lines(input.as_bytes(), |line| lines.push(line));
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "first");
        assert_eq!(lines[1].len(), MAX_LINE_BYTES);
        assert_eq!(lines[2], "x".repeat(10));
        assert_eq!(lines[3], "last");
    }

    #[test]
    fn output_is_streamed_with_its_channel_and_failure_is_reported() {
        let root = scratch("run");
        let program = script(&root, "echo \"state $2\"; echo problem >&2");
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&lines);
        let arguments = setup_arguments(Path::new("/state"), false, true);
        run_setup(&program, &arguments, Duration::from_secs(10), move |line| {
            sink.lock().unwrap().push((line.text, line.error));
        })
        .unwrap();
        let mut lines = lines.lock().unwrap().clone();
        lines.sort();
        assert_eq!(
            lines,
            [
                ("problem".to_string(), true),
                ("state /state".to_string(), false)
            ]
        );
        let failing = script(&root, "exit 1");
        assert_eq!(
            run_setup(&failing, &arguments, Duration::from_secs(10), |_| {}),
            Err(LinuxSetupError::new("setup_failed"))
        );
        let hanging = script(&root, "exec sleep 30");
        assert_eq!(
            run_setup(&hanging, &arguments, Duration::from_millis(200), |_| {}),
            Err(LinuxSetupError::new("setup_timeout"))
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_background_child_cannot_hold_the_setup_output_open() {
        let root = scratch("background");
        let program = script(&root, "sleep 3 &");
        let started = Instant::now();

        assert_eq!(
            run_setup(&program, &[], Duration::from_secs(1), |_| {}),
            Ok(())
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        std::fs::remove_dir_all(root).unwrap();
    }
}

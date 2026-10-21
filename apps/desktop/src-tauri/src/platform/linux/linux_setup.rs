//! First-run preparation from the settings window.
//!
//! The packaged `msime-linux-setup` script owns the whole preparation: it verifies the dictionary lock, optionally downloads what is missing, runs `msime-linux-prepare`, enables the user units and adds the input method to the running Fcitx5 or IBus input method list, falling back to printing the manual steps. This module only locates that script, runs it for the state directory this window already reads, and streams its output to the page, so the terminal and the graphical paths cannot drift apart.
use serde::{Deserialize, Serialize};
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
/// 安装包的 postinst 会在首次配置之前为每个登录用户注册匿名账号，所以状态目录在首次配置时通常已经存在、里面只有这几份文件：身份、会话，以及 Rust 刷新匿名令牌时与设置应用共用的 `account-refresh.lock`。`msime-linux-setup` 接受只含这些文件的目录（脚本里的 `ANONYMOUS_ACCOUNT_FILES`），`msime-linux-prepare` 的 `installer_account_state` 也一样，三处清单必须一致。
const ANONYMOUS_ACCOUNT_FILES: [&str; 3] = [
    "anonymous-account.json",
    "anonymous-session.json",
    "account-refresh.lock",
];
pub const SETUP_OUTPUT_EVENT: &str = "linux-setup-output";
/// 配置进行到哪一步，驱动首次配置页的进度条；载荷是 `msime-linux-setup` 报告的阶段名。
pub const SETUP_STAGE_EVENT: &str = "linux-setup-stage";
/// 设置应用运行脚本时设置这个环境变量，脚本据此额外打印机读的阶段行和结果行（脚本里的 `report`）；在终端里运行时不打印。
const REPORT_ENV: &str = "MSIME_LINUX_SETUP_REPORT";
const STAGE_PREFIX: &str = "msime-setup-stage: ";
const RESULT_PREFIX: &str = "msime-setup-result: ";
const NOTE_PREFIX: &str = "msime-setup-note: ";
/// 结果卡片上最多放几条提示；脚本现在最多报告两条，多出来的仍在详细输出里。
const MAX_NOTES: usize = 8;
const STAGES: [&str; 6] = [
    "verify", "download", "prepare", "services", "register", "update",
];
/// 宿主启动时发现自行下载的词库落后于当前版本，引导脚本（`msime-linux-first-run-guide --reason dictionary-outdated`）留下的标记；设置应用据此提供「立即更新」，更新成功后删掉它。
const DICTIONARY_OUTDATED_MARKER: &str = "dictionary-outdated";
/// 首次配置窗口的大小：与 macOS 的安装窗口一样是一个居中的小窗口，比 macOS 高一些，放得下下载和云候选两个选项。
const SETUP_WINDOW_SIZE: (f64, f64) = (500.0, 640.0);
/// 设置窗口的大小和最小尺寸，与 tauri.linux.conf.json 里 `main` 的声明一致。
const SETTINGS_WINDOW_SIZE: (f64, f64) = (1000.0, 780.0);
const SETTINGS_WINDOW_MIN_SIZE: (f64, f64) = (360.0, 540.0);
/// 主窗口是否被 `shape_setup_window` 变成了首次配置窗口；只有这时 `leave_linux_setup_window` 才把它换回设置窗口的大小，用户自己调过大小的设置窗口不受影响。
static SETUP_WINDOW_SHAPED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxSetupStatus {
    prepared: bool,
    state_directory: Option<String>,
    /// 目录已存在、没有 runtime options，且不只有安装流程创建的匿名账号文件；脚本拒绝覆盖这样的目录。
    directory_occupied: bool,
    setup_available: bool,
    /// 宿主报告过自行下载的词库落后于当前版本（见 `DICTIONARY_OUTDATED_MARKER`），设置应用要先提供更新。
    dictionary_outdated: bool,
    /// 这次配置把输入法加入输入法列表的结果；只有刚跑完的配置才有。
    registration: Option<LinuxSetupRegistration>,
    /// 刚跑完的配置要用户留意的提示（GNOME Wayland 的 GTK_IM_MODULE、玲珑与 IBus、Fcitx5 托盘），配置成功时也显示在结果卡片上。
    notes: Vec<String>,
}

/// `msime-linux-setup` 最后报告的输入法列表情况：正在运行的框架（`fcitx5`、`ibus`，都没在运行时为空）以及水杉输入法是否已在它的列表里。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinuxSetupRegistration {
    host: Option<String>,
    registered: bool,
}

/// 脚本输出里的一行：机读的阶段、结果或提示，或者给人看的输出。
#[derive(Debug, PartialEq, Eq)]
enum ReportLine {
    Stage(&'static str),
    Result(LinuxSetupRegistration),
    Note(String),
    Output,
}

/// 脚本跑完后交给页面的机读结果。
#[derive(Debug, Default)]
struct SetupReport {
    registration: Option<LinuxSetupRegistration>,
    notes: Vec<String>,
}

/// 认出脚本的机读行。只认 stdout 上、阶段名在已知清单里、结果的框架是 `fcitx5` 或 `ibus`、提示是非空字符串的行；其余一律当作普通输出照常显示。提示行不进输出区：脚本已经另外打印了给人看的同一段文字。
fn report_line(line: &SetupLine) -> ReportLine {
    if line.error {
        return ReportLine::Output;
    }
    if let Some(stage) = line.text.strip_prefix(STAGE_PREFIX) {
        return serde_json::from_str::<String>(stage)
            .ok()
            .and_then(|stage| STAGES.into_iter().find(|known| *known == stage))
            .map_or(ReportLine::Output, ReportLine::Stage);
    }
    if let Some(note) = line.text.strip_prefix(NOTE_PREFIX) {
        return serde_json::from_str::<String>(note)
            .ok()
            .map(|note| note.trim().to_owned())
            .filter(|note| !note.is_empty())
            .map_or(ReportLine::Output, ReportLine::Note);
    }
    if let Some(result) = line.text.strip_prefix(RESULT_PREFIX) {
        return serde_json::from_str::<LinuxSetupRegistration>(result)
            .ok()
            .filter(|result| {
                result
                    .host
                    .as_deref()
                    .is_none_or(|host| host == "fcitx5" || host == "ibus")
            })
            .map_or(ReportLine::Output, ReportLine::Result);
    }
    ReportLine::Output
}

/// 引导脚本放标记的目录，与 `msime-linux-first-run-guide` 的 `guide_dir` 同一条规则：有绝对的 `XDG_RUNTIME_DIR` 时在它下面（随登录会话清空），否则在缓存目录下。两处都查，因为写标记时的环境不一定与设置应用相同。
///
/// 子目录名随本安装包所属的版本：`edition_linux.py` 把各版本引导脚本里的 `msime-client` 改写成该版本的目录（如 `msime-client-wubi`），这里必须读同一个，否则五笔版看不到自己的标记，并排安装时还会删掉 full 的标记。
fn dictionary_outdated_markers(
    client_directory: &str,
    runtime: Option<&OsStr>,
    cache: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Vec<PathBuf> {
    let runtime = runtime.map(PathBuf::from).filter(|path| path.is_absolute());
    let cache = cache
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            home.map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(".cache"))
        });
    runtime
        .into_iter()
        .chain(cache)
        .map(|directory| {
            directory
                .join(client_directory)
                .join(DICTIONARY_OUTDATED_MARKER)
        })
        .collect()
}

fn session_dictionary_outdated_markers() -> Vec<PathBuf> {
    dictionary_outdated_markers(
        &msime_client_core::edition::Edition::linux_package_identity_or_full().client_directory,
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        std::env::var_os("XDG_CACHE_HOME").as_deref(),
        std::env::var_os("HOME").as_deref(),
    )
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
        dictionary_outdated: false,
        registration: None,
        notes: Vec::new(),
    }
}

/// 加上宿主是否报告过词库过期。只对已配置的状态有意义：还没配置时首次配置本来就会备齐当前版本的词库。
fn with_dictionary_outdated(mut status: LinuxSetupStatus, markers: &[PathBuf]) -> LinuxSetupStatus {
    status.dictionary_outdated = status.prepared
        && markers.iter().any(|marker| {
            fs::symlink_metadata(marker).is_ok_and(|metadata| metadata.file_type().is_file())
        });
    status
}

/// 首次配置或词库更新要不要用小窗口：没有准备好的配置（首次配置页），或宿主报告了词库过期（更新页）。
pub fn setup_window_pending(options: Option<&Path>) -> bool {
    let status = with_dictionary_outdated(
        status_for(options, setup_program().as_deref()),
        &session_dictionary_outdated_markers(),
    );
    !status.prepared || status.dictionary_outdated
}

/// 把主窗口变成居中的首次配置窗口，与 macOS 的 `shape_first_install_window` 相同；离开时由 `leave_linux_setup_window` 换回设置窗口的大小。Wayland 下 `center` 可能不生效，窗口仍可用。
pub fn shape_setup_window(window: &tauri::WebviewWindow) {
    let (width, height) = SETUP_WINDOW_SIZE;
    let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let _ = window.set_resizable(false);
    let _ = window.set_maximizable(false);
    let _ = window.center();
    SETUP_WINDOW_SHAPED.store(true, Ordering::Release);
}

/// 离开首次配置（或词库更新）页时把主窗口换回设置窗口的大小。
#[tauri::command]
pub fn leave_linux_setup_window(window: tauri::WebviewWindow) -> Result<(), LinuxSetupError> {
    if !SETUP_WINDOW_SHAPED.swap(false, Ordering::AcqRel) {
        return Ok(());
    }
    let (width, height) = SETTINGS_WINDOW_SIZE;
    let (min_width, min_height) = SETTINGS_WINDOW_MIN_SIZE;
    let unavailable = |_| LinuxSetupError::new("setup_unavailable");
    window.set_resizable(true).map_err(unavailable)?;
    window.set_maximizable(true).map_err(unavailable)?;
    window
        .set_min_size(Some(tauri::LogicalSize::new(min_width, min_height)))
        .map_err(unavailable)?;
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(unavailable)?;
    window.center().map_err(unavailable)
}

/// 首次配置没能自动加入 Fcitx5 输入法组时，结果卡片上的「打开 Fcitx5 配置」：启动 `fcitx5-configtool`，用户在里面把水杉输入法加入当前输入法组。
#[tauri::command]
pub async fn open_fcitx5_configtool() -> Result<(), LinuxSetupError> {
    tauri::async_runtime::spawn_blocking(|| {
        super::linux_process::launch("fcitx5-configtool", &[], Duration::from_millis(300))
    })
    .await
    .map_err(|_| LinuxSetupError::new("configtool_unavailable"))?
    .then_some(())
    .ok_or(LinuxSetupError::new("configtool_unavailable"))
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
    if cloud_candidates {
        arguments.push("--cloud-candidates".into());
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
        .env(REPORT_ENV, "1")
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
    with_dictionary_outdated(
        status_for(options.path.as_deref(), setup_program().as_deref()),
        &session_dictionary_outdated_markers(),
    )
}

/// 在阻塞线程里跑脚本：给人看的行发到页面，阶段行发成进度事件，结果行和提示行留下来随返回值交给页面。
async fn run_reporting(
    app: AppHandle,
    running: &LinuxSetupState,
    program: PathBuf,
    arguments: Vec<OsString>,
) -> Result<SetupReport, LinuxSetupError> {
    if running.0.swap(true, Ordering::AcqRel) {
        return Err(LinuxSetupError::new("setup_running"));
    }
    let flag = Arc::clone(&running.0);
    let report = Arc::new(std::sync::Mutex::new(SetupReport::default()));
    let reported = Arc::clone(&report);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = run_setup(
            &program,
            &arguments,
            SETUP_TIMEOUT,
            move |line| match report_line(&line) {
                ReportLine::Stage(stage) => {
                    let _ = app.emit(SETUP_STAGE_EVENT, stage);
                }
                ReportLine::Result(result) => {
                    if let Ok(mut report) = reported.lock() {
                        report.registration = Some(result);
                    }
                }
                ReportLine::Note(note) => {
                    if let Ok(mut report) = reported.lock() {
                        if report.notes.len() < MAX_NOTES {
                            report.notes.push(note);
                        }
                    }
                }
                ReportLine::Output => {
                    let _ = app.emit(SETUP_OUTPUT_EVENT, line);
                }
            },
        );
        flag.store(false, Ordering::Release);
        result
    })
    .await
    .map_err(|_| LinuxSetupError::new("setup_failed"))?;
    result?;
    let report = report
        .lock()
        .map(|mut report| std::mem::take(&mut *report))
        .unwrap_or_default();
    Ok(report)
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
    let arguments = setup_arguments(&state_directory, download, cloud_candidates);
    let report = run_reporting(app, &running, program, arguments).await?;
    Ok(LinuxSetupStatus {
        registration: report.registration,
        notes: report.notes,
        ..status_for(Some(&options_path), setup_program().as_deref())
    })
}

/// 设置应用里的「立即更新」词库：与终端里的 `msime-linux-setup --update --download` 相同，取回当前版本的词库、切换代次并回放用户词库。成功后脚本自己删掉宿主留下的过期标记（终端里运行也一样）；这里再删一次，旧版脚本没有这一步时页面也不会再报过期。
#[tauri::command]
pub async fn run_linux_dictionary_update(
    app: AppHandle,
    options: State<'_, RuntimeOptionsState>,
    running: State<'_, LinuxSetupState>,
) -> Result<LinuxSetupStatus, LinuxSetupError> {
    let program = setup_program().ok_or(LinuxSetupError::new("setup_unavailable"))?;
    let options_path = options
        .path
        .clone()
        .ok_or(LinuxSetupError::new("setup_unavailable"))?;
    if !status_for(Some(&options_path), Some(&program)).prepared {
        return Err(LinuxSetupError::new("setup_unavailable"));
    }
    let state_directory = options_path
        .parent()
        .ok_or(LinuxSetupError::new("setup_unavailable"))?
        .to_path_buf();
    run_reporting(app, &running, program, update_arguments(&state_directory)).await?;
    let markers = session_dictionary_outdated_markers();
    for marker in &markers {
        if fs::symlink_metadata(marker).is_ok_and(|metadata| metadata.file_type().is_file()) {
            let _ = fs::remove_file(marker);
        }
    }
    Ok(with_dictionary_outdated(
        status_for(Some(&options_path), setup_program().as_deref()),
        &markers,
    ))
}

fn update_arguments(state_directory: &Path) -> Vec<OsString> {
    vec![
        OsString::from("--state"),
        state_directory.into(),
        "--update".into(),
        "--download".into(),
    ]
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

    /// Runs `body` through `/bin/sh -c` with `arguments` as `$1`, `$2`, ... instead of writing it to an executable file first. A test that writes an executable and execs it races every other test in the binary that forks a child at that moment: the child inherits the still-open write descriptor until it execs, and Linux refuses to exec a file open for writing (`ETXTBSY`), which `run_setup` reports as `setup_unavailable`.
    fn shell(body: &str, arguments: &[OsString]) -> (PathBuf, Vec<OsString>) {
        let mut shell_arguments = vec![OsString::from("-c"), body.into(), "sh".into()];
        shell_arguments.extend(arguments.iter().cloned());
        (PathBuf::from("/bin/sh"), shell_arguments)
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

    fn line(text: &str, error: bool) -> SetupLine {
        SetupLine {
            text: text.to_owned(),
            error,
        }
    }

    #[test]
    fn report_lines_are_recognised_only_in_their_exact_shape() {
        assert_eq!(
            report_line(&line("msime-setup-stage: \"prepare\"", false)),
            ReportLine::Stage("prepare")
        );
        assert_eq!(
            report_line(&line(
                "msime-setup-result: {\"host\": \"fcitx5\", \"registered\": true}",
                false
            )),
            ReportLine::Result(LinuxSetupRegistration {
                host: Some("fcitx5".into()),
                registered: true,
            })
        );
        assert_eq!(
            report_line(&line(
                "msime-setup-result: {\"host\": null, \"registered\": false}",
                false
            )),
            ReportLine::Result(LinuxSetupRegistration {
                host: None,
                registered: false,
            })
        );
        assert_eq!(
            report_line(&line(
                "msime-setup-note: \"提示：当前是 GNOME 的 Wayland 会话\"",
                false
            )),
            ReportLine::Note("提示：当前是 GNOME 的 Wayland 会话".into())
        );
        // 未知阶段、未知框架、空提示、不是字符串的提示、stderr 上的同样文字和普通输出都照常显示给用户。
        for (text, error) in [
            ("msime-setup-note: \"  \"", false),
            ("msime-setup-note: [\"提示\"]", false),
            ("msime-setup-note: \"提示\"", true),
            ("msime-setup-stage: \"format-disk\"", false),
            (
                "msime-setup-result: {\"host\": \"kime\", \"registered\": true}",
                false,
            ),
            ("msime-setup-result: not json", false),
            ("msime-setup-stage: \"prepare\"", true),
            ("词库已校验：/usr/share/msime-client/resources", false),
        ] {
            assert_eq!(
                report_line(&line(text, error)),
                ReportLine::Output,
                "{text}"
            );
        }
    }

    #[test]
    fn setup_reports_through_the_environment_and_keeps_report_lines_out_of_the_log() {
        let (program, arguments) = shell(
            "echo \"report=$MSIME_LINUX_SETUP_REPORT\"; echo 'msime-setup-stage: \"verify\"'",
            &[],
        );
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&lines);
        run_setup(&program, &arguments, Duration::from_secs(10), move |line| {
            sink.lock()
                .unwrap()
                .push(report_line(&line) == ReportLine::Output);
        })
        .unwrap();
        let mut lines = lines.lock().unwrap().clone();
        lines.sort();
        assert_eq!(lines, [false, true]);
    }

    #[test]
    fn dictionary_outdated_follows_the_guide_marker_once_prepared() {
        let root = scratch("outdated");
        let markers = dictionary_outdated_markers(
            "msime-client",
            Some(root.join("runtime").as_os_str()),
            None,
            Some(root.join("home").as_os_str()),
        );
        assert_eq!(
            markers,
            [
                root.join("runtime/msime-client/dictionary-outdated"),
                root.join("home/.cache/msime-client/dictionary-outdated"),
            ]
        );
        // 引导脚本按版本改写成写到该版本自己的目录（edition_linux.py 的 CLIENT_DIRECTORY 规则），这里读的是同一个；并排安装的两个版本各看各的标记，更新成功后也只删自己的那个。
        let mut edition_markers: Vec<PathBuf> = msime_client_core::edition::Edition::all()
            .iter()
            .filter_map(|edition| edition.linux())
            .flat_map(|linux| {
                dictionary_outdated_markers(
                    &linux.client_directory,
                    Some(root.join("runtime").as_os_str()),
                    None,
                    None,
                )
            })
            .collect();
        let count = edition_markers.len();
        assert!(count > 1);
        assert!(edition_markers
            .iter()
            .all(|marker| marker.ends_with("dictionary-outdated")));
        edition_markers.sort();
        edition_markers.dedup();
        assert_eq!(edition_markers.len(), count);
        // 相对路径不算数，与引导脚本一致。
        assert!(dictionary_outdated_markers(
            "msime-client",
            Some(OsStr::new("relative")),
            Some(OsStr::new("relative")),
            None
        )
        .is_empty());
        let options = root.join("config/msime-client/runtime-options.json");
        std::fs::create_dir_all(options.parent().unwrap()).unwrap();
        std::fs::write(&options, "{}").unwrap();
        let prepared = status_for(Some(&options), None);
        assert!(!with_dictionary_outdated(prepared.clone(), &markers).dictionary_outdated);
        std::fs::create_dir_all(markers[1].parent().unwrap()).unwrap();
        std::fs::write(&markers[1], "").unwrap();
        assert!(with_dictionary_outdated(prepared, &markers).dictionary_outdated);
        // 还没配置时首次配置本来就会备齐当前版本的词库，不提示更新。
        let missing = status_for(Some(&root.join("missing/runtime-options.json")), None);
        assert!(!with_dictionary_outdated(missing, &markers).dictionary_outdated);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dictionary_update_runs_the_same_update_as_the_terminal_command() {
        assert_eq!(
            update_arguments(Path::new("/home/user/.config/msime-client")),
            [
                "--state",
                "/home/user/.config/msime-client",
                "--update",
                "--download"
            ]
        );
    }

    #[test]
    fn download_is_only_requested_when_the_user_allowed_it() {
        let state = Path::new("/home/user/.config/msime-client");
        assert_eq!(
            setup_arguments(state, false, false),
            ["--state", "/home/user/.config/msime-client"]
        );
        assert_eq!(
            setup_arguments(state, true, false),
            ["--state", "/home/user/.config/msime-client", "--download"]
        );
    }

    #[test]
    fn cloud_candidates_are_only_requested_when_the_user_allowed_them() {
        let state = Path::new("/home/user/.config/msime-client");
        assert_eq!(
            setup_arguments(state, false, true),
            [
                "--state",
                "/home/user/.config/msime-client",
                "--cloud-candidates"
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
        let arguments = setup_arguments(Path::new("/state"), false, true);
        let (program, streaming) = shell("echo \"state $2\"; echo problem >&2", &arguments);
        let lines = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&lines);
        run_setup(&program, &streaming, Duration::from_secs(10), move |line| {
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
        let (failing, failing_arguments) = shell("exit 1", &arguments);
        assert_eq!(
            run_setup(
                &failing,
                &failing_arguments,
                Duration::from_secs(10),
                |_| {}
            ),
            Err(LinuxSetupError::new("setup_failed"))
        );
        let (hanging, hanging_arguments) = shell("exec sleep 30", &arguments);
        assert_eq!(
            run_setup(
                &hanging,
                &hanging_arguments,
                Duration::from_millis(200),
                |_| {}
            ),
            Err(LinuxSetupError::new("setup_timeout"))
        );
    }

    #[test]
    fn a_background_child_cannot_hold_the_setup_output_open() {
        let (program, arguments) = shell("sleep 3 &", &[]);
        let started = Instant::now();

        assert_eq!(
            run_setup(&program, &arguments, Duration::from_secs(1), |_| {}),
            Ok(())
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}

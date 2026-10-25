//! 设置页的检查更新（`msime_client_services::update_check`）。
//!
//! 检查放在这里而不是在 webview 里 `fetch`，是为了让每个宿主都问同一份 Rust 代码：哪个发布更新、哪个安装包属于本版本和本架构；原生 Windows 设置窗口和 HarmonyOS 页面经由 `msime_client_update_check` 调到同一个函数。

use crate::CommandError;
use msime_client_services::update_check::{
    check_for_update, UpdateCheck, UpdateCheckError, UpdateCheckRequest,
};
#[cfg(any(target_os = "linux", test))]
use msime_client_services::update_check::{ReleaseUpdate, UpdateChannel};

fn update_check_error(error: UpdateCheckError) -> CommandError {
    CommandError {
        code: match error {
            UpdateCheckError::Invalid => "update_check_invalid",
            UpdateCheckError::Unavailable => "update_check_unavailable",
        },
    }
}

/// 把 `platform` 在 `channel`（缺省为稳定版，见 `UpdateCheckRequest`）里最新的发布与 `current_version` 比较。在阻塞线程池上等待 GitHub，每个请求最多十秒。
#[tauri::command]
pub async fn update_check(
    platform: String,
    current_version: String,
    edition: Option<String>,
    arch: Option<String>,
    channel: Option<String>,
) -> Result<UpdateCheck, CommandError> {
    let request = UpdateCheckRequest {
        platform,
        current_version,
        edition,
        arch,
        channel,
    };
    tauri::async_runtime::spawn_blocking(move || check_for_update(&request))
        .await
        .map_err(|_| CommandError {
            code: "update_check_unavailable",
        })?
        .map_err(update_check_error)
}

/// 只为带完整更新配置的 macOS 安装包提供原生更新入口。
#[tauri::command]
pub fn application_update_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        msime_host_macos::settings_update_supported()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

/// Sparkle 的标准界面负责下载、验签、替换应用和重启，始终运行在 AppKit 主线程。`channel` 是关于页当前选的更新通道（偏好 `update_channel` 的值，可能还没自动保存），决定读哪一份设置应用的 appcast（见 `msime_host_macos::check_settings_update_on_channel`）；不传时读稳定版的 feed，与以前的页面一致。不认识的通道名不检查，报 `application_update_unavailable`。
#[tauri::command]
pub async fn application_update_check(
    app: tauri::AppHandle,
    channel: Option<String>,
) -> Result<(), CommandError> {
    #[cfg(target_os = "macos")]
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let started = match channel {
                Some(channel) => msime_host_macos::check_settings_update_on_channel(&channel),
                None => msime_host_macos::check_settings_update(),
            };
            let _ = sender.send(started);
        })
        .map_err(|_| CommandError {
            code: "application_update_unavailable",
        })?;
        tauri::async_runtime::spawn_blocking(move || receiver.recv().unwrap_or(false))
            .await
            .unwrap_or(false)
            .then_some(())
            .ok_or(CommandError {
                code: "application_update_unavailable",
            })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, channel);
        Err(CommandError {
            code: "application_update_unavailable",
        })
    }
}

// ---- Linux 后台更新检查 ----
//
// macOS 的输入法每天在后台检查一次、静默下载并在切换输入源时安装；Linux 的安装包归包管理器管（deb、rpm、AUR、OBS、nix），不能在应用里安装，所以这里只做检查和提醒：打开设置窗口时（启动时或把已在运行的设置进程叫到前台时；只为面板启动的设置进程不检查）调用共享的后台检查（`msime_client_services::update_check::background`），通道按偏好 `update_channel`。多久问一次、连不上时多久再试、换了通道立刻重查、同一个构建（版本号和运行 ID）只提醒一次，都由共享层决定，记录是状态目录下的 `update-check.json`。这里只负责 Linux 自己的部分：发现该提醒的新版本时发一条桌面通知，点通知上的「查看」打开设置的「关于」页，关于页直接显示找到的新版本；发通知的那个进程退出以后再打开关于页也能看到，直到运行的构建追上它或换了通道。「查看」只在发通知的进程还在时有效（它要等 notify-send 退出才知道点了哪个动作），之后从设置窗口打开「关于」页即可。检查在设置应用里跑，不在 Fcitx5 插件或 IBus 宿主里联网。开发构建、构建时或运行时设置了 `MSIME_DISABLE_UPDATE_CHECK` 时不做后台检查（见 `background::automatic_checks_disabled`），关于页的「检查更新」按钮照常可用。请求的内容与「检查更新」按钮相同，见 PRIVACY.md「资源与更新下载」。

/// 等用户点通知最多等多久；之后结束等待的 notify-send，不留下一直挂着的进程。
#[cfg(target_os = "linux")]
const NOTIFICATION_WAIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// 后台检查的状态目录（放 `update-check.json` 和 `update-check.lock`），后台检查和关于页读记录都用它。
#[cfg(target_os = "linux")]
pub(crate) struct BackgroundUpdateState(std::path::PathBuf);

#[cfg(target_os = "linux")]
impl BackgroundUpdateState {
    pub(crate) fn new(directory: &std::path::Path) -> Self {
        Self(directory.to_path_buf())
    }
}

/// Linux 设置应用发给共享后台检查的请求：平台 `linux`、本机架构，以及本安装包的版本 id 和要跟的通道。后台检查和关于页读记录用同一个形状，记录才对得上。
#[cfg(any(target_os = "linux", test))]
pub(crate) fn linux_update_request(
    current_version: String,
    edition: Option<String>,
    channel: UpdateChannel,
) -> UpdateCheckRequest {
    UpdateCheckRequest {
        platform: "linux".to_owned(),
        current_version,
        edition,
        arch: Some(std::env::consts::ARCH.to_owned()),
        channel: Some(channel.as_str().to_owned()),
    }
}

/// 偏好里选的更新通道；偏好读不出来时按稳定版。
#[cfg(target_os = "linux")]
fn preferred_update_channel(app: &tauri::AppHandle) -> UpdateChannel {
    use tauri::Manager;
    app.try_state::<std::sync::Arc<msime_client_core::preferences::PreferencesStore>>()
        .and_then(|store| store.load().ok())
        .map(|snapshot| snapshot.preferences.update_channel)
        .unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn current_update_request(app: &tauri::AppHandle, channel: UpdateChannel) -> UpdateCheckRequest {
    linux_update_request(
        app.package_info().version.to_string(),
        crate::host_edition(app).map(|edition| edition.id.clone()),
        channel,
    )
}

/// 关于页的「版本与更新」先显示后台检查已经找到的新版本，不必再点一次「检查更新」。页面传来的 `channel` 是它当前选的通道（可能还没自动保存），不传时按偏好里的；记录是按别的通道检查的、或者找到的版本已经不比运行的构建新时为 `None`。只读一个小文件，不联网。
#[tauri::command]
pub fn pending_update_check(app: tauri::AppHandle, channel: Option<String>) -> Option<UpdateCheck> {
    #[cfg(target_os = "linux")]
    {
        use msime_client_services::update_check::background;
        use tauri::Manager;
        let state = app.try_state::<BackgroundUpdateState>()?;
        let channel = match channel {
            Some(value) => UpdateChannel::parse(&value)?,
            None => preferred_update_channel(&app),
        };
        background::pending_update(&state.0, &current_update_request(&app, channel))
            .ok()
            .flatten()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, channel);
        None
    }
}

/// 在后台线程里做一次共享后台检查（未到时间则什么也不做），见本节开头的说明。设置窗口启动时和被叫到前台时都调用它；上一次还没结束时不再起一个。
#[cfg(target_os = "linux")]
pub(crate) fn start_linux_background_update_check(app: &tauri::AppHandle) {
    use msime_client_services::update_check::background;
    use std::sync::atomic::{AtomicBool, Ordering};
    use tauri::Manager;
    static CHECKING: AtomicBool = AtomicBool::new(false);
    if tauri::is_dev() || background::automatic_checks_disabled() {
        return;
    }
    let Some(directory) = app
        .try_state::<BackgroundUpdateState>()
        .map(|state| state.0.clone())
    else {
        return;
    };
    if CHECKING.swap(true, Ordering::AcqRel) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("msime-update-check".to_owned())
        .spawn(move || {
            run_linux_background_update_check(&app, &directory);
            CHECKING.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        CHECKING.store(false, Ordering::Release);
    }
}

#[cfg(target_os = "linux")]
fn run_linux_background_update_check(app: &tauri::AppHandle, directory: &std::path::Path) {
    use msime_client_services::update_check::background;
    let request = current_update_request(app, preferred_update_channel(app));
    // 共享层返回时已经把要提醒的构建记成提醒过了，这里发一次通知即可；记录写不进去（Err）时什么也不发。
    if let Ok(background::BackgroundOutcome {
        notify: Some(update),
        ..
    }) = background::run_background_check(directory, &request)
    {
        notify_linux_update(app, &update);
    }
}

/// 通知的标题和正文。标题带上通道：测试版和每日构建写明是哪一种，每日构建还带构建号（同一个版本号每天都有新的每日构建）。正式版由包管理器更新；测试版和每日构建不进软件源，只能从发布页下载。
#[cfg(any(target_os = "linux", test))]
pub(crate) fn update_notification_text(
    name: &str,
    update: &ReleaseUpdate,
) -> (String, &'static str) {
    const STABLE: &str = "Linux 版由安装它的包管理器更新（apt、dnf、pacman 或 AUR 等），也可以从发布页下载对应的安装包。打开设置的「关于」页查看这个版本。";
    const PRERELEASE: &str = "测试版和每日构建不经包管理器分发，请从发布页下载对应的安装包。打开设置的「关于」页查看这个版本。";
    let version = &update.version.display;
    match (update.channel, update.build_run) {
        (UpdateChannel::Stable, _) => (format!("{name}有新版本 v{version}"), STABLE),
        (UpdateChannel::Beta, _) => (format!("{name}有新的测试版 v{version}"), PRERELEASE),
        (UpdateChannel::Nightly, Some(run)) => (
            format!("{name}有新的每日构建 v{version}（构建 {run}）"),
            PRERELEASE,
        ),
        (UpdateChannel::Nightly, None) => (format!("{name}有新的每日构建 v{version}"), PRERELEASE),
    }
}

/// 通知用的应用名和图标名：随本安装包所属的版本（五笔版是「水杉五笔」和 `msime-linux-wubi`），与引导脚本按版本改写出的通知一致；full 是「水杉输入法」和 `msime-linux`。设置程序在各版本之间共用，写死 full 的名字时五笔版的通知会找不到图标，并排安装时两条通知也分不出是谁的。
#[cfg(any(target_os = "linux", test))]
pub(crate) fn update_notification_identity(
    edition: &msime_client_core::edition::Edition,
) -> Option<(&str, &str)> {
    Some((
        edition.display_name.zh_hans.as_str(),
        edition.linux()?.package.as_str(),
    ))
}

/// 发「有新版本」的桌面通知。notify-send 支持动作（libnotify 0.7.9 起的 `--action`）时，点通知上的「查看」打开设置的「关于」页；它要等通知被点掉或关闭才退出，所以在单独的线程里等，最多等 `NOTIFICATION_WAIT`。
#[cfg(target_os = "linux")]
fn notify_linux_update(app: &tauri::AppHandle, update: &ReleaseUpdate) {
    use crate::platform::linux::linux_process;
    use msime_client_core::edition::Edition;
    let edition = Edition::of_linux_package().unwrap_or_else(|_| Edition::full());
    let Some((name, icon)) = update_notification_identity(edition) else {
        return;
    };
    let (title, body) = update_notification_text(name, update);
    let actions = linux_process::read_text(
        "notify-send",
        &["--help"],
        64 * 1024,
        std::time::Duration::from_secs(2),
    )
    .is_some_and(|help| help.contains("--action"));
    if !actions {
        linux_process::launch(
            "notify-send",
            &["-a", name, "-i", icon, &title, body],
            std::time::Duration::from_millis(300),
        );
        return;
    }
    let Ok(mut child) = std::process::Command::new("notify-send")
        .args(["-a", name, "-i", icon, "--action=about=查看", &title, body])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return;
    };
    let deadline = std::time::Instant::now() + NOTIFICATION_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
    let mut chosen = String::new();
    if let Some(mut stdout) = child.stdout.take() {
        use std::io::Read;
        let _ = (&mut stdout).take(256).read_to_string(&mut chosen);
    }
    if chosen.trim() == "about" {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            crate::activate_desktop_surface(
                &handle,
                msime_client_core::host_surface::SurfaceRoute::Settings(Some(
                    msime_client_core::host_surface::SettingsCategory::About,
                )),
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_services::update_check::Version;

    fn release(display: &str, channel: UpdateChannel, build_run: Option<u64>) -> ReleaseUpdate {
        ReleaseUpdate {
            version: Version {
                display: display.to_owned(),
                parts: display
                    .split('.')
                    .map(|part| part.parse().unwrap())
                    .collect(),
            },
            release_url: "https://github.com/metasequoiaime/msime/releases/tag/linux-v9.9.9".into(),
            installer_name: None,
            installer_sha256: None,
            signed: Some(false),
            channel,
            build_run,
        }
    }

    #[test]
    fn notification_names_the_installed_edition() {
        use msime_client_core::edition::Edition;
        assert_eq!(
            update_notification_identity(Edition::full()),
            Some(("水杉输入法", "msime-linux"))
        );
        let wubi = Edition::by_id("wubi").unwrap();
        assert_eq!(
            update_notification_identity(wubi),
            Some(("水杉五笔", "msime-linux-wubi"))
        );
    }

    #[test]
    fn the_notification_title_names_the_channel_and_the_nightly_build() {
        let (title, body) = update_notification_text(
            "水杉输入法",
            &release("9.9.9", UpdateChannel::Stable, Some(5)),
        );
        assert_eq!(title, "水杉输入法有新版本 v9.9.9");
        assert!(body.contains("包管理器更新"));
        let (title, body) =
            update_notification_text("水杉五笔", &release("9.9.9", UpdateChannel::Beta, Some(5)));
        assert_eq!(title, "水杉五笔有新的测试版 v9.9.9");
        assert!(body.contains("不经包管理器分发"));
        let (title, _) = update_notification_text(
            "水杉输入法",
            &release("9.9.9", UpdateChannel::Nightly, Some(123)),
        );
        assert_eq!(title, "水杉输入法有新的每日构建 v9.9.9（构建 123）");
        let (title, _) = update_notification_text(
            "水杉输入法",
            &release("9.9.9", UpdateChannel::Nightly, None),
        );
        assert_eq!(title, "水杉输入法有新的每日构建 v9.9.9");
    }

    #[test]
    fn background_and_about_page_requests_name_the_channel() {
        let request = linux_update_request(
            "1.2.3".to_owned(),
            Some("wubi".to_owned()),
            UpdateChannel::Nightly,
        );
        assert_eq!(request.platform, "linux");
        assert_eq!(request.current_version, "1.2.3");
        assert_eq!(request.edition.as_deref(), Some("wubi"));
        assert_eq!(request.arch.as_deref(), Some(std::env::consts::ARCH));
        assert_eq!(request.channel.as_deref(), Some("nightly"));
        assert_eq!(request.effective_channel(), Some(UpdateChannel::Nightly));
    }

    #[test]
    fn a_record_written_by_the_old_settings_app_still_reads_through_the_shared_layer() {
        use msime_client_services::update_check::background;
        // 以前的 Linux 设置应用自己写的记录：没有 channel 和 notified_run。共享层把它读成稳定版的记录，关于页照样显示找到的版本、同一个版本不再提醒一次。
        let directory = tempfile::tempdir().unwrap();
        let old = serde_json::json!({
            "checked_at": 1,
            "attempted_at": 1,
            "notified_version": "9.9.9",
            "available": {
                "version": {"display": "9.9.9", "parts": [9, 9, 9]},
                "release_url": "https://github.com/metasequoiaime/msime/releases/tag/linux-v9.9.9",
                "installer_name": null,
                "installer_sha256": null,
                "signed": false
            }
        });
        std::fs::write(
            directory.path().join(background::RECORD_FILE),
            serde_json::to_vec(&old).unwrap(),
        )
        .unwrap();
        let stable = linux_update_request("1.2.3".to_owned(), None, UpdateChannel::Stable);
        assert_eq!(
            background::pending_update(directory.path(), &stable).unwrap(),
            Some(UpdateCheck::Available(release(
                "9.9.9",
                UpdateChannel::Stable,
                None
            )))
        );
        // 换到测试版以后，稳定版找到的版本不再显示。
        let beta = linux_update_request("1.2.3".to_owned(), None, UpdateChannel::Beta);
        assert_eq!(
            background::pending_update(directory.path(), &beta).unwrap(),
            None
        );
    }
}

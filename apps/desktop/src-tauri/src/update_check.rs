//! 设置页的检查更新（`msime_client_services::update_check`）。
//!
//! 检查放在这里而不是在 webview 里 `fetch`，是为了让每个宿主都问同一份 Rust 代码：哪个发布更新、哪个安装包属于本版本和本架构；原生 Windows 设置窗口和 HarmonyOS 页面经由 `msime_client_update_check` 调到同一个函数。

use crate::CommandError;
#[cfg(any(target_os = "linux", test))]
use msime_client_services::update_check::ReleaseUpdate;
use msime_client_services::update_check::{
    check_for_update, UpdateCheck, UpdateCheckError, UpdateCheckRequest,
};
#[cfg(any(target_os = "linux", test))]
use serde::{Deserialize, Serialize};

fn update_check_error(error: UpdateCheckError) -> CommandError {
    CommandError {
        code: match error {
            UpdateCheckError::Invalid => "update_check_invalid",
            UpdateCheckError::Unavailable => "update_check_unavailable",
        },
    }
}

/// 把 `platform` 最新的已发布版本与 `current_version` 比较。在阻塞线程池上等待 GitHub，最多十秒。
#[tauri::command]
pub async fn update_check(
    platform: String,
    current_version: String,
    edition: Option<String>,
    arch: Option<String>,
) -> Result<UpdateCheck, CommandError> {
    let request = UpdateCheckRequest {
        platform,
        current_version,
        edition,
        arch,
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

/// Sparkle 的标准界面负责下载、验签、替换应用和重启，始终运行在 AppKit 主线程。
#[tauri::command]
pub async fn application_update_check(app: tauri::AppHandle) -> Result<(), CommandError> {
    #[cfg(target_os = "macos")]
    {
        let (sender, receiver) = std::sync::mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = sender.send(msime_host_macos::check_settings_update());
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
        let _ = app;
        Err(CommandError {
            code: "application_update_unavailable",
        })
    }
}

// ---- Linux 后台更新检查 ----
//
// macOS 的输入法每天在后台检查一次、静默下载并在切换输入源时安装；Linux 的安装包归包管理器管（deb、rpm、AUR、OBS、nix），不能在应用里安装，所以这里只做检查和提醒：设置应用启动时（打开设置窗口或任何一个面板都会启动它）若距上次检查已满一天，就在后台问一次发布列表，发现新版本时发一条桌面通知，点通知打开设置的「关于」页，关于页直接显示找到的新版本。找到的版本记进 `update-check.json`，发通知的那个进程退出以后再打开关于页也能看到，直到运行的版本追上它。通知上的「查看」只在发通知的进程还在时有效（它要等 notify-send 退出才知道点了哪个动作），之后从设置窗口打开「关于」页即可。检查在设置应用里跑，不在 Fcitx5 插件或 IBus 宿主里联网。

/// 两次后台检查之间至少隔多久，与 macOS 输入法的每日检查一致。
#[cfg(any(target_os = "linux", test))]
const BACKGROUND_INTERVAL_SECONDS: u64 = 24 * 60 * 60;
/// 状态目录里记下上次检查时间和已提醒过的版本的文件。
#[cfg(target_os = "linux")]
const BACKGROUND_RECORD: &str = "update-check.json";
/// 这份记录只有几个短字段，读的时候限制大小，坏文件当作从没检查过。
#[cfg(target_os = "linux")]
const BACKGROUND_RECORD_LIMIT: u64 = 4096;
/// 等用户点通知最多等多久；之后结束等待的 notify-send，不留下一直挂着的进程。
#[cfg(target_os = "linux")]
const NOTIFICATION_WAIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// 上次后台检查的记录。
#[cfg(any(target_os = "linux", test))]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BackgroundRecord {
    /// 上次检查的 Unix 时间（秒）。
    pub(crate) checked_at: u64,
    /// 已经发过通知的版本号，同一个版本只提醒一次。
    #[serde(default)]
    pub(crate) notified_version: Option<String>,
    /// 上次检查找到的新版本。关于页打开时显示它，不必等下一次检查；运行的版本追上它以后不再显示。
    #[serde(default)]
    pub(crate) available: Option<ReleaseUpdate>,
}

/// 现在是否该再检查一次：满一天，或者时钟被往回拨过（记录在将来）。
#[cfg(any(target_os = "linux", test))]
pub(crate) fn background_check_due(record: &BackgroundRecord, now: u64) -> bool {
    now < record.checked_at || now - record.checked_at >= BACKGROUND_INTERVAL_SECONDS
}

/// 这次检查结果要不要发通知：有新版本，且这个版本还没提醒过。返回要提醒的版本号。
#[cfg(any(target_os = "linux", test))]
pub(crate) fn background_notification(
    check: &UpdateCheck,
    record: &BackgroundRecord,
) -> Option<String> {
    match check {
        UpdateCheck::Available(update)
            if record.notified_version.as_deref() != Some(update.version.display.as_str()) =>
        {
            Some(update.version.display.clone())
        }
        _ => None,
    }
}

/// 关于页要显示的后台检查结果：记录里找到的新版本仍高于运行的版本时才算，升级以后旧记录不再提示。
#[cfg(any(target_os = "linux", test))]
pub(crate) fn pending_update(
    record: &BackgroundRecord,
    current_version: &str,
) -> Option<UpdateCheck> {
    use msime_client_services::update_check::{compare_versions, parse_version};
    let update = record.available.as_ref()?;
    let current = parse_version(current_version)?;
    compare_versions(&update.version, &current)
        .is_gt()
        .then(|| UpdateCheck::Available(update.clone()))
}

/// 后台检查的记录文件（状态目录下的 `update-check.json`），关于页从这里读上次找到的新版本。
#[cfg(target_os = "linux")]
pub(crate) struct BackgroundUpdateState(std::path::PathBuf);

#[cfg(target_os = "linux")]
impl BackgroundUpdateState {
    pub(crate) fn new(directory: &std::path::Path) -> Self {
        Self(directory.join(BACKGROUND_RECORD))
    }
}

/// 关于页的「版本与更新」先显示后台检查已经找到的新版本，不必再点一次「检查更新」。
#[tauri::command]
pub fn pending_update_check(app: tauri::AppHandle) -> Option<UpdateCheck> {
    #[cfg(target_os = "linux")]
    {
        use tauri::Manager;
        let state = app.try_state::<BackgroundUpdateState>()?;
        pending_update(
            &read_background_record(&state.0),
            &app.package_info().version.to_string(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        None
    }
}

#[cfg(target_os = "linux")]
fn read_background_record(path: &std::path::Path) -> BackgroundRecord {
    use std::io::Read;
    std::fs::File::open(path)
        .ok()
        .and_then(|file| {
            let mut text = String::new();
            file.take(BACKGROUND_RECORD_LIMIT)
                .read_to_string(&mut text)
                .ok()?;
            serde_json::from_str(&text).ok()
        })
        .unwrap_or_default()
}

/// 在后台线程里检查一次（未满一天则什么也不做），见本节开头的说明。
#[cfg(target_os = "linux")]
pub(crate) fn start_linux_background_update_check(
    app: tauri::AppHandle,
    directory: std::path::PathBuf,
) {
    let _ = std::thread::Builder::new()
        .name("msime-update-check".to_owned())
        .spawn(move || {
            let path = directory.join(BACKGROUND_RECORD);
            let record = read_background_record(&path);
            let Ok(now) = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs())
            else {
                return;
            };
            if !background_check_due(&record, now) {
                return;
            }
            let request = UpdateCheckRequest {
                platform: "linux".to_owned(),
                current_version: app.package_info().version.to_string(),
                edition: crate::host_edition(&app).map(|edition| edition.id.clone()),
                arch: Some(std::env::consts::ARCH.to_owned()),
            };
            // 连不上时不记时间，下次启动再试；检查成功才算这一天检查过了。
            let Ok(check) = check_for_update(&request) else {
                return;
            };
            let notify = background_notification(&check, &record);
            let updated = BackgroundRecord {
                checked_at: now,
                notified_version: notify.clone().or(record.notified_version),
                available: match check {
                    UpdateCheck::Available(update) => Some(update),
                    UpdateCheck::Current(_) | UpdateCheck::None => None,
                },
            };
            if let Ok(text) = serde_json::to_vec(&updated) {
                let _ = crate::shared::atomic_file::write(&path, &text);
            }
            if let Some(version) = notify {
                notify_linux_update(&app, &version);
            }
        });
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
fn notify_linux_update(app: &tauri::AppHandle, version: &str) {
    use crate::platform::linux::linux_process;
    use msime_client_core::edition::Edition;
    let edition = Edition::of_linux_package().unwrap_or_else(|_| Edition::full());
    let Some((name, icon)) = update_notification_identity(edition) else {
        return;
    };
    let title = format!("{name}有新版本 v{version}");
    let body = "Linux 版由安装它的包管理器更新（apt、dnf、pacman 或 AUR 等），也可以从发布页下载对应的安装包。打开设置的「关于」页查看这个版本。";
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
    use msime_client_services::update_check::{ReleaseUpdate, Version};

    fn release(display: &str) -> ReleaseUpdate {
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
        }
    }

    #[test]
    fn background_check_runs_once_a_day_and_after_the_clock_moves_back() {
        let record = BackgroundRecord {
            checked_at: 1_000_000,
            notified_version: None,
            available: None,
        };
        assert!(background_check_due(
            &BackgroundRecord::default(),
            1_000_000
        ));
        assert!(!background_check_due(&record, 1_000_000 + 60));
        assert!(!background_check_due(
            &record,
            1_000_000 + BACKGROUND_INTERVAL_SECONDS - 1
        ));
        assert!(background_check_due(
            &record,
            1_000_000 + BACKGROUND_INTERVAL_SECONDS
        ));
        assert!(background_check_due(&record, 999_000));
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
    fn the_about_page_keeps_showing_a_found_update_until_it_is_installed() {
        let record = BackgroundRecord {
            checked_at: 1,
            notified_version: Some("9.9.9".to_owned()),
            available: Some(release("9.9.9")),
        };
        // 发通知的进程退出以后，关于页仍从记录里读到这个版本。
        assert_eq!(
            pending_update(&record, "1.2.3"),
            Some(UpdateCheck::Available(release("9.9.9")))
        );
        // 已经升级到（或越过）这个版本时不再提示。
        assert_eq!(pending_update(&record, "9.9.9"), None);
        assert_eq!(pending_update(&record, "10.0.0"), None);
        assert_eq!(pending_update(&BackgroundRecord::default(), "1.2.3"), None);
        // 记录写成文件再读回来是同一个版本。
        let text = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<BackgroundRecord>(&text).unwrap(),
            record
        );
    }

    #[test]
    fn each_new_version_is_announced_once() {
        let fresh = BackgroundRecord::default();
        assert_eq!(
            background_notification(&UpdateCheck::Available(release("9.9.9")), &fresh),
            Some("9.9.9".to_owned())
        );
        let told = BackgroundRecord {
            checked_at: 1,
            notified_version: Some("9.9.9".to_owned()),
            available: None,
        };
        assert_eq!(
            background_notification(&UpdateCheck::Available(release("9.9.9")), &told),
            None
        );
        assert_eq!(
            background_notification(&UpdateCheck::Available(release("9.10.0")), &told),
            Some("9.10.0".to_owned())
        );
        assert_eq!(
            background_notification(&UpdateCheck::Current(release("9.9.9")), &fresh),
            None
        );
        assert_eq!(background_notification(&UpdateCheck::None, &fresh), None);
        // 记录的形状：旧文件没有 `notified_version` 也读得出来。
        assert_eq!(
            serde_json::from_str::<BackgroundRecord>("{\"checked_at\":5}").unwrap(),
            BackgroundRecord {
                checked_at: 5,
                notified_version: None,
                available: None,
            }
        );
    }
}

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(test, not(target_os = "android"))
))]
mod ai;
mod clipboard_history;
#[cfg(not(target_os = "android"))]
mod dictionary_import;
// Only the two hosts that have to replay input into another window build this.
// macOS delivers through the input method itself and needs none of it.
mod notices;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod panel_input;
mod panel_window;
mod platform;
mod shared;
mod vocabulary;
mod voice;

// The refactor that moved panel delivery out of the crate root left these calls
// behind unqualified, and nothing compiled the two hosts that build the module,
// so the shells stayed broken. Name them explicitly rather than glob-importing:
// the module already does `use crate::*`, and a glob back would make every
// shared name ambiguous.
#[cfg(target_os = "linux")]
use clipboard_history::{start_linux_clipboard_monitor, write_linux_clipboard};
// Everything but the group gated on both hosts is one host's own. Windows reaches its foreground window through send_panel_key_windows and send_panel_text_windows, which the call sites already name directly.
#[cfg(any(target_os = "linux", target_os = "windows"))]
use panel_input::{
    cloud_clipboard_input_target, record_panel_typing_statistics, remember_opening_panel_target,
    remember_panel_input_target, CLOUD_CLIPBOARD_PANEL,
};
#[cfg(target_os = "linux")]
use panel_input::{
    panel_input_target, panel_position, send_panel_ctrl_v, send_panel_key, send_panel_text,
    send_panel_voice_text,
};
#[cfg(target_os = "windows")]
use panel_input::{send_panel_key_windows, send_panel_text_windows, windows_panel_position};

#[cfg(target_os = "android")]
use platform::android::android_account;
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
use platform::desktop::{
    desktop_account, desktop_candidate_skin_community, desktop_community_report,
    desktop_plugin_community, desktop_plugins, desktop_preferences_monitor,
};
#[cfg(target_os = "ios")]
use platform::ios::ios_account;
#[cfg(target_os = "linux")]
use platform::linux::{
    linux_account, linux_audio_devices, linux_data_directory, linux_process,
    linux_provider_credentials, linux_setup,
};
#[cfg(target_os = "macos")]
use platform::macos::{
    macos_account, macos_cloud_clipboard, macos_cloud_dictionary, macos_data_directory,
    macos_handwriting, macos_input_source, macos_keyboard, macos_launch, macos_panel_session,
};
#[cfg(any(target_os = "ios", target_os = "android"))]
use platform::mobile::mobile_account_helpers::parse_cloud_dictionary_request;
#[cfg(any(target_os = "ios", target_os = "android"))]
use platform::mobile::mobile_community;
#[cfg(windows)]
use platform::windows::{windows_account, windows_voice};

use msime_client_core::clipboard::ClipboardHistoryStore;
use msime_client_core::host_surface::{HostCapabilities, HostPlatform, SurfaceRoute};
use msime_client_core::panels::{
    HandwritingRecognitionRequest, HandwritingRecognitionResult, KeyboardInputRequest,
};
use msime_client_core::preferences::{
    Preferences, PreferencesError, PreferencesSnapshot, PreferencesStore,
};
use msime_client_core::skin::custom_library::{
    CustomSkinLibraryAction, CustomSkinLibraryError, CustomSkinLibraryStore, SavedTouchKeyboardSkin,
};
use msime_client_core::skin::keyboard_trial::KeyboardSkinTrialStore;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use msime_client_core::typing_statistics::TypingSource;
use msime_client_core::typing_statistics::{TypingStatistics, TypingStatisticsStore};
#[cfg(target_os = "android")]
use msime_tauri_mobile_platform::{AndroidVoicePlatform, AndroidVoicePolishRequest};
// Both mobile hosts build the same transcription request; only the transport differs.
#[cfg(any(target_os = "ios", test))]
use msime_tauri_mobile_platform::IosKeyboardAiPreferences;
#[cfg(target_os = "ios")]
use msime_tauri_mobile_platform::MobilePlatform;
#[cfg(any(target_os = "ios", target_os = "android", test))]
use msime_tauri_mobile_platform::MobileVoiceRequestHeader;
#[cfg(any(target_os = "ios", target_os = "android", test))]
use msime_tauri_mobile_platform::MobileVoiceTranscriptionRequest;
// The packaged recognizer runs on every host; only the socket provider is unix.
#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
use msime_input_runtime::UnixSocketProvider;
use msime_input_runtime::{HandwritingPoint, HandwritingQuery};
use serde_json::Value;
use std::collections::HashMap;
#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "android",
    target_os = "ios",
    test
))]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::io::Write;
#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
use std::os::unix::fs::FileTypeExt;
#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "android",
    target_os = "ios",
    test
))]
use std::path::Path;
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tauri::Emitter;
use tauri::Manager;

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use msime_host_api::mcp_clients;
use msime_host_api::system_fonts;
use shared::export_file;
use shared::skin_directory;
#[cfg(any(target_os = "linux", target_os = "windows"))]
use shared::voice::voice_output;
#[cfg(any(
    all(unix, not(any(target_os = "ios", target_os = "android"))),
    target_os = "windows"
))]
use shared::voice::voice_sessions;

const MAX_RUNTIME_OPTIONS_CANDIDATE_CAPACITY: usize = 5;

#[tauri::command]
fn supports_font_catalog() -> bool {
    font_catalog_supported()
}

/// iOS lists UIKit's families through the mobile-platform plugin rather than host-api, the way the HarmonyOS page asks ArkUI.
fn font_catalog_supported() -> bool {
    cfg!(target_os = "ios") || system_fonts::supported()
}

/// The platform this shell is running on. The shared UI previously inferred this
/// from `navigator.userAgent`, which hid working controls on every host the
/// regex did not name.
pub(crate) fn host_platform() -> HostPlatform {
    if cfg!(target_os = "windows") {
        HostPlatform::Windows
    } else if cfg!(target_os = "macos") {
        HostPlatform::Macos
    } else if cfg!(target_os = "android") {
        HostPlatform::Android
    } else if cfg!(target_os = "ios") {
        HostPlatform::Ios
    } else {
        HostPlatform::Linux
    }
}

pub(crate) fn clipboard_history_uses_preference(platform: HostPlatform) -> bool {
    !matches!(platform, HostPlatform::Ios)
}

/// The surface a native host asked this shell to present, from `--route=<route>`,
/// `MSIME_CLIENT_ROUTE`, or the superseded `MSIME_CLIENT_PANEL`. An unparseable
/// route opens the ordinary settings window rather than failing startup.
fn requested_surface_route() -> Option<SurfaceRoute> {
    let argument = std::env::args()
        .skip(1)
        .find_map(|argument| argument.strip_prefix("--route=").map(str::to_string));
    let requested = argument
        .or_else(|| std::env::var("MSIME_CLIENT_ROUTE").ok())
        // Superseded by --route=; kept so existing menu launchers keep working.
        .or_else(|| std::env::var("MSIME_CLIENT_PANEL").ok())?;
    SurfaceRoute::parse(requested.trim()).ok()
}

#[tauri::command]
fn host_capabilities(app: tauri::AppHandle) -> HostCapabilities {
    let mut capabilities = HostCapabilities::for_platform(host_platform());
    // Font enumeration is a build-time capability, not a platform assumption.
    capabilities.system_fonts = font_catalog_supported();
    capabilities.os_version = macos_product_version();
    capabilities.candidate_panel_limit = linux_candidate_panel_limit();
    let host_options = app
        .try_state::<DictionaryHostOptions>()
        .and_then(|options| options.snapshot().ok());
    drop_uninstalled_language_schemes(
        &mut capabilities,
        host_options.as_ref(),
        cfg!(target_os = "windows"),
    );
    capabilities
}

/// Cantonese and Zhuyin each read a dictionary the package installs beside the Engine resources, which the HostOptions document names in `language_dictionaries` only when one is there. Without its dictionary host-api falls back from the scheme, so the page shows it unavailable instead of offering a choice that never takes effect. Every other scheme needs nothing beyond the resources.
///
/// `beside_resources` is for Windows, whose `runtime-options.json` is written once at first run and never refreshed: the Server and the TIP each find the dictionaries beside the resources in memory, so a document without the key still means the `language-dictionaries` directory next to its absolute `resources`.
fn drop_uninstalled_language_schemes(
    capabilities: &mut HostCapabilities,
    host_options: Option<&Value>,
    beside_resources: bool,
) {
    use msime_client_core::preferences::InputScheme;
    let named = host_options
        .and_then(|document| document.get("language_dictionaries"))
        .and_then(Value::as_str)
        .map(PathBuf::from);
    let beside = || {
        host_options
            .and_then(|document| document.get("resources"))
            .and_then(Value::as_str)
            .map(std::path::Path::new)
            .filter(|resources| resources.is_absolute())
            .and_then(std::path::Path::parent)
            .map(|parent| parent.join("language-dictionaries"))
    };
    let directory = match named {
        Some(directory) => Some(directory),
        None if beside_resources => beside(),
        None => None,
    }
    .filter(|directory| directory.is_absolute());
    capabilities.input_schemes.retain(|scheme| {
        let dictionary = match scheme {
            InputScheme::Cantonese => "cantonese.db",
            InputScheme::Zhuyin => "zhuyin.db",
            _ => return true,
        };
        directory
            .as_deref()
            .is_some_and(|directory| directory.join(dictionary).is_file())
    });
}

/// What the running Linux host found about the desktop's candidate panel. Only the host knows which panel draws its list - GNOME Shell's popup, a Fcitx5 theme the user picked, the desktop's Kimpanel - so it writes that finding to a per-session file and the page reads it here instead of guessing from the desktop name.
#[cfg(target_os = "linux")]
fn linux_candidate_panel_limit() -> Option<msime_client_core::host_surface::CandidatePanelLimit> {
    use msime_client_core::host_surface::CandidatePanelLimit;
    let file = CandidatePanelLimit::status_file(std::env::var_os("XDG_RUNTIME_DIR").as_deref())?;
    CandidatePanelLimit::from_host_status(&read_candidate_panel_status(&file)?)
}

#[cfg(any(target_os = "linux", test))]
const CANDIDATE_PANEL_STATUS_READ_LIMIT: u64 = 4096;

#[cfg(any(target_os = "linux", test))]
fn read_candidate_panel_status(path: &Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let bytes =
        crate::shared::bounded_body::read_bounded(file, CANDIDATE_PANEL_STATUS_READ_LIMIT as usize)
            .ok()?;
    String::from_utf8(bytes).ok()
}

#[cfg(not(target_os = "linux"))]
fn linux_candidate_panel_limit() -> Option<msime_client_core::host_surface::CandidatePanelLimit> {
    None
}

/// The macOS release, read straight out of the file the system keeps it in.
///
/// `sw_vers` would answer the same question, but it answers it by spawning a
/// process on the settings window's first paint, and this is one `read_to_string`
/// of a file that is XML on every release this client supports. Anything
/// unexpected in it is not worth reporting a guess for, so the caller gets
/// `None` and the page falls back to the web view's own user agent.
#[cfg(target_os = "macos")]
fn macos_product_version() -> Option<String> {
    product_version_from_plist(
        &std::fs::read_to_string("/System/Library/CoreServices/SystemVersion.plist").ok()?,
    )
}

#[cfg(not(target_os = "macos"))]
fn macos_product_version() -> Option<String> {
    None
}

/// `ProductVersion` out of that plist, or `None` if what is there is not a
/// release number. The check is the point: this string is attached to a report
/// the user files, so it reports what the file says or nothing at all.
#[cfg(any(target_os = "macos", test))]
pub(crate) fn product_version_from_plist(plist: &str) -> Option<String> {
    let rest = plist.split_once("<key>ProductVersion</key>")?.1;
    let value = rest
        .split_once("<string>")?
        .1
        .split_once("</string>")?
        .0
        .trim();
    (!value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.'))
    .then(|| value.to_owned())
}

/// The settings section a host menu asked for, if any. The launcher passes it
/// in the environment, like the panel routes; the settings page falls back to
/// its own default when this is absent or unusable.
#[tauri::command]
fn initial_settings_page() -> Option<String> {
    // A `settings:<category>` route is the contract every host now shares; the
    // dedicated variable stays as the compatibility path for older launchers.
    settings_page_from_route(requested_surface_route()).or_else(|| {
        requested_settings_page(std::env::var("MSIME_CLIENT_SETTINGS_PAGE").ok().as_deref())
    })
}

/// The settings category a surface route names, if it names one.
fn settings_page_from_route(route: Option<SurfaceRoute>) -> Option<String> {
    route
        .and_then(SurfaceRoute::settings_category)
        .map(|category| category.as_str().to_owned())
}

fn requested_settings_page(value: Option<&str>) -> Option<String> {
    // Only a short identifier is accepted here; the page list itself lives in
    // the shared settings UI, which refuses ids it does not have.
    value
        .map(str::trim)
        .filter(|page| {
            !page.is_empty()
                && page.len() <= 32
                && page
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
        })
        .map(str::to_owned)
}

#[tauri::command]
async fn list_font_families(app: tauri::AppHandle) -> Result<Vec<String>, CommandError> {
    #[cfg(target_os = "ios")]
    let listed = {
        let platform = app
            .try_state::<MobilePlatform<tauri::Wry>>()
            .ok_or(CommandError {
                code: "font_catalog",
            })?
            .inner()
            .clone();
        tauri::async_runtime::spawn_blocking(move || {
            platform.list_font_families().map_err(|_| "font_catalog")
        })
    };
    #[cfg(not(target_os = "ios"))]
    let listed = {
        let _ = app;
        tauri::async_runtime::spawn_blocking(system_fonts::list)
    };
    listed
        .await
        .map_err(|_| CommandError {
            code: "font_catalog",
        })?
        .map_err(|code| CommandError { code })
}

#[tauri::command]
async fn resolve_font_families(names: Vec<String>) -> Result<Vec<String>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || system_fonts::resolve_css_families(names))
        .await
        .map_err(|_| CommandError {
            code: "font_family",
        })?
        .map_err(|code| CommandError { code })
}

#[tauri::command]
async fn list_voice_capture_devices() -> Result<Value, CommandError> {
    #[cfg(target_os = "linux")]
    {
        let devices = tauri::async_runtime::spawn_blocking(linux_audio_devices::list)
            .await
            .map_err(|_| CommandError {
                code: "audio_devices",
            })?;
        serde_json::to_value(devices).map_err(|_| CommandError {
            code: "audio_devices",
        })
    }
    #[cfg(target_os = "windows")]
    {
        let devices = tauri::async_runtime::spawn_blocking(msime_host_api::voice_capture_devices)
            .await
            .map_err(|_| CommandError {
                code: "audio_devices",
            })?;
        serde_json::to_value(
            devices
                .into_iter()
                .map(|(id, label)| {
                    serde_json::json!({
                        "backend": "windows",
                        "id": id,
                        "label": label
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|_| CommandError {
            code: "audio_devices",
        })
    }
    #[cfg(target_os = "macos")]
    {
        let devices = tauri::async_runtime::spawn_blocking(msime_host_macos::voice_capture_devices)
            .await
            .map_err(|_| CommandError {
                code: "audio_devices",
            })?;
        serde_json::to_value(
            devices
                .into_iter()
                .map(|(id, label)| {
                    serde_json::json!({
                        "backend": "macos",
                        "id": id,
                        "label": label
                    })
                })
                .collect::<Vec<_>>(),
        )
        .map_err(|_| CommandError {
            code: "audio_devices",
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    Err(CommandError {
        code: "unavailable",
    })
}

#[tauri::command]
async fn capture_voice_pcm(milliseconds: u32) -> Result<Vec<f32>, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        msime_host_api::voice_capture_pcm(milliseconds).map_err(|code| CommandError { code })
    })
    .await
    .map_err(|_| CommandError {
        code: "audio_capture",
    })?
}

/// The app data directory of `app.msime.client`, the identifier Windows and Linux shared with the other desktop shells before each got its own (`tauri.windows.conf.json`, `tauri.linux.conf.json`). Nothing but this shell ever used it, so what earlier versions left there is read where the current directory has nothing, and never moved.
#[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
pub(crate) fn legacy_app_data_dir(app_data: &Path) -> PathBuf {
    app_data.with_file_name("app.msime.client")
}

#[derive(Clone)]
struct ClipboardHistoryState(Arc<Mutex<ClipboardHistoryStore>>);
#[derive(Clone)]
struct DictionaryHostOptions {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    path: PathBuf,
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    document: Arc<Value>,
}

impl DictionaryHostOptions {
    fn snapshot(&self) -> Result<Value, CommandError> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            // Keep the installer-selected path separate from the IBus runtime path; deployments can supply different files for these roles.
            let mut document =
                read_runtime_options(&self.path).map_err(|_| CommandError { code: "storage" })?;
            // By default this is the same file the skin catalog is published into, and the Host API rejects the unknown field, so it is dropped here as the IBus and Fcitx5 hosts drop it before their own calls.
            if let Some(object) = document.as_object_mut() {
                object.remove("candidate_skin_catalog");
            }
            Ok(document)
        }
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        {
            Ok((*self.document).clone())
        }
    }
}

struct SkinDirectoryState(PathBuf);
/// The Engine's user directory: where the documents a user writes by hand live, `custom_translations.txt` among them.
struct UserDirectoryState(PathBuf);
struct TypingStatisticsState(TypingStatisticsStore);
/// The shared preferences directory, where the input method writes `diagnostic.log` when its diagnostic switch is on.
struct DiagnosticLogState(PathBuf);

/// The application data directory the 背单词 store and wordbook library live under.
///
/// The directory rather than the stores themselves: an imported book is written through one and
/// read back through the other, and holding the path means both are constructed from the same
/// place every time instead of two handles that could be pointed at different roots.
struct VocabularyState(std::path::PathBuf, std::path::PathBuf);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TypingStatisticsStatus {
    statistics: TypingStatistics,
    availability: &'static str,
    last_written_ms: Option<u64>,
}

fn typing_statistics_status(
    store: &TypingStatisticsStore,
    statistics: TypingStatistics,
) -> Result<TypingStatisticsStatus, CommandError> {
    let last_written = store
        .last_written()
        .map_err(|_| CommandError { code: "storage" })?;
    Ok(TypingStatisticsStatus {
        statistics,
        availability: if last_written.is_some() {
            "ready"
        } else {
            "neverWritten"
        },
        last_written_ms: last_written.and_then(|time| {
            time.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        }),
    })
}

#[tauri::command]
async fn load_typing_statistics(
    state: tauri::State<'_, TypingStatisticsState>,
) -> Result<TypingStatisticsStatus, CommandError> {
    let store = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let statistics = store.load().map_err(|_| CommandError { code: "storage" })?;
        typing_statistics_status(&store, statistics)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
async fn set_typing_statistics_enabled(
    state: tauri::State<'_, TypingStatisticsState>,
    enabled: bool,
) -> Result<TypingStatisticsStatus, CommandError> {
    let store = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let statistics = store
            .set_enabled(enabled)
            .map_err(|_| CommandError { code: "storage" })?;
        #[cfg(target_os = "macos")]
        msime_host_macos::notify_typing_statistics_enabled(enabled);
        typing_statistics_status(&store, statistics)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
async fn set_typing_statistics_retention(
    state: tauri::State<'_, TypingStatisticsState>,
    retention: String,
) -> Result<TypingStatisticsStatus, CommandError> {
    let store = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // The window counts back from the user's day, and this process is the one that knows
        // which day that is - the same reason a recorded commit carries one.
        let now =
            time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
        let date = now.date();
        let today = format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            u8::from(date.month()),
            date.day()
        );
        let statistics = store
            .set_retention(
                msime_client_core::typing_statistics::StatisticsRetention::parse(&retention),
                &today,
            )
            .map_err(|_| CommandError { code: "storage" })?;
        typing_statistics_status(&store, statistics)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
async fn reset_typing_statistics(
    state: tauri::State<'_, TypingStatisticsState>,
) -> Result<TypingStatisticsStatus, CommandError> {
    let store = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let statistics = store
            .reset()
            .map_err(|_| CommandError { code: "storage" })?;
        typing_statistics_status(&store, statistics)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Reveal the folder holding the statistics file.
///
/// The page states that the statistics never leave this machine; this is how that claim can be
/// checked rather than taken on trust. The host picks the folder - the webview cannot name one.
#[tauri::command]
async fn open_typing_statistics_directory(
    state: tauri::State<'_, TypingStatisticsState>,
) -> Result<(), CommandError> {
    let root = state.0.directory().to_path_buf();
    open_directory(root).await
}

async fn open_directory(root: PathBuf) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || skin_directory::open(&root))
        .await
        .map_err(|_| CommandError { code: "storage" })?
        .map_err(|code| CommandError { code })
}

/// What the settings page's diagnostic-log action should show: the log file itself where the platform can select a file in its file manager and the file exists, otherwise the directory that will hold it.
#[derive(Debug, PartialEq, Eq)]
enum DiagnosticLogTarget {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    File(PathBuf),
    Directory(PathBuf),
}

fn diagnostic_log_target(directory: &std::path::Path) -> DiagnosticLogTarget {
    #[cfg(target_os = "macos")]
    {
        let file = directory.join("diagnostic.log");
        if file.is_file() {
            return DiagnosticLogTarget::File(file);
        }
    }
    DiagnosticLogTarget::Directory(directory.to_path_buf())
}

#[cfg(target_os = "macos")]
fn reveal_file_in_finder(file: &std::path::Path) -> Result<(), &'static str> {
    let status = std::process::Command::new("open")
        .arg("-R")
        .arg(file)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|_| "storage")?;
    status.success().then_some(()).ok_or("storage")
}

/// Reveal the input method's diagnostic log so it can be sent after a reproduction.
///
/// MSIME-Windows writes its log to the Desktop to make it easy to find; on macOS the file stays in the preferences directory under Application Support and this action selects it in Finder instead. The host picks the location - the webview cannot name one.
#[tauri::command]
async fn open_diagnostic_log_directory(
    state: tauri::State<'_, DiagnosticLogState>,
) -> Result<(), CommandError> {
    let directory = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || match diagnostic_log_target(&directory) {
        #[cfg(target_os = "macos")]
        DiagnosticLogTarget::File(file) => reveal_file_in_finder(&file),
        #[cfg(not(target_os = "macos"))]
        DiagnosticLogTarget::File(_) => Err("unavailable"),
        DiagnosticLogTarget::Directory(root) => skin_directory::open(&root),
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
    .map_err(|code| CommandError { code })
}

/// Write a document the settings page exported into the user's Downloads folder and return the path.
///
/// The page names the file and the host picks the folder, the same outcome as the Windows source's WebView2 download. A download link cannot do it here: the WKWebView behind the macOS window cancels downloads it has no handler for. A taken name becomes `name (2).txt` rather than being overwritten.
#[tauri::command]
async fn save_export(
    app: tauri::AppHandle,
    name: String,
    contents: String,
) -> Result<String, CommandError> {
    let directory = app
        .path()
        .download_dir()
        .map_err(|_| CommandError { code: "storage" })?;
    tauri::async_runtime::spawn_blocking(move || export_file::save(&directory, &name, &contents))
        .await
        .map_err(|_| CommandError { code: "storage" })?
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|code| CommandError { code })
}

/// `msime-mcp` beside this executable, the runtime options it would be pointed at, the entry to paste into an assistant, and whether each assistant offered here already has it.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tauri::command]
async fn mcp_server_status(
    runtime: tauri::State<'_, RuntimeOptionsState>,
) -> Result<mcp_clients::McpServerStatus, CommandError> {
    let options = runtime.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let executable = std::env::current_exe().map_err(|_| CommandError { code: "storage" })?;
        mcp_clients::status(&executable, options.as_deref(), |name| {
            std::env::var_os(name)
        })
        .map_err(|code| CommandError { code })
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Write the entry into `client`'s configuration file. A different `msime` entry there fails with `mcp_entry_exists` unless `replace` is set, so the page asks before overwriting it.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tauri::command]
async fn install_mcp_client(
    runtime: tauri::State<'_, RuntimeOptionsState>,
    client: mcp_clients::McpClient,
    replace: bool,
) -> Result<mcp_clients::InstallOutcome, CommandError> {
    let options = runtime.path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let executable = std::env::current_exe().map_err(|_| CommandError {
            code: "mcp_server_missing",
        })?;
        mcp_clients::install_client(&executable, options.as_deref(), client, replace, |name| {
            std::env::var_os(name)
        })
        .map_err(|code| CommandError { code })
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

fn read_skin_toolbar_stylesheet_at(
    root: PathBuf,
    id: &str,
) -> Result<Option<String>, CommandError> {
    msime_client_core::skin::catalog::read_toolbar_stylesheet(root, id)
        .map_err(|_| CommandError { code: "storage" })
}

#[tauri::command]
async fn read_skin_toolbar_stylesheet(
    directory: tauri::State<'_, SkinDirectoryState>,
    id: String,
) -> Result<Option<String>, CommandError> {
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || read_skin_toolbar_stylesheet_at(root, &id))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

fn read_skin_stylesheet_at(
    root: PathBuf,
    id: &str,
    relative: &str,
) -> Result<String, CommandError> {
    msime_client_core::skin::catalog::read_stylesheet(root, id, relative)
        .map_err(|_| CommandError { code: "storage" })
}

#[tauri::command]
async fn read_skin_stylesheet(
    directory: tauri::State<'_, SkinDirectoryState>,
    id: String,
    relative: String,
) -> Result<String, CommandError> {
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || read_skin_stylesheet_at(root, &id, &relative))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

/// The user's own candidate glosses, as a document the settings page edits.
///
/// The reference has the user drop `custom_translations.txt` into the profile directory and says so in
/// its documentation. That instruction does not survive the move to macOS, where the same directory
/// lives under `~/Library` and the Finder hides it by default, so the overlay was reachable on paper
/// and not in practice. The page already knows how to edit the document - it was only ever handed to
/// HarmonyOS - so the host supplies the two ends, and the Engine keeps reading the same file.
const CUSTOM_TRANSLATIONS_MAX_BYTES: usize = 1024 * 1024;

fn custom_translations_path(user: &std::path::Path) -> PathBuf {
    user.join("custom_translations.txt")
}

fn read_custom_translations_at(user: PathBuf) -> Result<String, CommandError> {
    crate::shared::atomic_file::check_directory_ancestors(&user)
        .map_err(|_| CommandError { code: "storage" })?;
    let path = custom_translations_path(&user);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        // No overlay yet is the ordinary state, not a failure: the page opens on an empty document.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(_) => return Err(CommandError { code: "storage" }),
    };
    if !metadata.file_type().is_file() {
        return Err(CommandError { code: "storage" });
    }
    let file = std::fs::File::open(path).map_err(|_| CommandError { code: "storage" })?;
    let bytes = crate::shared::bounded_body::read_bounded(file, CUSTOM_TRANSLATIONS_MAX_BYTES)
        .map_err(|_| CommandError { code: "storage" })?;
    // A UTF-8 BOM is an encoding marker the reference accepts, not part of the first source word.
    let text = String::from_utf8(bytes).map_err(|_| CommandError { code: "storage" })?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned())
}

fn write_custom_translations_at(user: PathBuf, text: &str) -> Result<(), CommandError> {
    if text.len() > CUSTOM_TRANSLATIONS_MAX_BYTES || text.contains('\0') {
        return Err(CommandError {
            code: "invalid_document",
        });
    }
    let path = custom_translations_path(&user);
    // An emptied document means "no overlay". Removing the file says that; leaving an empty one
    // behind would have the Engine open and read an empty set every session instead.
    if text.trim().is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(CommandError { code: "storage" }),
        };
    }
    // Use a fresh private sibling and publish it atomically. This avoids
    // following a pre-existing staging symlink and leaves the previous overlay
    // intact if writing or syncing fails.
    crate::shared::atomic_file::write(&path, text.as_bytes())
        .map_err(|_| CommandError { code: "storage" })
}

#[tauri::command]
async fn read_custom_translations(
    directory: tauri::State<'_, UserDirectoryState>,
) -> Result<String, CommandError> {
    let user = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || read_custom_translations_at(user))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
async fn write_custom_translations(
    directory: tauri::State<'_, UserDirectoryState>,
    text: String,
) -> Result<(), CommandError> {
    let user = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || write_custom_translations_at(user, &text))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkinImageResponse {
    content_type: &'static str,
    bytes: Vec<u8>,
}

fn read_skin_image_at(
    root: PathBuf,
    id: &str,
    relative: &str,
) -> Result<SkinImageResponse, CommandError> {
    let resource = msime_client_core::skin::catalog::read_resource(root, id, relative)
        .map_err(|_| CommandError { code: "storage" })?;
    if !resource.content_type.starts_with("image/") {
        return Err(CommandError {
            code: "invalid_resource",
        });
    }
    Ok(SkinImageResponse {
        content_type: resource.content_type,
        bytes: resource.bytes,
    })
}

#[tauri::command]
async fn read_skin_image(
    directory: tauri::State<'_, SkinDirectoryState>,
    id: String,
    relative: String,
) -> Result<SkinImageResponse, CommandError> {
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || read_skin_image_at(root, &id, &relative))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SkinFontResponse {
    content_type: &'static str,
    bytes: Vec<u8>,
}

fn read_skin_font_at(
    root: PathBuf,
    id: &str,
    relative: &str,
) -> Result<SkinFontResponse, CommandError> {
    let resource = msime_client_core::skin::catalog::read_resource(root, id, relative)
        .map_err(|_| CommandError { code: "storage" })?;
    if !resource.content_type.starts_with("font/") {
        return Err(CommandError {
            code: "invalid_resource",
        });
    }
    Ok(SkinFontResponse {
        content_type: resource.content_type,
        bytes: resource.bytes,
    })
}

#[tauri::command]
async fn read_skin_font(
    directory: tauri::State<'_, SkinDirectoryState>,
    id: String,
    relative: String,
) -> Result<SkinFontResponse, CommandError> {
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || read_skin_font_at(root, &id, &relative))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
async fn open_skin_directory(
    #[cfg_attr(not(target_os = "ios"), allow(unused_variables))] app: tauri::AppHandle,
    directory: tauri::State<'_, SkinDirectoryState>,
) -> Result<(), CommandError> {
    let root = directory.0.clone();
    // The iOS skin folder is in the App Group container, which Files cannot show, so the button imports a folder the user picks instead; `skin_directory_import` tells the page to say so.
    #[cfg(target_os = "ios")]
    return import_picked_skin(&app, root).await;
    #[cfg(not(target_os = "ios"))]
    open_directory(root).await
}

#[cfg(target_os = "ios")]
async fn import_picked_skin(app: &tauri::AppHandle, root: PathBuf) -> Result<(), CommandError> {
    let platform = app
        .try_state::<MobilePlatform<tauri::Wry>>()
        .ok_or(CommandError {
            code: "unavailable",
        })?
        .inner()
        .clone();
    let Some(source) = platform
        .pick_skin_folder()
        .await
        .map_err(|_| CommandError {
            code: "skin_import",
        })?
    else {
        return Ok(());
    };
    let copied = tauri::async_runtime::spawn_blocking(move || {
        msime_client_core::skin::folder_import::import(&source, &root)
    })
    .await;
    let _ = platform.end_skin_folder_access();
    copied
        .map_err(|_| CommandError { code: "storage" })?
        .map(|_| ())
        .map_err(|code| CommandError { code })
}

#[derive(serde::Serialize)]
struct SkinCatalogResponse {
    directory: String,
    #[serde(flatten)]
    catalog: msime_client_core::skin::catalog::SkinCatalog,
}

fn read_skin_catalog(root: PathBuf) -> SkinCatalogResponse {
    SkinCatalogResponse {
        directory: root.to_string_lossy().into_owned(),
        catalog: msime_client_core::skin::catalog::scan(root),
    }
}

#[tauri::command]
async fn scan_skin_catalog(
    directory: tauri::State<'_, SkinDirectoryState>,
    runtime: tauri::State<'_, RuntimeOptionsState>,
) -> Result<SkinCatalogResponse, CommandError> {
    // The host chooses the root; the webview cannot request arbitrary folders.
    let root = directory.0.clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rescan_skin_catalog(root, &runtime))
        .await
        .map_err(|_| CommandError { code: "storage" })
}

fn rescan_skin_catalog(root: PathBuf, runtime: &RuntimeOptionsState) -> SkinCatalogResponse {
    let response = read_skin_catalog(root);
    // A rescan is how a skin the user copied in or removed reaches the page, so it is also when the input method has to hear of it; otherwise its menu keeps the old list until the next save.
    // Publishing is best effort: a damaged or unwritable runtime options file must not cost the page the list it just scanned.
    #[cfg(target_os = "linux")]
    let _ = publish_candidate_skin_catalog(runtime, &response.catalog);
    #[cfg(not(target_os = "linux"))]
    let _ = runtime;
    response
}

/// Discover the helper-code tables shipped beside the Engine's verified resources. The WebView
/// receives metadata only; the resource path stays in the host options state and never comes from
/// page input.
fn list_helpcode_schemas_at(
    options: &DictionaryHostOptions,
) -> Result<Vec<msime_client_core::helpcode::CustomHelpcodeSchema>, CommandError> {
    let document = options.snapshot()?;
    let resources = document
        .get("resources")
        .and_then(Value::as_str)
        .filter(|value| std::path::Path::new(value).is_absolute())
        .ok_or(CommandError { code: "storage" })?;
    msime_host_api::list_custom_helpcode_schemas(resources)
        .map_err(|_| CommandError { code: "storage" })
}

#[tauri::command]
async fn list_helpcode_schemas(
    options: tauri::State<'_, DictionaryHostOptions>,
) -> Result<Vec<msime_client_core::helpcode::CustomHelpcodeSchema>, CommandError> {
    let options = options.inner().clone();
    tauri::async_runtime::spawn_blocking(move || list_helpcode_schemas_at(&options))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

/// `SettingsClient.resolveTheme`: the page's global theme draft, resolved the way `msime_client_resolve_theme` resolves it for native hosts.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveThemeRequest {
    global_theme: msime_client_core::skin::theme::GlobalTheme,
    #[serde(default)]
    custom_theme: msime_client_core::preferences::CustomTheme,
    dark: bool,
    layout: msime_client_core::preferences::CandidateLayout,
}

/// Resolve `request` against the installed packages under `root`. A custom theme whose package is missing or unreadable resolves without it, as it does on every native host.
fn resolve_theme_at(
    root: &std::path::Path,
    request: ResolveThemeRequest,
) -> Result<msime_client_core::skin::theme::ResolvedTheme, CommandError> {
    use msime_client_core::skin::theme::{self, GlobalTheme, ThemePackage};
    request.custom_theme.validate()?;
    let global_theme = request.global_theme;
    let package = request
        .custom_theme
        .candidate_skin
        .as_deref()
        .filter(|_| global_theme == GlobalTheme::Custom)
        .and_then(|id| msime_client_core::skin::catalog::load_package(root, id).ok())
        .map(|summary| ThemePackage::from(&summary));
    Ok(theme::resolve(
        global_theme,
        &request.custom_theme,
        request.dark,
        request.layout,
        package.as_ref(),
    ))
}

#[tauri::command]
async fn resolve_theme(
    directory: tauri::State<'_, SkinDirectoryState>,
    request: ResolveThemeRequest,
) -> Result<msime_client_core::skin::theme::ResolvedTheme, CommandError> {
    // The host chooses the root; the webview names a package only by id.
    let root = directory.0.clone();
    tauri::async_runtime::spawn_blocking(move || resolve_theme_at(&root, request))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[derive(Clone)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
struct RuntimeOptionsState {
    path: Option<PathBuf>,
    document: Arc<Mutex<Value>>,
    /// The external skins directory whose catalog the Linux candidate hosts read from `candidate_skin_catalog` in this document; `None` publishes no catalog.
    skins: Option<PathBuf>,
}

#[cfg(unix)]
impl RuntimeOptionsState {
    fn snapshot(&self) -> Result<Value, std::io::Error> {
        let document = self
            .document
            .lock()
            .map_err(|_| std::io::Error::other("runtime options lock poisoned"))?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let mut document = document;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        if let Some(path) = self.path.as_ref() {
            *document = read_runtime_options(path)?;
        }
        Ok(document.clone())
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "android",
    target_os = "ios",
    test
))]
const RUNTIME_OPTIONS_READ_LIMIT: u64 = 2 << 20;

#[cfg(any(
    target_os = "linux",
    target_os = "windows",
    target_os = "android",
    target_os = "ios",
    test
))]
fn read_runtime_options_bytes(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    if let Some(parent) = path.parent() {
        crate::shared::atomic_file::check_directory_ancestors(parent)?;
    }
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "runtime options is not a regular file",
        ));
    }
    let file = fs::File::open(path)?;
    match crate::shared::bounded_body::read_bounded(file, RUNTIME_OPTIONS_READ_LIMIT as usize) {
        Ok(bytes) => Ok(bytes),
        Err(crate::shared::bounded_body::BoundedReadError::TooLarge) => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "runtime options exceed size limit",
        )),
        Err(crate::shared::bounded_body::BoundedReadError::Read(error)) => Err(error),
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn read_runtime_options(path: &Path) -> Result<Value, std::io::Error> {
    let document: Value = serde_json::from_slice(&read_runtime_options_bytes(path)?)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    if !document.is_object() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "runtime options must be an object",
        ));
    }
    Ok(document)
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct PanelInputState(std::sync::Mutex<HashMap<String, PanelInputTarget>>);

#[cfg(not(target_os = "linux"))]
#[derive(Default)]
struct PanelInputState(std::sync::Mutex<Option<PanelInputTarget>>);

/// A panel input target that is valid for one open of a panel only, which the cloud clipboard panel keeps beside [`PanelInputState`] (see `panel_input`). Each open starts empty, a capture is only accepted for the open it was taken for, and closing the panel drops it.
#[cfg(any(target_os = "linux", target_os = "windows", test))]
#[derive(Debug)]
pub(crate) struct FreshInputTarget<T> {
    open: u64,
    target: Option<T>,
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
impl<T> Default for FreshInputTarget<T> {
    fn default() -> Self {
        Self {
            open: 0,
            target: None,
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", test))]
impl<T: Clone> FreshInputTarget<T> {
    /// Starts a new open, forgetting the previous open's target, and returns the token its capture must be recorded with.
    pub(crate) fn begin_open(&mut self) -> u64 {
        self.open = self.open.wrapping_add(1);
        self.target = None;
        self.open
    }

    /// Records what the capture for `open` found. A capture taken for an open that has since been superseded or closed is discarded.
    pub(crate) fn record(&mut self, open: u64, target: Option<T>) {
        if open == self.open {
            self.target = target;
        }
    }

    pub(crate) fn close(&mut self) {
        self.open = self.open.wrapping_add(1);
        self.target = None;
    }

    pub(crate) fn target(&self) -> Option<T> {
        self.target.clone()
    }
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
#[derive(Clone, Default)]
struct DesktopSettingsLinger {
    generation: Arc<AtomicU64>,
    quitting: Arc<AtomicBool>,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
enum PanelInputTarget {
    X11(String),
    Sway(u64),
    Ydotool,
    Wayland,
    // No external tool reached the editor, but the MSIME input method serves the panel socket and types into whatever context it has focused.
    InputMethod,
}

// The window that owned the caret before the panel appeared. Panels never take
// focus, but a click still has to reach that window and not the panel itself.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
struct PanelInputTarget(msime_host_windows::InputTarget);

#[cfg(target_os = "macos")]
#[derive(Clone, Debug)]
struct PanelInputTarget(msime_host_macos::LaunchTarget);

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "windows"),
    not(target_os = "macos")
))]
#[derive(Clone, Debug)]
struct PanelInputTarget;

// Debug so a test that unwraps a command result says which code came back rather than only that one did.
#[derive(Debug, serde::Serialize)]
struct CommandError {
    code: &'static str,
}

impl From<PreferencesError> for CommandError {
    fn from(value: PreferencesError) -> Self {
        Self {
            code: match value {
                PreferencesError::Conflict => "conflict",
                PreferencesError::InvalidPageSize => "invalid",
                PreferencesError::InvalidFrequency => "frequency_invalid",
                PreferencesError::InvalidMixedInput => "mixed_input_invalid",
                PreferencesError::InvalidFloatingToolbar => "floating_toolbar_invalid",
                PreferencesError::ConflictingKeyBindings => "key_conflict",
                PreferencesError::InvalidPlugins => "plugins_invalid",
                PreferencesError::UnsupportedFormat | PreferencesError::Json(_) => "format",
                _ => "storage",
            },
        }
    }
}

fn custom_skin_library_error(value: CustomSkinLibraryError) -> CommandError {
    CommandError {
        code: match value {
            CustomSkinLibraryError::Full => "custom_skin_full",
            CustomSkinLibraryError::InvalidName => "custom_skin_invalid_name",
            CustomSkinLibraryError::DuplicateName => "custom_skin_duplicate_name",
            CustomSkinLibraryError::NotFound => "custom_skin_not_found",
            CustomSkinLibraryError::Json(_) | CustomSkinLibraryError::Invalid => {
                "custom_skin_format"
            }
            CustomSkinLibraryError::Io(_) => "storage",
        },
    }
}

/// The settings document the restore-defaults button would write.
///
/// It reads the current document and hands back what a restore would produce, without writing
/// anything: the page puts it in the draft and the user saves it like any other edit, so a misclick
/// costs nothing and the change is visible before it lands. `restored_to_defaults` decides what
/// survives -- the credentials and what addresses the same service -- so that rule lives beside the
/// struct rather than in the page.
#[tauri::command]
async fn restored_default_preferences(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<Preferences, CommandError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store
            .load()
            .map(|snapshot| snapshot.preferences.restored_to_defaults())
            .map_err(CommandError::from)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// What the settings page's 修复配置文件 produced.
#[cfg(not(target_os = "ios"))]
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct PreferencesRecovery {
    snapshot: PreferencesSnapshot,
    /// Absent when the document already loaded and nothing was written.
    backup_path: Option<String>,
    salvaged: bool,
}

/// Repair a preferences document that `load_preferences` rejects, as the Windows source repairs a config.toml that does not parse.
///
/// `PreferencesStore::recover` backs the damaged file up beside it before anything is written and keeps every setting and service key the current schema still accepts. The runtime options are republished like a save, so hosts that read them see the repaired values, and a repaired document that leaves clipboard history off clears the stored history as a save would. iOS is left out: its keyboard keeps a native mirror of the AI settings that only the save path updates, and a repair there would leave the two describing different services.
#[cfg(not(target_os = "ios"))]
#[tauri::command]
async fn recover_preferences(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    runtime: tauri::State<'_, RuntimeOptionsState>,
) -> Result<PreferencesRecovery, CommandError> {
    let store = store.inner().clone();
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let recovery = match store.recover().map_err(CommandError::from)? {
            msime_client_core::preferences::RecoveryOutcome::NotNeeded(snapshot) => {
                PreferencesRecovery {
                    snapshot,
                    backup_path: None,
                    salvaged: false,
                }
            }
            msime_client_core::preferences::RecoveryOutcome::Recovered {
                snapshot,
                backup_path,
                salvaged,
            } => PreferencesRecovery {
                snapshot,
                backup_path: Some(backup_path.to_string_lossy().into_owned()),
                salvaged,
            },
        };
        let synced = sync_runtime_options(&runtime, &recovery.snapshot.preferences);
        if clipboard_history_uses_preference(host_platform())
            && !recovery.snapshot.preferences.clipboard_history
        {
            store
                .clear_disabled_clipboard_history()
                .map_err(CommandError::from)?;
        }
        synced.map_err(CommandError::from)?;
        Ok(recovery)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Open the folder holding the preferences document, where a repair leaves its backup. The page passes no path; the host opens its own store's directory.
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
#[tauri::command]
async fn open_preferences_directory(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<(), CommandError> {
    let root = store.directory().to_path_buf();
    open_directory(root).await
}

#[tauri::command]
async fn load_preferences(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<PreferencesSnapshot, CommandError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.load().map_err(CommandError::from))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

async fn save_preferences_impl(
    store: Arc<PreferencesStore>,
    runtime: RuntimeOptionsState,
    expected_revision: u64,
    preferences: Preferences,
    #[cfg(target_os = "ios")] platform: MobilePlatform<tauri::Wry>,
) -> Result<PreferencesSnapshot, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(any(target_os = "ios", target_os = "linux"))]
        let previous = store.load().map_err(CommandError::from)?;
        let snapshot = store
            .save(expected_revision, preferences)
            .map_err(CommandError::from)?;
        #[cfg(target_os = "ios")]
        if let Err(_) = platform.save_keyboard_ai(&ios_keyboard_ai_preferences(
            &snapshot.preferences.ai_assistant,
        )) {
            // Do not leave the canonical Rust document and the keyboard's
            // native mirror describing different AI services. The revision
            // returned by save() is the only revision that can safely roll
            // back the write; a concurrent writer is reported as storage
            // failure rather than overwritten.
            let _ = store.save(snapshot.revision, previous.preferences);
            return Err(CommandError { code: "ai_storage" });
        }
        // Published before the clipboard history is cleared, so a save refused here has not already deleted the history its restored preferences keep enabled.
        let synced = sync_runtime_options(&runtime, &snapshot.preferences);
        // A document the Linux hosts could not read is refused, and so is the save that produced it: the store goes back to the preferences the hosts still run with, so the page's error is the whole outcome rather than a store and a runtime options file that disagree. As with the iOS rollback above, a concurrent writer wins over the rollback.
        #[cfg(target_os = "linux")]
        if matches!(synced, Err(RuntimeOptionsError::TooLarge)) {
            let _ = store.save(snapshot.revision, previous.preferences);
        }
        // Cleared whatever the sync's outcome: any other sync failure leaves history disabled in the store, and its captured history must not outlive that. The clear re-reads the store, so after the rollback above restored history it keeps the file.
        if clipboard_history_uses_preference(host_platform())
            && !snapshot.preferences.clipboard_history
        {
            store
                .clear_disabled_clipboard_history()
                .map_err(CommandError::from)?;
        }
        synced.map_err(CommandError::from)?;
        Ok(snapshot)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn save_preferences(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    runtime: tauri::State<'_, RuntimeOptionsState>,
    platform: tauri::State<'_, MobilePlatform<tauri::Wry>>,
    expected_revision: u64,
    preferences: Preferences,
) -> Result<PreferencesSnapshot, CommandError> {
    save_preferences_impl(
        store.inner().clone(),
        runtime.inner().clone(),
        expected_revision,
        preferences,
        platform.inner().clone(),
    )
    .await
}

#[cfg(not(target_os = "ios"))]
#[tauri::command]
async fn save_preferences(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    runtime: tauri::State<'_, RuntimeOptionsState>,
    expected_revision: u64,
    preferences: Preferences,
) -> Result<PreferencesSnapshot, CommandError> {
    save_preferences_impl(
        store.inner().clone(),
        runtime.inner().clone(),
        expected_revision,
        preferences,
    )
    .await
}

#[cfg(any(target_os = "ios", test))]
fn ios_keyboard_ai_preferences(
    preferences: &msime_client_core::preferences::AiAssistantPreferences,
) -> IosKeyboardAiPreferences {
    let provider = match preferences.provider.as_str() {
        "everyapi" => "everyAPI",
        "openai" => "openAI",
        "anthropic" => "anthropic",
        "gemini" => "gemini",
        "deepseek" => "deepSeek",
        "qwen" => "qwen",
        "kimi" => "kimi",
        "zhipu" => "zhipu",
        "siliconflow" => "siliconFlow",
        "openrouter" => "openRouter",
        _ => "custom",
    }
    .to_owned();
    let endpoint = preferences.endpoint.trim();
    let token = reqwest::Url::parse(endpoint)
        .ok()
        .and_then(|url| {
            let explicit_authority = endpoint.split_once("://").is_some_and(|(_, authority)| {
                authority
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| *byte != b'/')
            });
            if url.scheme() != "https"
                || !explicit_authority
                || url.host_str().is_none_or(str::is_empty)
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return None;
            }
            let origin = format!(
                "https://{}:{}",
                url.host_str()?.to_ascii_lowercase(),
                url.port().unwrap_or(443)
            );
            preferences
                .tokens
                .get(&preferences.provider)
                .or_else(|| preferences.tokens.get(&origin))
                .or_else(|| (!preferences.token.is_empty()).then_some(&preferences.token))
                .cloned()
        })
        .unwrap_or_default();
    let enabled = preferences.enabled
        && !preferences.endpoint.trim().is_empty()
        && !preferences.model.trim().is_empty()
        && !preferences.prompt.trim().is_empty()
        && !token.trim().is_empty();
    IosKeyboardAiPreferences {
        // Rust preferences may intentionally be enabled before the user has
        // supplied a credential. Keep that draft in the canonical store, but
        // clear the native mirror until the keyboard can actually authenticate.
        enabled,
        provider,
        endpoint: preferences.endpoint.clone(),
        model: preferences.model.clone(),
        prompt: if preferences.prompt.trim().is_empty() {
            "请润色以下文字，保持原意，只返回修改后的文字。".to_owned()
        } else {
            preferences.prompt.clone()
        },
        token,
    }
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn configured_provider_socket(
    document: &Value,
    key: &str,
    environment: &str,
    discovered: &str,
) -> Option<PathBuf> {
    document
        .get(key)
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| {
            std::env::var_os(environment)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
        })
        .or_else(|| discover_session_provider(discovered))
}

#[cfg(any(target_os = "linux", all(test, unix)))]
fn credential_provider_socket(document: &Value, service: &str) -> Option<PathBuf> {
    if service.starts_with("voice.") {
        return voice::resolve_voice_provider_socket(document);
    }
    if service.starts_with("translation.") {
        return configured_provider_socket(
            document,
            "translation_provider_socket",
            "MSIME_TRANSLATION_PROVIDER_SOCKET",
            "translation.sock",
        )
        .or_else(|| {
            configured_provider_socket(
                document,
                "online_provider_socket",
                "MSIME_ONLINE_PROVIDER_SOCKET",
                "online.sock",
            )
        });
    }
    (service == "ai.assistant").then(|| {
        configured_provider_socket(
            document,
            "online_provider_socket",
            "MSIME_ONLINE_PROVIDER_SOCKET",
            "online.sock",
        )
    })?
}

/// The AI service's model catalogue, by way of the provider that holds its
/// credential.
///
/// The hosts that keep the token in the shell fetch this over HTTP themselves
/// (`ai::ai_models`). On Linux the token is in the provider's owner-only file by
/// design, so the shell has nothing to authenticate with and asks the provider
/// instead. Same command name, so the settings page does not need to know which
/// host it is on.
#[cfg(target_os = "linux")]
#[tauri::command]
async fn ai_models(
    runtime: tauri::State<'_, RuntimeOptionsState>,
    provider: String,
    endpoint: String,
) -> Result<Vec<String>, CommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let document = runtime.snapshot().map_err(|_| CommandError {
            code: "ai_models_unavailable",
        })?;
        let path = credential_provider_socket(&document, "ai.assistant").ok_or(CommandError {
            code: "ai_models_unavailable",
        })?;
        UnixSocketProvider::new(path)
            .ai_models(&provider, &endpoint)
            .ok_or(CommandError {
                code: "ai_models_unavailable",
            })
    })
    .await
    .map_err(|_| CommandError {
        code: "ai_models_unavailable",
    })?
}

/// One polish request through the user's AI service, for the settings page's test
/// box. Routed through the provider for the same reason as the model listing.
#[cfg(target_os = "linux")]
#[tauri::command]
async fn ai_test(
    runtime: tauri::State<'_, RuntimeOptionsState>,
    provider: String,
    endpoint: String,
    model: String,
    prompt: String,
    text: String,
) -> Result<String, CommandError> {
    let runtime = runtime.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let document = runtime.snapshot().map_err(|_| CommandError {
            code: "ai_test_unavailable",
        })?;
        let path = credential_provider_socket(&document, "ai.assistant").ok_or(CommandError {
            code: "ai_test_unavailable",
        })?;
        UnixSocketProvider::new(path)
            .ai_test(&provider, &endpoint, &model, &prompt, &text)
            .ok_or(CommandError {
                code: "ai_test_unavailable",
            })
    })
    .await
    .map_err(|_| CommandError {
        code: "ai_test_unavailable",
    })?
}

#[tauri::command]
async fn test_api_credential(
    runtime: tauri::State<'_, RuntimeOptionsState>,
    service: String,
    config: Value,
) -> Result<msime_input_runtime::CredentialTestResult, CommandError> {
    #[cfg(target_os = "linux")]
    {
        let runtime = runtime.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let document = runtime.snapshot().map_err(|_| CommandError {
                code: "unavailable",
            })?;
            let path = credential_provider_socket(&document, &service).ok_or(CommandError {
                code: "unavailable",
            })?;
            UnixSocketProvider::new(path)
                .test_credential(&service, &config)
                .ok_or(CommandError {
                    code: "unavailable",
                })
        })
        .await
        .map_err(|_| CommandError {
            code: "unavailable",
        })?;
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    {
        let _ = runtime;
        let result = tauri::async_runtime::spawn_blocking(move || {
            if service == "voice.asr" {
                if config.get("provider").and_then(Value::as_str) == Some("doubao") {
                    return msime_client_core::credential::doubao::test(
                        &config,
                        &msime_client_core::credential::doubao::WebSocketTransport,
                    );
                }
                return msime_client_core::credential::asr::test(
                    &config,
                    &msime_client_core::credential::asr::HttpTransport,
                );
            }
            if service.starts_with("translation.") {
                let milliseconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|duration| duration.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(0);
                return msime_client_core::credential::translation::test(
                    &service,
                    &config,
                    milliseconds,
                    &msime_client_core::credential::translation::HttpTransport,
                );
            }
            msime_client_core::credential::probe::test_chat(
                &service,
                &config,
                &msime_client_core::credential::probe::HttpsProbeTransport,
            )
        })
        .await
        .map_err(|_| CommandError {
            code: "unavailable",
        })?;
        Ok(msime_input_runtime::CredentialTestResult {
            ok: result.ok,
            message: result.message,
        })
    }
    #[cfg(target_os = "ios")]
    {
        let _ = runtime;
        let result = tauri::async_runtime::spawn_blocking(move || {
            let result = msime_client_core::credential::probe::test_chat(
                &service,
                &config,
                &msime_client_core::credential::probe::HttpsProbeTransport,
            );
            msime_input_runtime::CredentialTestResult {
                ok: result.ok,
                message: result.message,
            }
        })
        .await
        .map_err(|_| CommandError {
            code: "unavailable",
        })?;
        Ok(result)
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos",
        target_os = "ios"
    )))]
    {
        let _ = (runtime, service, config);
        Err(CommandError {
            code: "unavailable",
        })
    }
}

#[tauri::command]
async fn load_custom_skin_library(
    store: tauri::State<'_, CustomSkinLibraryStore>,
) -> Result<Vec<SavedTouchKeyboardSkin>, CommandError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || store.load().map_err(custom_skin_library_error))
        .await
        .map_err(|_| CommandError { code: "storage" })?
}

#[tauri::command]
async fn mutate_custom_skin_library(
    store: tauri::State<'_, CustomSkinLibraryStore>,
    action: CustomSkinLibraryAction,
) -> Result<Vec<SavedTouchKeyboardSkin>, CommandError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store.mutate(action).map_err(custom_skin_library_error)
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Why the runtime options the native hosts read were not rewritten.
#[derive(Debug)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
enum RuntimeOptionsError {
    /// The error itself is only ever printed, by `Debug` in a failing test; the page is told "storage".
    Io(#[allow(dead_code)] std::io::Error),
    /// The document would be longer than the Linux hosts read, so it was not written and the previous file stays in place.
    TooLarge,
}

impl From<std::io::Error> for RuntimeOptionsError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<RuntimeOptionsError> for CommandError {
    fn from(value: RuntimeOptionsError) -> Self {
        Self {
            code: match value {
                RuntimeOptionsError::Io(_) => "storage",
                RuntimeOptionsError::TooLarge => "runtime_options_too_large",
            },
        }
    }
}

fn sync_runtime_options(
    runtime: &RuntimeOptionsState,
    preferences: &Preferences,
) -> Result<(), RuntimeOptionsError> {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        let Some(path) = runtime.path.as_ref() else {
            return Ok(());
        };
        // The Linux hosts draw an installed skin from the catalog in this document, and no other writer keeps it current: publish it with every save so a user who picks one sees its colours without ever rescanning.
        #[cfg(target_os = "linux")]
        let catalog = runtime
            .skins
            .as_deref()
            .map(|root| (root, msime_client_core::skin::catalog::scan(root)));
        let mut document = runtime
            .document
            .lock()
            .map_err(|_| std::io::Error::other("runtime options lock poisoned"))?;
        // Another settings process or the host may have updated endpoints and
        // resource paths since this panel started. Preserve that document.
        let mut current = read_runtime_options(path)?;
        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
        let mut host_preferences = serde_json::to_value(preferences)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        // The IBus and Fcitx5 hosts never draw the screen keyboard, and the settings app's own keyboard reads the preference store, so the base64 photo of the custom theme's keyboard design stays out of their copy: a few hundred KiB of it would put the whole document past what they read.
        #[cfg(target_os = "linux")]
        if let Some(design) = host_preferences
            .pointer_mut("/custom_theme/keyboard")
            .and_then(Value::as_object_mut)
        {
            design.remove("photo");
        }
        current["preferences"] = host_preferences;
        #[cfg(target_os = "linux")]
        let bytes = match &catalog {
            Some((root, catalog)) => {
                runtime_options_with_skin_catalog(&mut current, root, catalog)?
            }
            None => linux_runtime_options_bytes(&current)?,
        };
        #[cfg(target_os = "android")]
        let bytes = serde_json::to_vec_pretty(&current)
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        shared::atomic_file::write(path, &bytes)?;
        *document = current;
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        let _ = (runtime, preferences);
    }
    Ok(())
}

/// The most runtime-options.json may hold. The IBus launcher, the Fcitx5 addon and `msime_host_api::refresh_host_options` all read at most 16 KiB and keep their previous configuration (IBus) or fail (Fcitx5, the upgrade refresh) on anything longer.
#[cfg(target_os = "linux")]
const LINUX_RUNTIME_OPTIONS_LIMIT: usize = 16384;

/// How large the skin catalog may let runtime-options.json grow: the last 1 KiB of `LINUX_RUNTIME_OPTIONS_LIMIT` is left for the upgrade refresh, which rewrites the resource and dictionary paths and appends a newline without knowing about the catalog.
#[cfg(target_os = "linux")]
const LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET: usize = LINUX_RUNTIME_OPTIONS_LIMIT - 1024;

/// Serialize `document` as the Linux hosts will read it, refusing one longer than they read so that a save never replaces a working file with one that stops both hosts.
#[cfg(target_os = "linux")]
fn linux_runtime_options_bytes(document: &Value) -> Result<Vec<u8>, RuntimeOptionsError> {
    let bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    if bytes.len() > LINUX_RUNTIME_OPTIONS_LIMIT {
        return Err(RuntimeOptionsError::TooLarge);
    }
    Ok(bytes)
}

/// Serialize `document` with the installed skins, scanned from `root`, as `candidate_skin_catalog`, dropping packages from the end until the document fits within `LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET`.
///
/// The currently selected skin is dropped last, since its colours are the ones on screen. When not even an empty catalog fits, the key is left out, so the catalog never becomes the reason a document the hosts could read no longer loads; a document too large for the hosts even without it is refused.
#[cfg(target_os = "linux")]
fn runtime_options_with_skin_catalog(
    document: &mut Value,
    root: &std::path::Path,
    catalog: &msime_client_core::skin::catalog::SkinCatalog,
) -> Result<Vec<u8>, RuntimeOptionsError> {
    let serialize = |document: &Value| {
        serde_json::to_vec_pretty(document)
            .map_err(|error| std::io::Error::other(error.to_string()))
    };
    let selected = document["preferences"]["custom_theme"]["candidate_skin"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let mut published =
        msime_client_core::skin::catalog::host_candidate_catalog(catalog, root, &selected);
    loop {
        document["candidate_skin_catalog"] = published.clone();
        let bytes = serialize(document)?;
        if bytes.len() <= LINUX_RUNTIME_OPTIONS_CATALOG_BUDGET {
            return Ok(bytes);
        }
        let Some(packages) = published["packages"].as_array_mut() else {
            break;
        };
        if packages.is_empty() {
            break;
        }
        let dropped = packages
            .iter()
            .rposition(|package| package["id"] != selected.as_str())
            .unwrap_or(packages.len() - 1);
        packages.remove(dropped);
    }
    if let Some(object) = document.as_object_mut() {
        object.remove("candidate_skin_catalog");
    }
    linux_runtime_options_bytes(document)
}

/// Write a freshly scanned catalog into the runtime options the Linux hosts read, leaving every other key as it is on disk. Before setup there is no document to publish into, which is not an error.
#[cfg(target_os = "linux")]
fn publish_candidate_skin_catalog(
    runtime: &RuntimeOptionsState,
    catalog: &msime_client_core::skin::catalog::SkinCatalog,
) -> Result<(), RuntimeOptionsError> {
    let (Some(path), Some(root)) = (runtime.path.as_ref(), runtime.skins.as_ref()) else {
        return Ok(());
    };
    let mut document = runtime
        .document
        .lock()
        .map_err(|_| std::io::Error::other("runtime options lock poisoned"))?;
    let mut current = match read_runtime_options(path) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let unchanged = current.get("candidate_skin_catalog").cloned();
    let bytes = runtime_options_with_skin_catalog(&mut current, root, catalog)?;
    // A rescan that finds what was already published leaves the file alone, so the hosts watching it do not reload for nothing.
    if current.get("candidate_skin_catalog") != unchanged.as_ref() {
        shared::atomic_file::write(path, &bytes)?;
    }
    *document = current;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn start_desktop_preferences_monitor(
    app: &tauri::AppHandle,
    store: std::sync::Arc<PreferencesStore>,
    history: Arc<Mutex<ClipboardHistoryStore>>,
    #[cfg(target_os = "linux")] history_path: PathBuf,
) {
    let app = app.clone();
    let _ = std::thread::Builder::new()
        .name("msime-preferences-monitor".to_owned())
        .spawn(move || {
            let mut monitor = desktop_preferences_monitor::Monitor::new(&store);
            #[cfg(target_os = "macos")]
            let mut last_change_count = msime_host_macos::clipboard_snapshot(
                store
                    .load()
                    .map(|snapshot| snapshot.preferences.clipboard_history)
                    .unwrap_or(false),
            )
            .map(|snapshot| snapshot.change_count);
            loop {
                #[cfg(target_os = "windows")]
                {
                    // The native Server signals after a successful bounded
                    // store update. Keep the same timeout as the old poll so
                    // preferences and writes from other tools remain visible
                    // even when the event is unavailable.
                    let _ = msime_host_windows::wait_for_clipboard_history_change(
                        std::time::Duration::from_millis(750),
                    );
                }
                #[cfg(target_os = "linux")]
                {
                    let _ = desktop_preferences_monitor::wait_for_filesystem_change(
                        &history_path,
                        std::time::Duration::from_millis(750),
                    );
                }
                #[cfg(target_os = "macos")]
                {
                    std::thread::sleep(std::time::Duration::from_millis(750));
                    let enabled = store
                        .load()
                        .map(|snapshot| snapshot.preferences.clipboard_history)
                        .unwrap_or(false);
                    if let Some(snapshot) = msime_host_macos::clipboard_snapshot(enabled) {
                        desktop_preferences_monitor::record_macos_clipboard_change(
                            snapshot,
                            &mut last_change_count,
                            enabled,
                            &store,
                            &history,
                        );
                    }
                }
                #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
                std::thread::sleep(std::time::Duration::from_millis(750));
                monitor.poll(&app, &store, &history);
            }
        });
}

/// Keep the host's reason instead of flattening every failure to "storage".
///
/// The host distinguishes three things the user can actually act on - the
/// dictionary is locked by another process, the edit itself was refused, and
/// the store could not be opened - and the page used to print one identical
/// sentence for all of them.
fn dictionary_error_code(reason: &str) -> &'static str {
    match reason {
        "dictionary maintenance busy" => "dictionary_busy",
        // A request over the host's 64 KiB, which the batched desktop import only gives for a file over its own bound or a single line too long for any request. The shared parser's own "dictionary import is too large" is deliberately not mapped here: only Android, which sends the whole file in one request, reaches it, and there the limit is 64 KiB rather than the 32 MB this code's message names.
        "invalid dictionary buffer" => "dictionary_too_large",
        "dictionary import rejected" => "dictionary_import_rejected",
        "dictionary read rejected" => "dictionary_read_rejected",
        "dictionary pinyin unavailable" => "dictionary_pinyin_unavailable",
        "dictionary access unavailable" => "dictionary_unavailable",
        "learned-data reset rejected" => "dictionary_reset_rejected",
        "bundled dictionary entry is read-only" => "dictionary_bundled_readonly",
        // The host appends which rule the entry broke. The page words a code refusal per dictionary kind, so a word or weight refusal must not share that code, or it would send the user to fix a code that is already valid.
        reason if reason == msime_host_api::INVALID_DICTIONARY_ENTRY => "dictionary_invalid_entry",
        reason => match reason
            .strip_prefix(msime_host_api::INVALID_DICTIONARY_ENTRY)
            .and_then(|rest| rest.strip_prefix(": "))
        {
            Some(rule) if invalid_entry_rule_is_about_word(rule) => "dictionary_invalid_word",
            Some(_) => "dictionary_invalid_entry",
            None => "storage",
        },
    }
}

/// Does a refusal reason name the word or the weight rather than the code? These are the host's `validate_entry` rules (`word ...`, `weight ...`) and the Engine's own sentences for the same rules in `validate_personal_dictionary_entry`.
fn invalid_entry_rule_is_about_word(rule: &str) -> bool {
    rule.starts_with("word ")
        || rule.starts_with("weight ")
        || rule == "Weight must be between 1 and 100000000"
        || rule == "The word contains an unsupported control character"
}

fn dictionary_action_requires_quiesce(action: &Value) -> bool {
    matches!(
        action.get("operation").and_then(Value::as_str),
        Some("edit" | "import" | "reset")
    )
}

#[cfg(any(target_os = "ios", test))]
fn ios_personal_dictionary_action(action: &Value) -> bool {
    matches!(
        action.get("operation").and_then(Value::as_str),
        Some(
            "list" | "edit" | "import" | "import_personal" | "export" | "retry" | "dismiss_failure"
        )
    )
}

#[cfg(target_os = "ios")]
fn ios_personal_dictionary_request(request: &Value) -> Result<Value, CommandError> {
    let bytes = serde_json::to_vec(
        request
            .get("action")
            .ok_or(CommandError { code: "storage" })?,
    )
    .map_err(|_| CommandError { code: "storage" })?;
    if bytes.len() > 1_200_000 {
        return Err(CommandError { code: "storage" });
    }
    let response = msime_ios_native_ffi::personal_dictionary_request(&bytes)
        .ok_or(CommandError { code: "storage" })?;
    let envelope: Value =
        serde_json::from_slice(&response).map_err(|_| CommandError { code: "storage" })?;
    if envelope.get("ok") == Some(&Value::Bool(true)) {
        return envelope
            .get("value")
            .cloned()
            .ok_or(CommandError { code: "storage" });
    }
    let code = match envelope.get("error").and_then(Value::as_str) {
        Some("dictionary_busy") => "dictionary_busy",
        Some("dictionary_conflict") => "dictionary_conflict",
        Some("dictionary_too_many") => "dictionary_too_many",
        Some("dictionary_unavailable") => "dictionary_unavailable",
        Some("dictionary_import_rejected") => "dictionary_import_rejected",
        _ => "storage",
    };
    Err(CommandError { code })
}

#[tauri::command]
async fn dictionary_request(
    state: tauri::State<'_, DictionaryHostOptions>,
    action: serde_json::Value,
) -> Result<serde_json::Value, CommandError> {
    let options = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let options = options.snapshot()?;
        let requires_quiesce = dictionary_action_requires_quiesce(&action);
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        let _ = requires_quiesce;
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let user_data = options["user_data"].as_str().map(str::to_owned);
        let request = serde_json::json!({ "options": options, "action": action });
        #[cfg(target_os = "ios")]
        if ios_personal_dictionary_action(&request["action"]) {
            return ios_personal_dictionary_request(&request);
        }
        #[cfg(target_os = "android")]
        {
            let bytes =
                serde_json::to_vec(&request).map_err(|_| CommandError { code: "storage" })?;
            return msime_host_api::personal_dictionary_request_json(&bytes).map_err(|reason| {
                CommandError {
                    code: dictionary_error_code(&reason),
                }
            });
        }
        #[cfg(not(target_os = "android"))]
        {
            // One request to the host. An import too large for one request is several, each through this, so whatever releases the input sessions below stays in force until the last of them.
            let host = |bytes: &[u8]| msime_host_api::dictionary_request_json(bytes);
            // The input hosts are asked to let go and the lock failure is retried until they have. On Linux and macOS that is a lease the hosts find on their timers, with the macOS input method also told at once over a distributed notification when the lease first goes up; on Windows the Server is asked over its pipe and answers once its sessions are gone. Either is renewed before each later request, so a large import that runs past the 30 second bound keeps the hosts released, and let go when `hosts` goes, after the last request, whatever the outcome.
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            let mut hosts = msime_client_core::dictionary::quiesce::QuiescedHosts::new(
                user_data.as_deref(),
                || {
                    #[cfg(target_os = "macos")]
                    msime_host_macos::quiesce_input_sessions();
                },
            );
            #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
            let send = |bytes: &[u8]| {
                if requires_quiesce {
                    hosts.run(|| host(bytes))
                } else {
                    host(bytes)
                }
            };
            #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
            let send = host;
            let result = dictionary_import::send_dictionary_action(
                &request["options"],
                &request["action"],
                send,
            );
            result.map_err(|reason| CommandError {
                code: dictionary_error_code(&reason),
            })
        }
    })
    .await
    .map_err(|_| CommandError { code: "storage" })?
}

/// Where a desktop cloud clipboard request is served, in order of precedence: a provider socket named in the host options or the environment, the session of the macOS input method that launched this panel, a provider socket discovered in the user's runtime directory, and otherwise the account this shell is signed in to.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[cfg_attr(not(unix), allow(dead_code))]
#[derive(Debug, PartialEq)]
enum CloudClipboardRoute<N> {
    Provider(PathBuf),
    Native(N),
    Account,
}

/// Picks the [`CloudClipboardRoute`] for one request. A configured socket wins outright, and one that is not an absolute path is an error rather than a reason to fall through: the user asked for that provider, so the account must not answer in its place. The native session and discovery are only consulted when nothing is configured.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[cfg_attr(not(unix), allow(dead_code))]
fn cloud_clipboard_route<N>(
    configured: Option<String>,
    native: impl FnOnce() -> Option<N>,
    discover: impl FnOnce() -> Option<PathBuf>,
) -> Result<CloudClipboardRoute<N>, CommandError> {
    if let Some(configured) = configured {
        let path = PathBuf::from(configured);
        return if path.is_absolute() {
            Ok(CloudClipboardRoute::Provider(path))
        } else {
            Err(CommandError {
                code: "unavailable",
            })
        };
    }
    if let Some(result) = native() {
        return Ok(CloudClipboardRoute::Native(result));
    }
    Ok(discover()
        .filter(|path| path.is_absolute())
        .map_or(CloudClipboardRoute::Account, CloudClipboardRoute::Provider))
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
async fn cloud_clipboard_request(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    options: tauri::State<'_, DictionaryHostOptions>,
    account: tauri::State<'_, desktop_account::AccountState>,
    action: Value,
) -> Result<Value, CommandError> {
    msime_host_api::cloud_clipboard::validate_request(&action)
        .map_err(|_| CommandError { code: "invalid" })?;
    // Windows has no provider socket or native session, so every request goes to the account.
    #[cfg(unix)]
    {
        let options = options.inner().clone();
        let label = window.label().to_owned();
        let provider_action = action.clone();
        let served = tauri::async_runtime::spawn_blocking(move || {
            let _ = (&app, &label);
            let document = options.snapshot()?;
            let configured = document
                .get("cloud_clipboard_provider_socket")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    std::env::var_os("MSIME_CLOUD_CLIPBOARD_PROVIDER_SOCKET")
                        .and_then(|value| value.into_string().ok())
                });
            #[cfg(target_os = "macos")]
            let native = || {
                app.state::<macos_cloud_clipboard::CloudState>()
                    .request(&label, &provider_action)
            };
            #[cfg(not(target_os = "macos"))]
            let native = || None::<Result<Value, CommandError>>;
            match cloud_clipboard_route(configured, native, || {
                discover_session_provider("cloud-clipboard.sock")
            })? {
                CloudClipboardRoute::Provider(path) => UnixSocketProvider::new(path)
                    .cloud_clipboard(provider_action)
                    .map(Some)
                    .ok_or(CommandError {
                        code: "unavailable",
                    }),
                CloudClipboardRoute::Native(result) => result.map(Some),
                CloudClipboardRoute::Account => Ok(None),
            }
        })
        .await
        .map_err(|_| CommandError {
            code: "unavailable",
        })??;
        if let Some(value) = served {
            return Ok(value);
        }
    }
    #[cfg(not(unix))]
    let _ = (app, window, options);
    platform::cloud_clipboard::cloud_clipboard_request(&account.session, action).await
}

#[cfg(any(target_os = "ios", target_os = "android"))]
#[tauri::command]
async fn cloud_clipboard_request(
    state: tauri::State<'_, platform::mobile::MobileAccountState>,
    action: Value,
) -> Result<Value, CommandError> {
    msime_host_api::cloud_clipboard::validate_request(&action).map_err(|_| CommandError {
        code: "invalid_cloud_clipboard",
    })?;
    platform::cloud_clipboard::cloud_clipboard_request(state.session(), action).await
}

#[tauri::command]
fn cloud_clipboard_can_send_text(app: tauri::AppHandle, window: tauri::WebviewWindow) -> bool {
    #[cfg(target_os = "macos")]
    return macos_panel_session::can_submit_clipboard(&app, window.label());
    // Only an editor captured for this open of the panel counts; without one the page copies instead of typing.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    return window.label() == CLOUD_CLIPBOARD_PANEL && cloud_clipboard_input_target(&app).is_some();
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, window);
        false
    }
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
#[tauri::command]
async fn cloud_dictionary_request(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    options: tauri::State<'_, DictionaryHostOptions>,
    action: Value,
) -> Result<Value, CommandError> {
    let _ = (&app, &window);
    let request =
        serde_json::from_value::<msime_host_api::cloud_dictionary::CloudDictionaryRequest>(
            action.clone(),
        )
        .map_err(|_| CommandError { code: "invalid" })?;
    msime_host_api::cloud_dictionary::validate_cloud_request(&request)
        .map_err(|_| CommandError { code: "invalid" })?;
    let options = options.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(unix)]
        {
            let document = options.snapshot()?;
            let configured = document
                .get("cloud_dictionary_provider_socket")
                .and_then(Value::as_str)
                .map(str::to_owned);
            let configured = configured.or_else(|| {
                std::env::var_os("MSIME_CLOUD_DICTIONARY_PROVIDER_SOCKET")
                    .and_then(|value| value.into_string().ok())
            });
            #[cfg(target_os = "macos")]
            if configured.is_none() {
                if let Some(result) = app
                    .state::<macos_cloud_dictionary::DictionaryState>()
                    .request(window.label(), &action)
                {
                    return result;
                }
            }
            let path = configured
                .map(PathBuf::from)
                .or_else(|| discover_session_provider("cloud-dictionary.sock"))
                .filter(|path| path.is_absolute())
                .ok_or(CommandError {
                    code: "unavailable",
                })?;
            UnixSocketProvider::new(path)
                .cloud_dictionary(action)
                .ok_or(CommandError {
                    code: "unavailable",
                })
        }
        #[cfg(not(unix))]
        {
            let _ = (options, action);
            Err(CommandError {
                code: "unavailable",
            })
        }
    })
    .await
    .map_err(|_| CommandError {
        code: "unavailable",
    })?
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn cloud_dictionary_request(
    state: tauri::State<'_, ios_account::AccountState>,
    action: Value,
) -> Result<Value, CommandError> {
    let request = parse_cloud_dictionary_request(&action)?;
    ios_account::cloud_dictionary_request(state, request).await
}

#[cfg(target_os = "android")]
#[tauri::command]
async fn cloud_dictionary_request(
    state: tauri::State<'_, android_account::AccountState>,
    action: Value,
) -> Result<Value, CommandError> {
    let request = parse_cloud_dictionary_request(&action)?;
    android_account::cloud_dictionary_request(state, request).await
}

#[derive(serde::Serialize)]
struct EmojiCatalogItem {
    text: String,
    keywords: String,
}

#[derive(serde::Serialize)]
struct EmojiCatalogGroup {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent: Option<String>,
    icon: String,
    items: Vec<EmojiCatalogItem>,
}

#[derive(serde::Serialize)]
struct EmojiCatalogResponse {
    emoji: Vec<EmojiCatalogGroup>,
    kaomoji: Vec<EmojiCatalogGroup>,
    symbols: Vec<EmojiCatalogGroup>,
    unavailable: Vec<&'static str>,
}

fn emoji_category_icon(title: &str) -> &'static str {
    [
        ("Smileys", "😀"),
        ("People", "🧑"),
        ("Animals", "🐾"),
        ("Food", "🍕"),
        ("Travel", "🚗"),
        ("Activities", "🎉"),
        ("Objects", "💡"),
        ("Symbols", "❤"),
        ("Flags", "🏳"),
    ]
    .into_iter()
    .find_map(|(name, icon)| title.contains(name).then_some(icon))
    .unwrap_or("☺")
}

// Reads the real catalog on every desktop host. The Windows build used to hit
// a stub that always failed, so the panel fell back to the compact built-in
// catalog - 97 emoji, 18 kaomoji, 48 symbols - behind a permanent "catalog
// failed to load" banner, and the symbol sub-tabs collapsed to one flat tab
// because only this path fills in each group's parent category.
fn reserve_emoji_group_page_capacity(
    groups: &mut Vec<EmojiCatalogGroup>,
    positions: &mut HashMap<String, usize>,
    item_count: usize,
) {
    if groups.is_empty() {
        groups.reserve(item_count);
        positions.reserve(item_count);
    }
}

fn read_local_emoji_groups(
    resources: &str,
    category: &str,
) -> Result<Vec<EmojiCatalogGroup>, &'static str> {
    if category == "symbols" {
        return msime_host_api::local_symbol_catalog(resources).map(|groups| {
            groups
                .into_iter()
                .map(|group| EmojiCatalogGroup {
                    title: group.title,
                    parent: Some(group.parent),
                    icon: group
                        .items
                        .first()
                        .map(|item| item.text.clone())
                        .unwrap_or_default(),
                    items: group
                        .items
                        .into_iter()
                        .map(|item| EmojiCatalogItem {
                            keywords: if item.annotation.is_empty() {
                                item.group
                            } else {
                                item.annotation
                            },
                            text: item.text,
                        })
                        .collect(),
                })
                .filter(|group| !group.items.is_empty())
                .collect()
        });
    }
    const PAGE_SIZE: u16 = 512;
    let mut groups = Vec::new();
    let mut positions = HashMap::new();
    let mut offset = 0usize;
    let mut complete = false;
    for _ in 0..256 {
        let page =
            msime_host_api::local_emoji_catalog_slice(resources, category, offset, PAGE_SIZE)?;
        reserve_emoji_group_page_capacity(&mut groups, &mut positions, page.items.len());
        for item in page.items {
            if item.text.is_empty() {
                continue;
            }
            let title = if item.group.is_empty() {
                "All".to_owned()
            } else {
                item.group
            };
            let index = if let Some(index) = positions.get(&title).copied() {
                index
            } else {
                let index = groups.len();
                positions.insert(title.clone(), index);
                groups.push(EmojiCatalogGroup {
                    icon: if category == "kaomoji" {
                        ";-)".to_owned()
                    } else {
                        emoji_category_icon(&title).to_owned()
                    },
                    title,
                    parent: None,
                    items: Vec::new(),
                });
                index
            };
            let group = &mut groups[index];
            group.items.push(EmojiCatalogItem {
                keywords: if item.annotation.is_empty() {
                    item.text.clone()
                } else {
                    item.annotation
                },
                text: item.text,
            });
        }
        if page.complete {
            complete = true;
            break;
        }
        if page.next_offset <= offset {
            return Err("local emoji catalog cursor did not advance");
        }
        offset = page.next_offset;
    }
    if !complete {
        return Err("local emoji catalog exceeds limit");
    }
    Ok(groups
        .into_iter()
        .filter(|group| !group.items.is_empty())
        .collect())
}

#[tauri::command]
async fn load_emoji_catalog(
    state: tauri::State<'_, DictionaryHostOptions>,
) -> Result<EmojiCatalogResponse, CommandError> {
    let options = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let document = options.snapshot()?;
        #[cfg(target_os = "linux")]
        let resource_directory = packaged_emoji_resources(&document).ok_or(CommandError {
            code: "unavailable",
        })?;
        #[cfg(target_os = "linux")]
        let resources = resource_directory
            .to_str()
            .ok_or(CommandError { code: "storage" })?;
        #[cfg(not(target_os = "linux"))]
        let resources = document
            .get("resources")
            .and_then(Value::as_str)
            .filter(|value| std::path::Path::new(value).is_absolute())
            .ok_or(CommandError { code: "storage" })?;
        let mut unavailable = Vec::with_capacity(3);
        let mut read = |category, name| {
            read_local_emoji_groups(resources, category).unwrap_or_else(|_| {
                unavailable.push(name);
                Vec::new()
            })
        };
        let emoji = read("", "emoji");
        let kaomoji = read("kaomoji", "kaomoji");
        let symbols = read("symbols", "symbols");
        Ok(EmojiCatalogResponse {
            emoji,
            kaomoji,
            symbols,
            unavailable,
        })
    })
    .await
    .map_err(|_| CommandError {
        code: "unavailable",
    })?
}

#[derive(Debug, serde::Serialize)]
struct HostActionError {
    code: &'static str,
}

#[cfg(any(target_os = "macos", test))]
fn macos_input_source_restart_args() -> [&'static str; 5] {
    [
        "-n",
        "-b",
        "app.msime.inputmethod.MetasequoiaIME",
        "--args",
        "--reregister-input-source",
    ]
}

#[cfg(any(target_os = "linux", test))]
fn linux_input_method_restart_command(
    fcitx5_running: bool,
) -> (&'static str, &'static [&'static str]) {
    if fcitx5_running {
        // Fcitx5 owns the process that loads the MSIME addon, so restarting it would take every other input method in the user's group down too. The controller's ReloadAddonConfig for the `msime` addon reaches the addon's reloadConfig, which resets MSIME in process: it ends every composition, closes the Engine sessions and re-reads runtime-options.json. `fcitx5-remote -r` sends ReloadConfig instead, which reloads only Fcitx5's global configuration and never reaches an addon. The call goes through `gdbus`, the same client msime-linux-setup uses for this controller; `gdbus call` waits for the reply, so a controller that refused the call fails the action.
        (
            "gdbus",
            &[
                "call",
                "--session",
                "--dest",
                "org.fcitx.Fcitx5",
                "--object-path",
                "/controller",
                "--method",
                "org.fcitx.Fcitx.Controller1.ReloadAddonConfig",
                "'msime'",
            ],
        )
    } else {
        ("ibus", &["restart"])
    }
}

// Off the main thread: the Linux restart runs fcitx5-remote and gdbus or ibus
// with timeouts of several seconds.
#[tauri::command]
async fn restart_input_method() -> Result<(), HostActionError> {
    tauri::async_runtime::spawn_blocking(restart_input_method_blocking)
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
}

fn restart_input_method_blocking() -> Result<(), HostActionError> {
    #[cfg(target_os = "windows")]
    {
        const PIPE_NAME: &str = r"\\.\pipe\FanyImeAuxNamedPipe";
        let payload = windows_restart_payload();
        for attempt in 0..5 {
            match fs::OpenOptions::new().write(true).open(PIPE_NAME) {
                Ok(mut pipe) => {
                    return pipe.write_all(&payload).map_err(|_| HostActionError {
                        code: "unavailable",
                    });
                }
                Err(_) if attempt < 4 => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(_) => break,
            }
        }
        Err(HostActionError {
            code: "unavailable",
        })
    }
    #[cfg(target_os = "macos")]
    {
        let status = std::process::Command::new("open")
            .args(macos_input_source_restart_args())
            .status()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        status.success().then_some(()).ok_or(HostActionError {
            code: "unavailable",
        })
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        Err(HostActionError {
            code: "unavailable",
        })
    }
    #[cfg(target_os = "linux")]
    {
        // `--check` asks the live Fcitx5 controller without starting one through
        // D-Bus. Prefer the framework that is actually running; an installed but
        // inactive Fcitx5 must not steal the IBus action.
        let fcitx5_running = linux_process::run_status(
            "fcitx5-remote",
            &["--check"],
            std::time::Duration::from_secs(1),
        );
        let (program, arguments) = linux_input_method_restart_command(fcitx5_running);
        linux_process::run_status(program, arguments, std::time::Duration::from_secs(3))
            .then_some(())
            .ok_or(HostActionError {
                code: "unavailable",
            })
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn install_input_source(app: tauri::AppHandle) -> Result<(), HostActionError> {
    let resource_directory = app.path().resource_dir().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    tauri::async_runtime::spawn_blocking(move || {
        macos_input_source::install(Some(&resource_directory)).map_err(|_| HostActionError {
            code: "unavailable",
        })
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

/// What the start-time install/refresh of the input method did, for the settings page to tell the user.
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, serde::Serialize)]
struct InputSourceStartupStatus {
    /// `installed`, `updated`, `up_to_date`, `not_installed` (a first install, left for the user to start from the install window), `login_required` (installed, but the source list only picks it up after the next login) or `failed`.
    action: &'static str,
    /// Whether the input source is in the System Settings list at the time of the request (see `input_source_status_now`); absent when that list could not be read.
    enabled: Option<bool>,
    bundled_version: Option<String>,
    installed_version: Option<String>,
    /// Copies of the input method in `/Library/Input Methods`, read at the time of the request like `enabled`. They compete with the user's copy and need an administrator to remove, which the settings page status notice asks the user to do.
    system_bundles: Vec<String>,
}

/// The start-time check runs in the background, so the settings page may ask before it has finished; the command waits for it. `None` inside means the check did not run for this launch (a panel launch, a run outside a packaged app, or a build that carries no input method).
#[cfg(target_os = "macos")]
#[derive(Default)]
struct InputSourceStartupState {
    result: Mutex<Option<Option<InputSourceStartupStatus>>>,
    finished: std::sync::Condvar,
    /// Whether the main window opened as the first-install window and the user has not left it yet.
    first_install_window: std::sync::atomic::AtomicBool,
}

#[cfg(target_os = "macos")]
impl InputSourceStartupState {
    fn finish(&self, status: Option<InputSourceStartupStatus>) {
        *self
            .result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(status);
        self.finished.notify_all();
    }

    fn wait(&self, timeout: std::time::Duration) -> Option<InputSourceStartupStatus> {
        let guard = self
            .result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let (guard, _) = self
            .finished
            .wait_timeout_while(guard, timeout, |result| result.is_none())
            .unwrap_or_else(|poison| poison.into_inner());
        guard.clone().flatten()
    }
}

#[cfg(target_os = "macos")]
fn run_input_source_startup(
    resource_directory: &std::path::Path,
    defer_first_install: bool,
) -> Option<InputSourceStartupStatus> {
    let status = match macos_input_source::ensure_current(resource_directory, defer_first_install) {
        Ok(outcome) => InputSourceStartupStatus {
            action: match outcome.refresh {
                macos_input_source::Refresh::Install => "installed",
                macos_input_source::Refresh::Update => "updated",
                macos_input_source::Refresh::UpToDate => "up_to_date",
                macos_input_source::Refresh::Deferred => "not_installed",
            },
            enabled: None,
            system_bundles: Vec::new(),
            bundled_version: outcome
                .bundled
                .as_ref()
                .map(|version| version.label().to_string()),
            installed_version: outcome
                .installed
                .as_ref()
                .map(|version| version.label().to_string()),
        },
        Err(macos_input_source::InstallError::SourceUnavailable) => return None,
        // A failed install or registration has already restored the previous bundle, so the installed version reported is the one still in place; a first install whose registration waits for the next login keeps the new bundle, so that is the one reported.
        Err(error) => InputSourceStartupStatus {
            action: if matches!(error, macos_input_source::InstallError::RegistrationPending) {
                "login_required"
            } else {
                "failed"
            },
            enabled: None,
            system_bundles: Vec::new(),
            bundled_version: macos_input_source::bundle_version(
                &resource_directory.join(macos_input_source::INPUT_SOURCE_BUNDLE_NAME),
            )
            .map(|version| version.label().to_string()),
            installed_version: macos_input_source::installed_bundle_path()
                .ok()
                .and_then(|path| macos_input_source::bundle_version(&path))
                .map(|version| version.label().to_string()),
        },
    };
    Some(status)
}

/// The start-time result with `enabled` and `system_bundles` read at the time of the call rather than when that check ran. The settings page may ask again at any time, so this must stay cheap: it never copies or registers anything, only waits for the one start-time check, reads the input source list and looks for a few paths.
#[cfg(target_os = "macos")]
fn input_source_status_now(
    state: &InputSourceStartupState,
    timeout: std::time::Duration,
    enabled: impl FnOnce() -> Option<bool>,
    system_bundles: impl FnOnce() -> Vec<std::path::PathBuf>,
) -> Option<InputSourceStartupStatus> {
    let mut status = state.wait(timeout)?;
    status.enabled = enabled();
    status.system_bundles = system_bundles()
        .into_iter()
        .map(|path| path.display().to_string())
        .collect();
    Some(status)
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn input_source_startup_status(
    state: tauri::State<'_, Arc<InputSourceStartupState>>,
) -> Result<Option<InputSourceStartupStatus>, HostActionError> {
    let state = Arc::clone(&state);
    // Copying and registering takes seconds, not minutes; the bound only keeps a wedged registration from holding the page's request open forever.
    tauri::async_runtime::spawn_blocking(move || {
        input_source_status_now(
            &state,
            std::time::Duration::from_secs(120),
            macos_input_source::input_source_enabled,
            macos_input_source::system_bundles,
        )
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })
}

/// The install window's button: the start-time check run again without deferring, so a first install reports the same `installed` / `login_required` / `failed` the start-time check would have. The result replaces the start-time one, which is what the settings page reads once the window has made way for it.
#[cfg(target_os = "macos")]
#[tauri::command]
async fn run_first_input_source_install(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<InputSourceStartupState>>,
) -> Result<InputSourceStartupStatus, HostActionError> {
    let resource_directory = app.path().resource_dir().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    let state = Arc::clone(&state);
    tauri::async_runtime::spawn_blocking(move || {
        let status = run_input_source_startup(&resource_directory, false);
        state.finish(status);
        input_source_status_now(
            &state,
            std::time::Duration::ZERO,
            macos_input_source::input_source_enabled,
            macos_input_source::system_bundles,
        )
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
    .ok_or(HostActionError {
        code: "unavailable",
    })
}

/// The install window's size. The main window opens at it on a first install (see the start-time check in `run`) and returns to the settings size once the user leaves the window.
#[cfg(target_os = "macos")]
const FIRST_INSTALL_WINDOW_SIZE: (f64, f64) = (480.0, 440.0);
/// The settings window's size and minimum, as declared for `main` in tauri.macos.conf.json.
#[cfg(target_os = "macos")]
const SETTINGS_WINDOW_SIZE: (f64, f64) = (1000.0, 780.0);
#[cfg(target_os = "macos")]
const SETTINGS_WINDOW_MIN_SIZE: (f64, f64) = (360.0, 540.0);

#[cfg(target_os = "macos")]
fn shape_first_install_window(window: &tauri::WebviewWindow) {
    let (width, height) = FIRST_INSTALL_WINDOW_SIZE;
    let _ = window.set_min_size(None::<tauri::LogicalSize<f64>>);
    let _ = window.set_size(tauri::LogicalSize::new(width, height));
    let _ = window.set_resizable(false);
    let _ = window.set_maximizable(false);
    let _ = window.center();
}

/// Whether the page should open as the install window: answered from what the window setup decided, so the settings page never waits on the start-time check for it.
#[cfg(target_os = "macos")]
#[tauri::command]
fn first_install_window_pending(state: tauri::State<'_, Arc<InputSourceStartupState>>) -> bool {
    state
        .first_install_window
        .load(std::sync::atomic::Ordering::Acquire)
}

/// Gives the main window back its settings size when the user leaves the install window.
#[cfg(target_os = "macos")]
#[tauri::command]
fn leave_first_install_window(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<InputSourceStartupState>>,
) -> Result<(), HostActionError> {
    state
        .first_install_window
        .store(false, std::sync::atomic::Ordering::Release);
    let (width, height) = SETTINGS_WINDOW_SIZE;
    let (min_width, min_height) = SETTINGS_WINDOW_MIN_SIZE;
    let unavailable = |_| HostActionError {
        code: "unavailable",
    };
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

#[cfg(target_os = "macos")]
#[tauri::command]
fn open_input_source_settings() -> Result<(), HostActionError> {
    let status = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.Keyboard-Settings.extension")
        .status()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    status.success().then_some(()).ok_or(HostActionError {
        code: "unavailable",
    })
}

// The input method writes through NSUserDefaults.standardUserDefaults, so its domain is its bundle identifier; reading any other name finds an empty - or stale - plist while the settings page reports that it saved.
#[cfg(target_os = "macos")]
const MACOS_INPUT_METHOD_DEFAULTS_DOMAIN: &str = "app.msime.inputmethod.MetasequoiaIME";

#[cfg(target_os = "macos")]
const MACOS_SHUANGPIN_KEYMAP_DEFAULTS_KEY: &str = "MSIMEClientShuangpinKeymap";

#[cfg(target_os = "macos")]
const MACOS_WUBI_AUTO_COMMIT_UNIQUE_DEFAULTS_KEY: &str = "MSIMEClientWubiAutoCommitUnique";

#[cfg(target_os = "macos")]
#[tauri::command]
async fn load_macos_shuangpin_keymap() -> Result<bool, HostActionError> {
    tauri::async_runtime::spawn_blocking(|| {
        let output = std::process::Command::new("defaults")
            .args([
                "read",
                MACOS_INPUT_METHOD_DEFAULTS_DOMAIN,
                MACOS_SHUANGPIN_KEYMAP_DEFAULTS_KEY,
            ])
            .output()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        if !output.status.success() {
            // An unset NSUserDefaults boolean has the same false value as
            // boolForKey:, which is what the native input controller uses.
            return Ok(false);
        }
        let value = String::from_utf8_lossy(&output.stdout);
        match value.trim() {
            "1" | "true" | "TRUE" => Ok(true),
            "0" | "false" | "FALSE" => Ok(false),
            _ => Err(HostActionError {
                code: "unavailable",
            }),
        }
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn save_macos_shuangpin_keymap(enabled: bool) -> Result<(), HostActionError> {
    tauri::async_runtime::spawn_blocking(move || {
        let status = std::process::Command::new("defaults")
            .args([
                "write",
                MACOS_INPUT_METHOD_DEFAULTS_DOMAIN,
                MACOS_SHUANGPIN_KEYMAP_DEFAULTS_KEY,
                "-bool",
                if enabled { "true" } else { "false" },
            ])
            .status()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        status.success().then_some(()).ok_or(HostActionError {
            code: "unavailable",
        })
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn load_macos_wubi_auto_commit_unique() -> Result<bool, HostActionError> {
    tauri::async_runtime::spawn_blocking(|| {
        let output = std::process::Command::new("defaults")
            .args([
                "read",
                MACOS_INPUT_METHOD_DEFAULTS_DOMAIN,
                MACOS_WUBI_AUTO_COMMIT_UNIQUE_DEFAULTS_KEY,
            ])
            .output()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        if !output.status.success() {
            // NSUserDefaults.boolForKey: also treats an unset value as false.
            return Ok(false);
        }
        let value = String::from_utf8_lossy(&output.stdout);
        match value.trim() {
            "1" | "true" | "TRUE" => Ok(true),
            "0" | "false" | "FALSE" => Ok(false),
            _ => Err(HostActionError {
                code: "unavailable",
            }),
        }
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn save_macos_wubi_auto_commit_unique(enabled: bool) -> Result<(), HostActionError> {
    tauri::async_runtime::spawn_blocking(move || {
        let status = std::process::Command::new("defaults")
            .args([
                "write",
                MACOS_INPUT_METHOD_DEFAULTS_DOMAIN,
                MACOS_WUBI_AUTO_COMMIT_UNIQUE_DEFAULTS_KEY,
                "-bool",
                if enabled { "true" } else { "false" },
            ])
            .status()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        status.success().then_some(()).ok_or(HostActionError {
            code: "unavailable",
        })
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

// The input method's Swift backend records here the Apple translation pairs (Simplified Chinese to each code) that it found downloadable but not yet downloaded, so its on-device glosses stay empty until the user downloads them in System Settings.
#[cfg(target_os = "macos")]
const MACOS_ON_DEVICE_TRANSLATION_DOWNLOADABLE_DEFAULTS_KEY: &str =
    "MSIMEOnDeviceTranslationDownloadableLanguages";

// Only the target languages the settings page can choose; anything else in the value is not ours to report.
#[cfg(any(target_os = "macos", test))]
fn parse_on_device_translation_downloadable(value: &str) -> Vec<String> {
    // The supported translation language set has eight entries; `zh` is filtered out below.
    let mut codes = Vec::with_capacity(7);
    for code in value.trim().split(',').map(str::trim) {
        if code != "zh"
            && msime_client_core::translation::is_supported_translation_language(code)
            && !codes.iter().any(|known| known == code)
        {
            codes.push(code.to_owned());
        }
    }
    codes
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn on_device_translation_downloadable_languages() -> Result<Vec<String>, HostActionError> {
    tauri::async_runtime::spawn_blocking(|| {
        let output = std::process::Command::new("defaults")
            .args([
                "read",
                MACOS_INPUT_METHOD_DEFAULTS_DOMAIN,
                MACOS_ON_DEVICE_TRANSLATION_DOWNLOADABLE_DEFAULTS_KEY,
            ])
            .output()
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
        // The key is removed once every pair it listed is downloaded, and never written before the input method first asks.
        if !output.status.success() {
            return Ok(Vec::new());
        }
        Ok(parse_on_device_translation_downloadable(
            &String::from_utf8_lossy(&output.stdout),
        ))
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

// Translation languages are downloaded under 语言与地区; System Settings has no URL for the sheet itself.
#[cfg(target_os = "macos")]
#[tauri::command]
fn open_translation_language_settings() -> Result<(), HostActionError> {
    let status = std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.Localization-Settings.extension")
        .status()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    status.success().then_some(()).ok_or(HostActionError {
        code: "unavailable",
    })
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn pick_voice_model_path(app: tauri::AppHandle) -> Result<Option<String>, HostActionError> {
    // A local speech model is an installed directory loaded by path, and a web view's file input hands back contents instead, so the settings page cannot resolve one itself. AppKit will only run the panel on the main thread.
    let (send, received) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = send.send(msime_host_macos::pick_voice_model_directory());
    })
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    received.recv().map_err(|_| HostActionError {
        code: "unavailable",
    })
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn pick_data_directory(
    app: tauri::AppHandle,
    selection: tauri::State<'_, DataDirectorySelectionState>,
) -> Result<Option<String>, HostActionError> {
    let (send, received) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = send.send(msime_host_macos::pick_directory());
    })
    .map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let chosen = received.recv().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let path = chosen.map(PathBuf::from);
    *selection.0.lock().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })? = path.clone();
    Ok(path.map(|path| path.to_string_lossy().into_owned()))
}

#[cfg(target_os = "macos")]
#[derive(Default)]
struct DataDirectorySelectionState(Mutex<Option<PathBuf>>);

#[cfg(target_os = "macos")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DataDirectoryStatus {
    path: String,
    is_default: bool,
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn data_directory_status(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, RuntimeOptionsState>,
) -> Result<DataDirectoryStatus, HostActionError> {
    let document = runtime.snapshot().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let path = document
        .get("preferences_directory")
        .and_then(Value::as_str)
        .filter(|path| PathBuf::from(path).is_absolute())
        .ok_or(HostActionError {
            code: "data_directory_unavailable",
        })?;
    let default = app.path().app_data_dir().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    Ok(DataDirectoryStatus {
        path: path.to_owned(),
        is_default: std::path::Path::new(path) == default,
    })
}

#[cfg(target_os = "macos")]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DataDirectoryMoveResult {
    path: String,
    is_default: bool,
    retained_old_data: bool,
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn move_data_directory(
    app: tauri::AppHandle,
    runtime: tauri::State<'_, RuntimeOptionsState>,
    selection: tauri::State<'_, DataDirectorySelectionState>,
) -> Result<DataDirectoryMoveResult, HostActionError> {
    let document = runtime.snapshot().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let source = document
        .get("preferences_directory")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(HostActionError {
            code: "data_directory_unavailable",
        })?;
    let resources = document
        .get("resources")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(HostActionError {
            code: "data_directory_unavailable",
        })?;
    let target = selection
        .0
        .lock()
        .map_err(|_| HostActionError {
            code: "data_directory_unavailable",
        })?
        .take()
        .ok_or(HostActionError {
            code: "data_directory_invalid",
        })?;
    let default_root = app.path().app_data_dir().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let native_root = macos_launch::native_locator_root().map_err(|_| HostActionError {
        code: "data_directory_unavailable",
    })?;
    let locators = vec![
        default_root.join("runtime-options.json"),
        native_root.join("runtime-options.json"),
    ];

    let (send, received) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = send.send(msime_host_macos::stop_input_method());
    })
    .map_err(|_| HostActionError {
        code: "data_directory_busy",
    })?;
    if !received.recv().unwrap_or(false) {
        return Err(HostActionError {
            code: "data_directory_busy",
        });
    }

    let result = tauri::async_runtime::spawn_blocking(move || {
        let is_default =
            std::fs::canonicalize(&target).ok() == std::fs::canonicalize(&default_root).ok();
        macos_data_directory::move_data_directory(
            &source,
            &target,
            &default_root,
            &native_root,
            &locators,
            |destination| {
                let document = msime_host_api::prepare_host_configuration(&resources, destination)
                    .map_err(|_| macos_data_directory::MoveError::Prepare)?;
                serde_json::from_str(&document)
                    .map_err(|_| macos_data_directory::MoveError::Prepare)
            },
        )
        .map(|outcome| (target, outcome, is_default))
    })
    .await
    .map_err(|_| HostActionError {
        code: "data_directory_move_failed",
    })?
    .map_err(|error| HostActionError {
        code: match error {
            macos_data_directory::MoveError::InvalidTarget => "data_directory_invalid",
            macos_data_directory::MoveError::TargetNotEmpty => "data_directory_not_empty",
            macos_data_directory::MoveError::InvalidSource => "data_directory_unavailable",
            macos_data_directory::MoveError::Copy
            | macos_data_directory::MoveError::Prepare
            | macos_data_directory::MoveError::Publish => "data_directory_move_failed",
        },
    })?;
    let response = DataDirectoryMoveResult {
        path: result.0.to_string_lossy().into_owned(),
        is_default: result.2,
        retained_old_data: result.1.retained_old_data,
    };
    let exit_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(500));
        exit_app.exit(0);
    });
    Ok(response)
}

#[cfg(target_os = "macos")]
#[tauri::command]
async fn uninstall_input_source(
    remove_user_data: bool,
    app: tauri::AppHandle,
    runtime: tauri::State<'_, RuntimeOptionsState>,
) -> Result<(), HostActionError> {
    let document = runtime.snapshot().map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    let state = document
        .get("preferences_directory")
        .and_then(Value::as_str)
        .filter(|path| std::path::Path::new(path).is_absolute())
        .map(PathBuf::from)
        .ok_or(HostActionError {
            code: "unavailable",
        })?;
    let home = std::env::var_os("HOME").ok_or(HostActionError {
        code: "unavailable",
    })?;
    let input_methods = PathBuf::from(home).join("Library/Input Methods");
    // Prefer the current product name, but remove a copy left by the previous preview build if
    // that is the one still installed. Both carry the same bundle identifier.
    let bundle = ["水杉输入法.app", "水杉输入法（预览）.app"]
        .into_iter()
        .map(|name| input_methods.join(name))
        .find(|candidate| candidate.exists())
        .unwrap_or_else(|| input_methods.join("水杉输入法.app"));
    tauri::async_runtime::spawn_blocking(move || {
        // Wait for a start-time refresh or a manual install that is still writing the bundle.
        let _guard = macos_input_source::install_lock();
        msime_host_macos::uninstall_input_source(&bundle, &state, remove_user_data).map_err(|_| {
            HostActionError {
                code: "unavailable",
            }
        })
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })??;
    // The installed bundle is gone after a successful operation. Exit the
    // settings shell too, matching the native Apple flow and avoiding a UI
    // process that can no longer repair the removed installation.
    app.exit(0);
    Ok(())
}

fn windows_restart_payload() -> Vec<u8> {
    "RestartServer"
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect()
}

/// The colour a window wears before its page has painted anything.
///
/// A window is on screen the moment it is created, but the webview has nothing to show until the
/// bundle has loaded and rendered, and an empty webview is painted in the platform's default -
/// white on Windows, whatever theme the user runs. The reference covers that gap with a themed
/// Direct2D splash aligned to the settings frame (`settings_splash.cpp`, and one per panel). The
/// same gap closes here by handing the window the colour the page is about to paint anyway: there
/// is then nothing to see rather than a white flash.
///
/// These are `--chrome-bg` from the shared stylesheet, duplicated because a window background
/// cannot read CSS. `chrome_background_matches_the_shared_stylesheet` fails if they drift.
pub(crate) const CHROME_BACKGROUND_DARK: tauri::window::Color =
    tauri::window::Color(0x20, 0x20, 0x20, 0xff);
pub(crate) const CHROME_BACKGROUND_LIGHT: tauri::window::Color =
    tauri::window::Color(0xf3, 0xf3, 0xf3, 0xff);

/// Unknown theme takes the light colour: that is the platform default this is correcting, so a
/// wrong guess there is no worse than doing nothing.
pub(crate) fn chrome_background(theme: Option<tauri::Theme>) -> tauri::window::Color {
    match theme {
        Some(tauri::Theme::Dark) => CHROME_BACKGROUND_DARK,
        _ => CHROME_BACKGROUND_LIGHT,
    }
}

fn launch_route_from_args(args: &[String]) -> Option<SurfaceRoute> {
    args.iter().find_map(|argument| {
        argument
            .strip_prefix("--route=")
            .and_then(|route| SurfaceRoute::parse(route).ok())
    })
}

/// What a second launch should bring to the front.
///
/// Every surface this product opens for itself names one: the Windows host builds `--route=` into
/// the command line in `ShellLauncher.cpp`, and the tray, the toolbar and the panels all go through
/// it. So a launch *without* a route is the user starting the application themselves - the Start
/// menu entry, a shortcut, the installed icon - and the window they meant is the settings window.
///
/// Doing nothing in that case is what makes a second launch look broken: the running instance stays
/// behind whatever is in front, and the click appears to be ignored. The reference activates its
/// existing window here rather than exiting silently.
fn second_launch_route(args: &[String]) -> SurfaceRoute {
    launch_route_from_args(args).unwrap_or(SurfaceRoute::Settings(None))
}

/// Whether this macOS launch joins the single settings instance.
///
/// Only the settings window is one per user, as on Windows where `settings_launcher.cpp` re-routes the existing window instead of starting another. Panel launches carry a per-session identity and options path in their environment and exit with their panel, so each keeps its own process. A launch without a route is Finder or Launchpad opening the application, which means the settings window.
#[cfg(target_os = "macos")]
fn macos_settings_launch(route: Option<SurfaceRoute>) -> bool {
    route.is_none_or(|route| route.panel().is_none())
}

#[cfg(any(target_os = "linux", target_os = "windows"))]
fn cancel_settings_linger(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<DesktopSettingsLinger>() {
        state.generation.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
fn activate_desktop_surface(app: &tauri::AppHandle, route: SurfaceRoute) {
    // macOS has no settings linger: its settings process exits when the window closes.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    cancel_settings_linger(app);
    // On macOS only settings launches reach the running instance (see `macos_settings_launch`), so a forwarded route always names the settings window.
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    if let Some(surface) = route.panel_for(host_platform()) {
        let state = app.state::<PanelInputState>();
        let _ = panel_window::open_surface_panel(app, &state, surface);
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let page = route
        .settings_category()
        .map(|category| category.as_str())
        .unwrap_or_default();
    let _ = app.emit("settings-route", page);
}

// The Linux capture runs swaymsg, xdotool and the like; a synchronous command
// would hold the GTK main thread for their timeouts.
#[cfg(target_os = "linux")]
#[tauri::command]
async fn remember_input_target(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
) -> Result<(), HostActionError> {
    let label = window.label().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<PanelInputState>();
        remember_panel_input_target(&state, &label, label == "keyboard-panel")
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
#[allow(unused_variables)]
fn remember_input_target(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, PanelInputState>,
) -> Result<(), HostActionError> {
    #[cfg(target_os = "windows")]
    return remember_panel_input_target(&state);
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        #[cfg(target_os = "macos")]
        {
            let _ = window;
            let target = msime_host_macos::capture_launch_target().ok_or(HostActionError {
                code: "unavailable",
            })?;
            *state.0.lock().map_err(|_| HostActionError {
                code: "unavailable",
            })? = Some(PanelInputTarget(target));
            return Ok(());
        }
        let _ = (window, state);
        Ok(())
    }
}

#[tauri::command]
async fn send_key(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, PanelInputState>,
    request: KeyboardInputRequest,
) -> Result<(), HostActionError> {
    // Linux routes through the display server, Windows injects directly, so the
    // app handle belongs to only one of them.
    let _ = (&app, &window, &state);
    #[cfg(target_os = "linux")]
    {
        request.validate().map_err(|_| HostActionError {
            code: "invalid_key",
        })?;
        // The non-focusable keyboard follows the editor the user is typing
        // into now, like Windows RememberInputTargetWindow on each key press.
        // Other panels retain their original destination while being edited.
        // The keyboard panel is non-activating. Its serialized UI queue
        // refreshes this target immediately before each key, matching the
        // Windows panel's per-click foreground capture. Do not re-probe here:
        // an async command could otherwise observe a different editor than
        // the one captured for this queued key.
        let target = panel_input_target(&state, window.label())?;
        return tauri::async_runtime::spawn_blocking(move || send_panel_key(&app, target, request))
            .await
            .map_err(|_| HostActionError {
                code: "unavailable",
            })?;
    }
    #[cfg(target_os = "windows")]
    return send_panel_key_windows(&state, request, window.label() == "keyboard-panel");
    #[cfg(target_os = "macos")]
    {
        request.validate().map_err(|_| HostActionError {
            code: "invalid_key",
        })?;
        // Only the non-focusable keyboard follows the current editor. Other
        // panels use their own captured input-session handoff.
        if window.label() != "keyboard-panel" {
            return Err(HostActionError {
                code: "unavailable",
            });
        }
        let (send, received) = std::sync::mpsc::sync_channel(1);
        app.run_on_main_thread(move || {
            // The keyboard is non-activating, so the user can switch editors
            // while it remains open. Resolve the foreground target immediately
            // before each stroke and re-check it in native code before posting.
            let _ = send.send(msime_host_macos::send_keyboard_key(&request));
        })
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
        tauri::async_runtime::spawn_blocking(move || {
            received
                .recv()
                .unwrap_or(false)
                .then_some(())
                .ok_or(HostActionError {
                    code: "unavailable",
                })
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, state, request);
        Err(HostActionError {
            code: "unavailable",
        })
    }
}

#[tauri::command]
async fn recognize_handwriting(
    request: HandwritingRecognitionRequest,
    options: tauri::State<'_, DictionaryHostOptions>,
) -> Result<HandwritingRecognitionResult, HostActionError> {
    request.validate().map_err(|_| HostActionError {
        code: "invalid_stroke",
    })?;
    let query = HandwritingQuery {
        language: request.language,
        strokes: request
            .strokes
            .into_iter()
            .map(|stroke| {
                stroke
                    .points
                    .into_iter()
                    .map(|point| HandwritingPoint {
                        x: point.x,
                        y: point.y,
                    })
                    .collect()
            })
            .collect(),
    };
    let options = options.inner().clone();
    let model = tauri::async_runtime::spawn_blocking(move || {
        let document = options.snapshot().map_err(|_| HostActionError {
            code: "unavailable",
        })?;
        let document = serde_json::to_string(&document).map_err(|_| HostActionError {
            code: "unavailable",
        })?;
        Ok::<_, HostActionError>(packaged_handwriting_model(&document))
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })??;
    // A user-managed socket owns recognizer and model policy where one is
    // configured; otherwise the Engine's packaged recognizer answers, which is
    // the only path hosts without unix sockets have.
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
    let socket = match std::env::var_os("MSIME_HANDWRITING_PROVIDER_SOCKET") {
        Some(value) => {
            let path = PathBuf::from(value);
            if !path.is_absolute() {
                return Err(HostActionError {
                    code: "unavailable",
                });
            }
            Some(path)
        }
        None => discover_session_provider("handwriting.sock"),
    };
    #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
    if let Some(path) = socket {
        let candidates = tauri::async_runtime::spawn_blocking(move || {
            UnixSocketProvider::new(path).handwriting(query)
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .ok_or(HostActionError {
            code: "unavailable",
        })?;
        let result = HandwritingRecognitionResult { candidates };
        result.validate().map_err(|_| HostActionError {
            code: "invalid_stroke",
        })?;
        return Ok(result);
    }
    // Windows ships a recognizer with the language pack, and it is the only one
    // a stock machine has: the packaged Engine model is optional in the
    // installer. Try it first, and fall through to the model when Windows has
    // no Chinese handwriting feature installed.
    #[cfg(windows)]
    {
        let strokes: Vec<msime_host_windows::ink::Stroke> = query
            .strokes
            .iter()
            .map(|stroke| stroke.iter().map(|point| (point.x, point.y)).collect())
            .collect();
        let recognized = tauri::async_runtime::spawn_blocking(move || {
            msime_host_windows::ink::recognize(&strokes)
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
        match recognized {
            Ok(candidates) if !candidates.is_empty() => {
                let result = HandwritingRecognitionResult { candidates };
                result.validate().map_err(|_| HostActionError {
                    code: "invalid_stroke",
                })?;
                return Ok(result);
            }
            // Recognized nothing, or Windows has no Chinese recognizer. Either
            // way the packaged model below is still worth asking.
            _ => {}
        }
    }
    let Some(model) = model else {
        return Err(HostActionError {
            code: "unavailable",
        });
    };
    let candidates = tauri::async_runtime::spawn_blocking(move || {
        msime_host_api::handwriting_local_candidates(model.to_str().unwrap_or_default(), &query)
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    let result = HandwritingRecognitionResult { candidates };
    result.validate().map_err(|_| HostActionError {
        code: "invalid_stroke",
    })?;
    Ok(result)
}

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
fn discover_session_provider(filename: &str) -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|directory| directory.is_absolute())
        .and_then(|directory| discover_session_provider_in(&directory, filename))
}

#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
fn discover_session_provider_in(
    runtime_directory: &std::path::Path,
    filename: &str,
) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;

    let directory = runtime_directory.join("msime-client");
    let path = directory.join(filename);
    let directory_metadata = std::fs::symlink_metadata(&directory).ok()?;
    let socket_metadata = std::fs::symlink_metadata(&path).ok()?;
    let uid = rustix::process::geteuid().as_raw();
    if !directory_metadata.file_type().is_dir()
        || directory_metadata.uid() != uid
        || directory_metadata.mode() & 0o077 != 0
        || !socket_metadata.file_type().is_socket()
        || socket_metadata.uid() != uid
    {
        return None;
    }
    Some(path)
}

/// Locate the Engine's packaged handwriting model: the host options first, then
/// an explicit override, then the layouts the installers produce. Only an
/// absolute path to a file that exists is accepted, so a stale setting cannot
/// send strokes at something else.
fn packaged_handwriting_model(host_options: &str) -> Option<PathBuf> {
    serde_json::from_str::<Value>(host_options)
        .ok()
        .and_then(|value| {
            value
                .get("handwriting_model")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned)
        })
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("MSIME_HANDWRITING_MODEL")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| {
            #[cfg(target_os = "macos")]
            {
                std::env::current_exe()
                    .ok()
                    .and_then(|executable| macos_handwriting::bundled_model(&executable))
            }
            #[cfg(not(target_os = "macos"))]
            {
                None
            }
        })
        .or_else(|| {
            discover_packaged_file(
                "msime-client/handwriting/handwriting-zh_CN.model",
                "handwriting/handwriting-zh_CN.model",
            )
        })
        .filter(|path| path.is_absolute() && path.is_file())
}

#[cfg(target_os = "linux")]
fn packaged_emoji_resources(document: &Value) -> Option<PathBuf> {
    // Explicit configuration owns catalog selection: invalid paths must not
    // silently switch to a different installed catalog.
    let configured = document
        .get("resources")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("MSIME_EMOJI_RESOURCES")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        });
    if let Some(directory) = configured {
        return (directory.is_absolute() && directory.join("others.db").is_file())
            .then_some(directory);
    }
    discover_packaged_file("msime-client/emoji/others.db", "emoji/others.db")
        .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
}

fn discover_packaged_file(relative: &str, beside_executable: &str) -> Option<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(target_os = "linux")]
    {
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .filter(|path| path.is_absolute())
                    .map(|path| path.join(".local/share"))
            });
        if let Some(root) = data_home {
            candidates.push(root.join(relative));
        }
    }

    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            // Preserve the Windows bundle and relocatable Unix prefix layouts.
            candidates.push(directory.join(beside_executable));
            if let Some(prefix) = directory.parent() {
                candidates.push(prefix.join("share").join(relative));
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        let directories = std::env::var_os("XDG_DATA_DIRS")
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        candidates.extend(
            std::env::split_paths(&directories)
                .filter(|path| path.is_absolute())
                .map(|path| path.join(relative)),
        );
    }

    candidates
        .into_iter()
        .find(|path| path.is_absolute() && path.is_file())
}

#[tauri::command]
async fn submit_handwriting_candidate(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, PanelInputState>,
    typing_statistics: tauri::State<'_, TypingStatisticsState>,
    candidate: String,
) -> Result<(), HostActionError> {
    let _ = (&window, &typing_statistics, &state);
    #[cfg(target_os = "linux")]
    {
        msime_client_core::panels::validate_candidate(&candidate).map_err(|_| HostActionError {
            code: "invalid_text",
        })?;
        return send_panel_text(
            app,
            &state,
            &typing_statistics,
            window.label().to_owned(),
            candidate,
            TypingSource::Handwriting,
        )
        .await;
    }
    // Recognition without a way to commit is half a panel: Windows could
    // produce candidates and then refuse to insert the one the user picked.
    #[cfg(target_os = "windows")]
    {
        // Windows panels inject through the host rather than the runtime, so
        // they do not pass through the typing counter, matching send_text.
        let _ = (&app, &typing_statistics);
        msime_client_core::panels::validate_candidate(&candidate).map_err(|_| HostActionError {
            code: "invalid_text",
        })?;
        send_panel_text_windows(&state, &candidate)
    }
    #[cfg(target_os = "macos")]
    return macos_panel_session::submit(app, window, candidate).await;
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, state, candidate);
        Err(HostActionError {
            code: "unavailable",
        })
    }
}

#[tauri::command]
async fn send_text(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, PanelInputState>,
    typing_statistics: tauri::State<'_, TypingStatisticsState>,
    text: String,
) -> Result<(), HostActionError> {
    let _ = (&app, &window, &state);
    let _ = &typing_statistics;
    // The cloud clipboard panel only types into the editor captured for its current open; a page that asks anyway, for instance one still running from before the target was dropped, is refused here rather than trusted to have checked `cloud_clipboard_can_send_text`.
    #[cfg(target_os = "linux")]
    if window.label() == CLOUD_CLIPBOARD_PANEL {
        return panel_input::send_cloud_clipboard_text(app, &typing_statistics, text).await;
    }
    #[cfg(target_os = "windows")]
    if window.label() == CLOUD_CLIPBOARD_PANEL {
        return panel_input::send_cloud_clipboard_text_windows(&app, &text);
    }
    #[cfg(target_os = "linux")]
    return send_panel_text(
        app,
        &state,
        &typing_statistics,
        window.label().to_owned(),
        text,
        TypingSource::Unknown,
    )
    .await;
    #[cfg(target_os = "windows")]
    return send_panel_text_windows(&state, &text);
    #[cfg(target_os = "macos")]
    return macos_panel_session::submit(app, window, text).await;
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, state, text);
        Err(HostActionError {
            code: "unavailable",
        })
    }
}

#[tauri::command]
fn supports_clipboard_paste() -> bool {
    cfg!(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos"
    ))
}

#[tauri::command]
#[allow(unused_variables)]
async fn paste_clipboard_text(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, PanelInputState>,
    text: String,
) -> Result<(), HostActionError> {
    #[cfg(target_os = "linux")]
    {
        if text.is_empty()
            || text.len() > msime_client_core::clipboard::MAX_TEXT_BYTES
            || text.contains('\0')
        {
            return Err(HostActionError {
                code: "invalid_text",
            });
        }
        let target = panel_input_target(&state, window.label())?;
        tauri::async_runtime::spawn_blocking(move || {
            if !write_linux_clipboard(&text) {
                return Err(HostActionError {
                    code: "unavailable",
                });
            }
            send_panel_ctrl_v(&app, &target)
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
    }
    #[cfg(target_os = "windows")]
    {
        if text.is_empty()
            || text.len() > msime_client_core::clipboard::MAX_TEXT_BYTES
            || text.contains('\0')
        {
            return Err(HostActionError {
                code: "invalid_text",
            });
        }
        let target = {
            let mut remembered = state.0.lock().map_err(|_| HostActionError {
                code: "unavailable",
            })?;
            // Clipboard/Emoji submission can be asynchronous. If the user
            // moved to another editor while the panel was open, use the
            // current external foreground window just like voice submission;
            // when the panel is foreground, retain the captured destination.
            if msime_host_windows::foreground_is_external() {
                if let Some(current) = msime_host_windows::foreground_window() {
                    *remembered = Some(PanelInputTarget(current));
                }
            }
            remembered
                .as_ref()
                .map(|target| target.0)
                .ok_or(HostActionError {
                    code: "unavailable",
                })?
        };
        return tauri::async_runtime::spawn_blocking(move || {
            msime_host_windows::paste_text(target, &text)
                .then_some(())
                .ok_or(HostActionError {
                    code: "unavailable",
                })
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    }
    #[cfg(target_os = "macos")]
    return macos_panel_session::submit_clipboard(app, window, text).await;
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = (app, window, state, text);
        Err(HostActionError {
            code: "unavailable",
        })
    }
}

#[tauri::command]
async fn send_voice_text(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, PanelInputState>,
    typing_statistics: tauri::State<'_, TypingStatisticsState>,
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
    text: String,
) -> Result<(), HostActionError> {
    #[cfg(not(target_os = "macos"))]
    let _ = &window;
    #[cfg(target_os = "macos")]
    let _ = (&state, &typing_statistics, &store);
    #[cfg(target_os = "windows")]
    {
        let _ = (&app, &window);
        let target = {
            let mut remembered = state.0.lock().map_err(|_| HostActionError {
                code: "unavailable",
            })?;
            // Voice recognition is asynchronous. If the user moved to another
            // editor while it was running, the external foreground window is
            // the new destination; when the panel itself is foreground keep
            // the target captured before the panel opened.
            if msime_host_windows::foreground_is_external() {
                if let Some(current) = msime_host_windows::foreground_window() {
                    *remembered = Some(PanelInputTarget(current));
                }
            }
            remembered
                .as_ref()
                .map(|target| target.0)
                .ok_or(HostActionError {
                    code: "unavailable",
                })?
        };
        let store = store.inner().clone();
        let statistics = typing_statistics.0.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mode = store
                .load()
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?
                .preferences
                .voice_input
                .commit_mode;
            voice_output::submit(&text, &mode, |mode, text| match mode {
                // The native VoiceInputSession owns the Server's TSF
                // composition lease. A shared Tauri voice panel has no TSF
                // context of its own, so adapt the default `tsf` preference
                // to the same guarded foreground injection used by its
                // explicit SendInput mode. The native hotkey path remains
                // unchanged and continues to commit through TSF.
                voice_output::OutputMode::Tsf | voice_output::OutputMode::SendInput => {
                    msime_host_windows::focus_external(target)
                        && msime_host_windows::send_text(text)
                }
                voice_output::OutputMode::Clipboard => {
                    msime_host_windows::paste_voice_text(target, text)
                }
            })
            .map_err(|error| HostActionError {
                code: match error {
                    voice_output::OutputError::InvalidText => "invalid_text",
                    voice_output::OutputError::TsfRequiresServer => "tsf_requires_server",
                    voice_output::OutputError::Unavailable => "unavailable",
                },
            })?;
            record_panel_typing_statistics(&statistics, &text, TypingSource::Voice);
            Ok(())
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
    }
    #[cfg(target_os = "linux")]
    {
        let _ = &store;
        let target = panel_input_target(&state, window.label())?;
        let typing_statistics = typing_statistics.0.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let result = send_panel_voice_text(&app, &target, &text);
            if result.is_ok() {
                record_panel_typing_statistics(&typing_statistics, &text, TypingSource::Voice);
            }
            result
        })
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    }
    #[cfg(target_os = "macos")]
    return macos_panel_session::submit(app, window, text).await;
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        #[cfg(target_os = "ios")]
        {
            let platform = app
                .try_state::<MobilePlatform<tauri::Wry>>()
                .ok_or(HostActionError {
                    code: "unavailable",
                })?
                .inner()
                .clone();
            platform
                .save_voice_text(&text)
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
            let _ = (window, state, typing_statistics, store);
            Ok(())
        }
        #[cfg(target_os = "android")]
        {
            let platform = app
                .try_state::<AndroidVoicePlatform<tauri::Wry>>()
                .ok_or(HostActionError {
                    code: "unavailable",
                })?
                .inner()
                .clone();
            platform
                .save_voice_text(&text)
                .map_err(|_| HostActionError {
                    code: "unavailable",
                })?;
            let _ = (window, state, typing_statistics, store);
            Ok(())
        }
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            let _ = (app, window, state, typing_statistics, store, text);
            Err(HostActionError {
                code: "unavailable",
            })
        }
    }
}

#[tauri::command]
fn voice_input_language(
    store: tauri::State<'_, std::sync::Arc<PreferencesStore>>,
) -> Result<String, HostActionError> {
    store
        .inner()
        .load()
        .map(|snapshot| {
            let language = snapshot.preferences.voice_input.language;
            if language.is_empty() {
                "zh-CN".to_owned()
            } else {
                language
            }
        })
        .map_err(|_| HostActionError {
            code: "unavailable",
        })
}

fn external_url_is_safe(url: &str) -> bool {
    url.len() <= 4096
        && msime_client_core::is_bounded_text(url, 4096)
        && !url.bytes().any(|byte| {
            byte <= b' '
                || matches!(
                    byte,
                    b'"' | b'\'' | b'`' | b'&' | b'|' | b'<' | b'>' | b'\\'
                )
        })
        && url
            .strip_prefix("https://")
            .is_some_and(|rest| rest.as_bytes().first().is_some_and(|byte| *byte != b'/'))
        && reqwest::Url::parse(url).ok().is_some_and(|parsed| {
            parsed.scheme() == "https"
                && parsed.host_str().is_some_and(|host| !host.is_empty())
                && parsed.username().is_empty()
                && parsed.password().is_none()
        })
}

#[cfg(target_os = "android")]
#[tauri::command]
fn open_external_url(
    url: String,
    account: tauri::State<'_, android_account::AccountState>,
) -> Result<(), HostActionError> {
    if !external_url_is_safe(&url) {
        return Err(HostActionError {
            code: "invalid_url",
        });
    }
    account
        .platform
        .run_mobile_plugin::<()>("openExternalUrl", serde_json::json!({ "url": url }))
        .map_err(|_| HostActionError {
            code: "unavailable",
        })
}

#[cfg(not(target_os = "android"))]
#[tauri::command]
async fn open_external_url(url: String) -> Result<(), HostActionError> {
    tauri::async_runtime::spawn_blocking(move || open_external_url_blocking(&url))
        .await
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
}

#[cfg(not(target_os = "android"))]
fn open_external_url_blocking(url: &str) -> Result<(), HostActionError> {
    if !external_url_is_safe(url) {
        return Err(HostActionError {
            code: "invalid_url",
        });
    }
    launch_external_url(url)
}

/// Hands an https URL the caller has already validated to the default browser. None of the launch paths goes through a shell (`open` and `xdg-open` receive it as one argument, Windows uses ShellExecuteW), which is what lets the Google sign-in pass an authorization URL with `&`-separated query parameters that `external_url_is_safe` refuses for page-supplied links.
#[cfg(not(target_os = "android"))]
fn launch_external_url(url: &str) -> Result<(), HostActionError> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).status();
    #[cfg(target_os = "linux")]
    {
        // Generic-mode xdg-open waits on the browser; a launcher still running
        // after the check has opened the page.
        return linux_process::launch("xdg-open", &[url], std::time::Duration::from_secs(1))
            .then_some(())
            .ok_or(HostActionError {
                code: "unavailable",
            });
    }
    #[cfg(target_os = "windows")]
    {
        return msime_host_windows::open_url(url)
            .then_some(())
            .ok_or(HostActionError {
                code: "unavailable",
            });
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let result: Result<std::process::ExitStatus, std::io::Error> =
        Err(std::io::Error::other("unsupported"));
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    match result {
        Ok(status) if status.success() => Ok(()),
        _ => Err(HostActionError {
            code: "unavailable",
        }),
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
fn open_third_party_licenses(app: tauri::AppHandle) -> Result<(), HostActionError> {
    let notices = app
        .path()
        .resource_dir()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .join("Licenses")
        .join("THIRD_PARTY_NOTICES.txt");
    if !notices.is_file() {
        return Err(HostActionError {
            code: "unavailable",
        });
    }
    let status = std::process::Command::new("open")
        .arg(notices)
        .status()
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?;
    status.success().then_some(()).ok_or(HostActionError {
        code: "unavailable",
    })
}

// The Server launches this shell with MSIME_CLIENT_STATE_DIR and MSIME_CLIENT_HOST_OPTIONS pointing into its state directory. A launch without them, such as the Start Menu settings shortcut, otherwise fell back to this shell's own application directory and opened on a state the Server never reads. Use the Server's directory once the Server has prepared its runtime options there.
#[cfg(target_os = "windows")]
fn windows_server_state_directory() -> Option<PathBuf> {
    msime_host_windows::server_state_directory()
        .filter(|directory| directory.join("runtime-options.json").is_file())
}

// The Server's state root is the options' preferences_directory when one is set, and its state directory otherwise (production_preview_document in server_main.cpp); it is what the Server passes as MSIME_CLIENT_STATE_DIR.
#[cfg(target_os = "windows")]
fn windows_server_preferences_directory() -> Option<PathBuf> {
    let directory = windows_server_state_directory()?;
    let configured = read_runtime_options_bytes(&directory.join("runtime-options.json"))
        .ok()
        .and_then(|options| serde_json::from_slice::<Value>(&options).ok())
        .and_then(|options| {
            options
                .get("preferences_directory")
                .and_then(Value::as_str)
                .map(PathBuf::from)
        })
        .filter(|path| path.is_absolute());
    Some(configured.unwrap_or(directory))
}

#[cfg(target_os = "linux")]
fn linux_runtime_state_directory() -> Result<Option<PathBuf>, String> {
    let Some(options_path) = std::env::var_os("MSIME_CLIENT_HOST_OPTIONS")
        .or_else(|| std::env::var_os("MSIME_IBUS_OPTIONS"))
    else {
        return Ok(None);
    };
    let options_path = PathBuf::from(options_path);
    if !options_path.is_absolute() {
        return Err("Runtime options path must be absolute".into());
    }
    let options = read_runtime_options_bytes(&options_path)
        .map_err(|_| "Cannot read runtime options for shared state".to_owned())?;
    let options: Value = serde_json::from_slice(&options)
        .map_err(|_| "Cannot parse runtime options for shared state".to_owned())?;
    match options.get("preferences_directory") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) if PathBuf::from(value).is_absolute() => {
            Ok(Some(PathBuf::from(value)))
        }
        _ => Err("Runtime preferences directory must be absolute".into()),
    }
}

#[cfg(any(target_os = "ios", test))]
fn ios_host_options_document(
    contents: Option<&str>,
    resources: &std::path::Path,
    state_root: &std::path::Path,
) -> Result<Value, String> {
    match contents {
        Some(contents) => serde_json::from_str(contents)
            .map_err(|_| "Cannot parse prepared HostOptions JSON".to_owned()),
        None => {
            let mut document = serde_json::json!({
                "resources": resources.to_string_lossy(),
                "state_root": state_root.to_string_lossy(),
            });
            // The Cantonese and Zhuyin dictionaries are bundled beside EngineResources; naming them here is what lets host-api run those schemes and the page offer them.
            if let Some(directory) = msime_host_api::installed_language_dictionaries(resources) {
                document["language_dictionaries"] = Value::String(directory);
            }
            Ok(document)
        }
    }
}

#[cfg(any(target_os = "ios", test))]
fn ios_custom_skin_library_root(state_root: &std::path::Path) -> PathBuf {
    state_root
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| state_root.to_path_buf())
}

#[cfg(any(target_os = "ios", test))]
fn ios_community_resource_library_path(state_root: &std::path::Path) -> PathBuf {
    ios_custom_skin_library_root(state_root).join("CommunityLibrary.json")
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn open_system_keyboard_settings(
    state: tauri::State<'_, msime_tauri_mobile_platform::MobilePlatform<tauri::Wry>>,
) -> Result<(), CommandError> {
    let platform = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || platform.open_system_keyboard_settings())
        .await
        .map_err(|_| CommandError {
            code: "system_settings",
        })?
        .map_err(|_| CommandError {
            code: "system_settings",
        })
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn ios_onboarding_status(
    state: tauri::State<'_, msime_tauri_mobile_platform::MobilePlatform<tauri::Wry>>,
) -> Result<bool, CommandError> {
    let platform = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || platform.onboarding_completed())
        .await
        .map_err(|_| CommandError { code: "onboarding" })?
        .map_err(|_| CommandError { code: "onboarding" })
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn ios_onboarding_complete(
    state: tauri::State<'_, msime_tauri_mobile_platform::MobilePlatform<tauri::Wry>>,
) -> Result<(), CommandError> {
    let platform = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || platform.complete_onboarding())
        .await
        .map_err(|_| CommandError { code: "onboarding" })?
        .map_err(|_| CommandError { code: "onboarding" })
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn app_icon_info(
    state: tauri::State<'_, msime_tauri_mobile_platform::MobilePlatform<tauri::Wry>>,
) -> Result<msime_tauri_mobile_platform::AppIconInfo, CommandError> {
    let platform = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || platform.app_icon_info())
        .await
        .map_err(|_| CommandError { code: "app_icon" })?
        .map_err(|_| CommandError { code: "app_icon" })
}

#[cfg(target_os = "ios")]
#[tauri::command]
async fn app_icon_set(
    state: tauri::State<'_, msime_tauri_mobile_platform::MobilePlatform<tauri::Wry>>,
    style: String,
) -> Result<msime_tauri_mobile_platform::AppIconInfo, CommandError> {
    if !msime_tauri_mobile_platform::is_supported_app_icon_style(&style) {
        return Err(CommandError {
            code: "invalid_app_icon",
        });
    }
    let platform = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || platform.set_app_icon(&style))
        .await
        .map_err(|_| CommandError { code: "app_icon" })?
        .map_err(|_| CommandError { code: "app_icon" })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// `--sync-omarchy-theme`, which the Omarchy theme-set hook runs through msime-linux-settings: rewrite the `omarchy` skin from the current Omarchy palette and publish the catalog to the hosts, the way a rescan from the settings page would, then exit without opening a window. The state directory is the one the settings window would open on, so the package lands in the skin root it lists.
#[cfg(target_os = "linux")]
fn sync_omarchy_theme() -> i32 {
    let fail = |message: &str| {
        eprintln!("msime: {message}");
        1
    };
    let absolute = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let Some(path) =
        absolute("MSIME_IBUS_OPTIONS").or_else(|| absolute("MSIME_CLIENT_HOST_OPTIONS"))
    else {
        return fail("no runtime options path; run this through msime-linux-settings");
    };
    let directory = match absolute("MSIME_CLIENT_STATE_DIR") {
        Some(directory) => directory,
        None => match linux_runtime_state_directory() {
            Ok(Some(directory)) => directory,
            Ok(None) => {
                return fail(
                    "the runtime options name no state directory; run msime-linux-setup first",
                )
            }
            Err(error) => return fail(&error),
        },
    };
    let Some(palette) = linux_process::read_text(
        "omarchy-theme-color",
        &["--all"],
        64 * 1024,
        std::time::Duration::from_secs(5),
    ) else {
        return fail("omarchy-theme-color did not answer; is this an Omarchy session?");
    };
    let Some(manifest) =
        shared::omarchy_skin::skin_manifest(&shared::omarchy_skin::parse_resolved_colors(&palette))
    else {
        return fail("the current Omarchy theme has no background or foreground colour");
    };
    let root = directory.join("skins");
    if shared::omarchy_skin::install(&root, &manifest).is_err() {
        return fail("cannot write the Omarchy skin");
    }
    let runtime = RuntimeOptionsState {
        path: Some(path),
        document: Arc::new(Mutex::new(Value::Null)),
        skins: Some(root.clone()),
    };
    match publish_candidate_skin_catalog(&runtime, &msime_client_core::skin::catalog::scan(&root)) {
        Ok(()) => 0,
        Err(_) => fail("cannot publish the skin catalog to the runtime options"),
    }
}

pub fn run() {
    #[cfg(target_os = "linux")]
    if std::env::args_os()
        .skip(1)
        .any(|argument| argument == "--sync-omarchy-theme")
    {
        std::process::exit(sync_omarchy_theme());
    }
    #[cfg(target_os = "macos")]
    let mut keyboard_launch_target = macos_keyboard::startup_panel(requested_surface_route())
        .and_then(|_| msime_host_macos::capture_launch_target());
    let context = tauri::generate_context!();
    #[cfg(target_os = "macos")]
    let context = {
        let mut context = context;
        macos_keyboard::prepare_windows(
            &mut context.config_mut().app.windows,
            requested_surface_route(),
        );
        macos_panel_session::prepare_windows(
            &mut context.config_mut().app.windows,
            requested_surface_route(),
        );
        macos_cloud_clipboard::prepare_windows(
            &mut context.config_mut().app.windows,
            requested_surface_route(),
        );
        macos_cloud_dictionary::prepare_windows(
            &mut context.config_mut().app.windows,
            requested_surface_route(),
        );
        context
    };
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.plugin(tauri_nspanel::init());
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
        let route = second_launch_route(&args);
        let callback_app = app.clone();
        let _ = app.run_on_main_thread(move || activate_desktop_surface(&callback_app, route));
    }));
    // A second settings launch forwards its `--route=` to the running settings window and exits inside the plugin's setup; panel launches never register, so they neither receive nor forward.
    #[cfg(target_os = "macos")]
    let builder = if macos_settings_launch(requested_surface_route()) {
        builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let route = second_launch_route(&args);
            let callback_app = app.clone();
            let _ = app.run_on_main_thread(move || activate_desktop_surface(&callback_app, route));
        }))
    } else {
        builder
    };
    // The 插件 page's import picker. Rust calls it host-side; no capability grants the webview any dialog command, so the page cannot open a dialog or name a path itself.
    #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
    let builder = builder.plugin(tauri_plugin_dialog::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(android_account::init());
    #[cfg(target_os = "ios")]
    let builder = builder.plugin(msime_tauri_mobile_platform::init());
    builder
        .setup(|app| {
            #[cfg(target_os = "macos")]
            macos_account::setup(app.handle())?;
            #[cfg(target_os = "windows")]
            windows_account::setup(app.handle())?;
            #[cfg(target_os = "ios")]
            ios_account::setup(app.handle())?;
            #[cfg(target_os = "macos")]
            app.manage(macos_panel_session::PanelState::from_environment()?);
            #[cfg(target_os = "macos")]
            app.manage(macos_cloud_clipboard::CloudState::from_environment()?);
            #[cfg(target_os = "macos")]
            app.manage(macos_cloud_dictionary::DictionaryState::from_environment()?);
            // Refresh the input method on every start, as the Windows installer registers its TSF DLLs on every install and upgrade. In the background so a slow or failed registration never holds up the window. A first install is left to the user: the window opens as the install window instead, and its button runs the install (`run_first_input_source_install`). Only a packaged app does this: `tauri dev`, `cargo run` and a binary under target/<profile> resolve their resource directory to the cargo output directory, where tauri-build has copied the development input method, and must not replace the developer's installed one. A run with a HostOptions file other than the standard locator (a development host's) and a panel the running input method asked for are skipped too; a settings page the input method opens passes the standard locator and refreshes like any other start.
            #[cfg(target_os = "macos")]
            {
                let startup = Arc::new(InputSourceStartupState::default());
                app.manage(Arc::clone(&startup));
                let standard_options = macos_launch::native_locator_root()
                    .ok()
                    .map(|root| root.join("runtime-options.json"));
                let development_run = std::env::var_os("MSIME_IBUS_OPTIONS").is_some()
                    || macos_input_source::development_options_override(
                        std::env::var_os("MSIME_CLIENT_HOST_OPTIONS").as_deref(),
                        standard_options.as_deref(),
                    );
                let panel_launch = requested_surface_route().and_then(|route| route.panel()).is_some();
                match app.path().resource_dir() {
                    Ok(resource_directory)
                        if !tauri::is_dev()
                            && macos_input_source::is_packaged_resource_directory(
                                &resource_directory,
                            )
                            && !development_run
                            && !panel_launch =>
                    {
                        if macos_input_source::first_install_pending(&resource_directory) {
                            if let Some(main) = app.get_webview_window("main") {
                                shape_first_install_window(&main);
                                startup.first_install_window.store(true, std::sync::atomic::Ordering::Release);
                            }
                        }
                        tauri::async_runtime::spawn_blocking(move || {
                            startup.finish(run_input_source_startup(&resource_directory, true));
                        });
                    }
                    _ => startup.finish(None),
                }
            }
            #[cfg(target_os = "macos")]
            let macos_launch = {
                let options_override = std::env::var_os("MSIME_CLIENT_HOST_OPTIONS")
                    .or_else(|| std::env::var_os("MSIME_IBUS_OPTIONS"));
                let state_override = std::env::var_os("MSIME_CLIENT_STATE_DIR");
                let application_directory = app.path().app_data_dir()?;
                let resources_directory = if options_override.is_none() {
                    Some(app.path().resource_dir()?.join("EngineResources"))
                } else {
                    None
                };
                if options_override.is_none() && state_override.is_none() {
                    if let (Ok(legacy_roots), Some(resources)) = (
                        macos_launch::legacy_native_locator_roots(),
                        resources_directory.as_deref(),
                    ) {
                        if macos_launch::migrate_legacy_application_data(
                            &application_directory,
                            resources,
                            &legacy_roots,
                        )? {
                            // The migrated locator is already present in the canonical directory.
                        }
                    }
                }
                let launch = macos_launch::resolve_with_resources(
                    &application_directory,
                    resources_directory.as_deref(),
                    options_override,
                    state_override,
                )?;
                if launch.publish_native_locator {
                    macos_launch::publish_native_options(&launch.document)?;
                }
                launch
            };
            #[cfg(target_os = "macos")]
            let directory = macos_launch.preferences_directory.clone();
            #[cfg(target_os = "android")]
            let directory = app.path().app_data_dir()?.join("files/bootstrap/state");
            #[cfg(not(any(target_os = "android", target_os = "macos")))]
            let directory = match std::env::var_os("MSIME_CLIENT_STATE_DIR") {
                Some(value) => {
                    let path = std::path::PathBuf::from(value);
                    if !path.is_absolute() {
                        return Err("MSIME_CLIENT_STATE_DIR must be absolute".into());
                    }
                    path
                }
                None => {
                    #[cfg(target_os = "linux")]
                    let runtime_directory = linux_runtime_state_directory()?;
                    #[cfg(target_os = "windows")]
                    let runtime_directory = windows_server_preferences_directory();
                    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
                    let runtime_directory: Option<PathBuf> = None;
                    match runtime_directory {
                        Some(path) => path,
                        None => app.path().app_data_dir()?,
                    }
                }
            };
            #[cfg(target_os = "ios")]
            if directory.file_name() == Some(std::ffi::OsStr::new("MSIME")) {
                if let Some(root) = directory.parent() {
                    let _ = msime_host_api::migrate_apple_clipboard_history(root);
                }
            }
            let mut clipboard =
                ClipboardHistoryStore::open(directory.join("clipboard_history.json"));
            let _ = clipboard.load();
            // After `directory`, not beside the other hosts' account setup
            // above: the Linux session store lives in the shared state
            // directory, so it cannot be built before that directory is
            // resolved.
            #[cfg(target_os = "linux")]
            linux_account::setup(app.handle(), &directory)?;
            let preferences = Arc::new(PreferencesStore::new(&directory));
            let keyboard_skin_trials =
                KeyboardSkinTrialStore::new(&directory, Arc::clone(&preferences));
            #[cfg(target_os = "android")]
            let _ = keyboard_skin_trials.restore_pending();
            // The iOS keyboard extension stores named designs in the App Group
            // root, while the Rust preferences live below App Group/MSIME.
            // Point both hosts at the same bounded library file.
            #[cfg(target_os = "ios")]
            let custom_skin_directory = ios_custom_skin_library_root(&directory);
            #[cfg(not(target_os = "ios"))]
            let custom_skin_directory = directory.clone();
            app.manage(CustomSkinLibraryStore::new(custom_skin_directory));
            #[cfg(target_os = "android")]
            let community_resource_library_path = app
                .path()
                .app_data_dir()?
                .join("files/CommunityLibrary.json");
            // The iOS reply keyboard reads downloads directly from the App
            // Group root. Keep the Tauri community page on that exact file;
            // the Rust preferences and statistics remain below App Group/MSIME.
            #[cfg(target_os = "ios")]
            let community_resource_library_path =
                ios_community_resource_library_path(&directory);
            #[cfg(any(target_os = "android", target_os = "ios"))]
            app.manage(msime_client_core::community::resource_library::CommunityResourceLibraryStore::new(
                community_resource_library_path,
            ));
            app.manage(keyboard_skin_trials);
            let typing_statistics = TypingStatisticsStore::new(&directory);
            #[cfg(target_os = "ios")]
            if directory.file_name() == Some(std::ffi::OsStr::new("MSIME")) {
                if let Some(legacy_directory) = directory.parent() {
                    let _ = typing_statistics.migrate_from(legacy_directory);
                }
            }
            app.manage(TypingStatisticsState(typing_statistics));
            app.manage(DiagnosticLogState(directory.clone()));
            app.manage(notices::NoticesState(directory.clone()));
            // The staging root, not the Engine resource directory inside it: `wordbooks/` is a
            // sibling of `EngineResources/` because `ResourceStore::verify` requires that
            // directory to hold exactly the pinned dictionary artifacts, and one extra entry
            // would break the check whose job is to prove a shipped dictionary is intact. A host
            // that stages no books simply offers the imported ones.
            // The staging root, not the Engine resource directory inside it: `wordbooks/` is a
            // sibling of `EngineResources/` because `ResourceStore::verify` requires that
            // directory to hold exactly the pinned dictionary artifacts, and one extra entry
            // would break the check whose job is to prove a shipped dictionary is intact. A host
            // that stages no books simply offers the imported ones.
            app.manage(VocabularyState(
                directory.clone(),
                app.path()
                    .resource_dir()
                    .unwrap_or_else(|_| directory.clone()),
            ));
            app.manage(SkinDirectoryState(directory.join("skins")));
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            app.manage(desktop_plugins::PluginsState::new(&directory));
            app.manage(UserDirectoryState(directory.join("user")));
            app.manage(preferences.clone());
            let clipboard_state = ClipboardHistoryState(Arc::new(Mutex::new(clipboard)));
            app.manage(ClipboardHistoryState(Arc::clone(&clipboard_state.0)));
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            start_desktop_preferences_monitor(
                app.handle(),
                preferences.clone(),
                Arc::clone(&clipboard_state.0),
                #[cfg(target_os = "linux")]
                directory.join("clipboard_history.json"),
            );
            #[cfg(target_os = "linux")]
            start_linux_clipboard_monitor(Arc::clone(&clipboard_state.0), preferences);
            #[cfg(target_os = "windows")]
            msime_host_windows::start_clipboard_monitor({
                let preferences = preferences.clone();
                move |text| {
                    let _ = preferences.capture_clipboard_text(text);
                }
            });
            app.manage(PanelInputState::default());
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app.manage(panel_input::CloudClipboardInputState::default());
            // Before the settings page paints. The window is declared in tauri.conf.json, so this
            // is the first chance to colour it, and the theme is only knowable once it exists.
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_background_color(Some(chrome_background(main.theme().ok())));
            }
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            {
                let linger = DesktopSettingsLinger::default();
                app.manage(linger.clone());
                // The handler belongs to the window: App has no on_window_event,
                // and the label check this replaces only ever admitted "main".
                if let Some(main) = app.get_webview_window("main") {
                    let window = main.clone();
                    main.on_window_event(move |event| {
                        let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                            return;
                        };
                        if linger.quitting.load(Ordering::Acquire) {
                            return;
                        }
                        api.prevent_close();
                        let generation =
                            linger.generation.fetch_add(1, Ordering::AcqRel) + 1;
                        let _ = window.hide();
                        let app = window.app_handle().clone();
                        let linger = linger.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(10 * 60));
                            let timer_app = app.clone();
                            let _ = app.run_on_main_thread(move || {
                                if linger
                                    .generation
                                    .compare_exchange(
                                        generation,
                                        generation + 1,
                                        Ordering::AcqRel,
                                        Ordering::Acquire,
                                    )
                                    .is_ok()
                                {
                                    linger.quitting.store(true, Ordering::Release);
                                    if let Some(window) = timer_app.get_webview_window("main") {
                                        let _ = window.close();
                                    }
                                }
                            });
                        });
                    });
                }
            }
            // Same condition as `shared::voice::voice_sessions`: the mobile hosts record through
            // their own native plugins and never build this module.
            #[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
            app.manage(voice_sessions::VoiceSessions::default());
            app.manage(voice::local_models::LocalModelInstalls::default());
            #[cfg(target_os = "linux")]
            app.manage(linux_setup::LinuxSetupState::default());
            // Native packaging/installer supplies this verified HostOptions JSON.
            // Webview input never controls resource or state paths.
            #[cfg(target_os = "android")]
            let host_options_path = app
                .path()
                .app_data_dir()?
                .join("files/runtime-options.json");
            #[cfg(target_os = "ios")]
            let host_options_path = directory.join("runtime-options.json");
            #[cfg(target_os = "macos")]
            let host_options_path = macos_launch.options_path;
            #[cfg(not(any(target_os = "android", target_os = "ios", target_os = "macos")))]
            let host_options_path = std::env::var_os("MSIME_CLIENT_HOST_OPTIONS")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| {
                    std::env::var_os("MSIME_IBUS_OPTIONS")
                        .map(PathBuf::from)
                        .filter(|path| path.is_absolute())
                })
                .or_else(|| {
                    let mut candidates =
                        Vec::with_capacity(MAX_RUNTIME_OPTIONS_CANDIDATE_CAPACITY);
                    #[cfg(target_os = "windows")]
                    candidates.extend(
                        windows_server_state_directory()
                            .map(|directory| directory.join("runtime-options.json")),
                    );
                    if let Ok(dir) = app.path().app_data_dir() {
                        candidates.push(dir.join("runtime-options.json"));
                        candidates.push(legacy_app_data_dir(&dir).join("runtime-options.json"));
                    }
                    #[cfg(target_os = "windows")]
                    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                        candidates.push(PathBuf::from(local).join("MSIME-Client/runtime-options.json"));
                    }
                    // Without any prepared file the Linux window opens on the first-run page, which prepares exactly the fixed user locator every Linux frontend reads.
                    #[cfg(target_os = "linux")]
                    let user_locator = linux_setup::user_runtime_options();
                    #[cfg(not(target_os = "linux"))]
                    let user_locator: Option<PathBuf> = None;
                    candidates.extend(user_locator.clone());
                    candidates
                        .into_iter()
                        .find(|path| path.is_file())
                        .or(user_locator)
                })
                .ok_or_else(|| {
                    "MSIME_CLIENT_HOST_OPTIONS or MSIME_IBUS_OPTIONS must point to a prepared HostOptions JSON"
                        .to_string()
                })?;
            let host_document: Value = {
                #[cfg(target_os = "android")]
                {
                    match read_runtime_options_bytes(&host_options_path) {
                        Ok(host_options) => serde_json::from_slice(&host_options)
                            .map_err(|_| "Cannot parse prepared HostOptions JSON".to_string())?,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            // The Tauri shell owns the first-run guide. Before the
                            // native bootstrap publishes HostOptions, keep the
                            // managed states valid while resource-backed commands
                            // correctly fail closed until preparation completes.
                            serde_json::json!({
                                "resources": "",
                                "state_root": app.path().app_data_dir()?.join("files/bootstrap/state"),
                            })
                        }
                        Err(_) => return Err("Cannot read prepared HostOptions JSON".into()),
                    }
                }
                #[cfg(target_os = "ios")]
                {
                    let resources = app.path().resource_dir()?.join("EngineResources");
                    match read_runtime_options_bytes(&host_options_path) {
                        Ok(host_options) => {
                            let host_options = std::str::from_utf8(&host_options)
                                .map_err(|_| "Cannot parse prepared HostOptions JSON".to_string())?;
                            ios_host_options_document(
                                Some(host_options),
                                &resources,
                                &directory,
                            )?
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            ios_host_options_document(None, &resources, &directory)?
                        }
                        Err(_) => return Err("Cannot read prepared HostOptions JSON".into()),
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    macos_launch.document
                }
                #[cfg(target_os = "linux")]
                {
                    match read_runtime_options_bytes(&host_options_path) {
                        Ok(host_options) => serde_json::from_slice(&host_options)
                            .map_err(|_| "Cannot parse prepared HostOptions JSON".to_string())?,
                        // The first-run page prepares this file; until then every resource-backed command fails closed on the empty document, and the snapshot re-reads the file once it exists.
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            serde_json::json!({})
                        }
                        Err(_) => return Err("Cannot read prepared HostOptions JSON".into()),
                    }
                }
                #[cfg(not(any(
                    target_os = "android",
                    target_os = "ios",
                    target_os = "macos",
                    target_os = "linux"
                )))]
                {
                    let host_options = read_runtime_options_bytes(&host_options_path)
                        .map_err(|_| "Cannot read prepared HostOptions JSON".to_string())?;
                    serde_json::from_slice(&host_options)
                        .map_err(|_| "Cannot parse prepared HostOptions JSON".to_string())?
                }
            };
            let runtime_path = std::env::var_os("MSIME_IBUS_OPTIONS")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| Some(host_options_path.clone()));
            app.manage(DictionaryHostOptions {
                #[cfg(any(target_os = "linux", target_os = "android"))]
                path: host_options_path,
                #[cfg(not(any(target_os = "linux", target_os = "android")))]
                document: Arc::new(host_document.clone()),
            });
            app.manage(RuntimeOptionsState {
                path: runtime_path,
                document: Arc::new(Mutex::new(host_document)),
                skins: Some(directory.join("skins")),
            });
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::start_sync(app.handle());
            #[cfg(target_os = "macos")]
            app.manage(DataDirectorySelectionState::default());
            #[cfg(target_os = "linux")]
            app.manage(linux_data_directory::DataDirectorySelectionState::default());
            #[cfg(target_os = "macos")]
            if let Some(surface) = macos_keyboard::startup_panel(requested_surface_route())
                .or_else(|| macos_panel_session::startup_panel_for_launch(requested_surface_route()))
                .or_else(|| macos_cloud_clipboard::startup_panel(requested_surface_route()))
                .or_else(|| macos_cloud_dictionary::startup_panel(requested_surface_route())) {
                panel_window::open_panel_window(
                    app.handle(), surface.label, surface.query, surface.title,
                    f64::from(surface.width), f64::from(surface.height), None,
                ).map_err(|_| "Cannot open requested native panel".to_string())?;
            }
            // Both desktop hosts launch this shell with the panel their menu
            // named; the IBus property menu and the Windows tray menu are the
            // same contract, so the routes stay in one place.
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            if let Some(route) = requested_surface_route() {
                // A settings route targets the main window, which is already
                // showing; only panel surfaces need a window opened here.
                if let Some(surface) = route.panel_for(host_platform()) {
                    let (label, route, title, width, height) = (
                        surface.label,
                        surface.query,
                        surface.title,
                        f64::from(surface.width),
                        f64::from(surface.height),
                    );
                    // The menu process is the panel launcher in this path, so
                    // capture the foreground editor before the new window can
                    // take focus. This is the same handoff used by the
                    // settings-page panel commands.
                    let panel_input = app.state::<PanelInputState>();
                    #[cfg(target_os = "linux")]
                    let position = {
                        remember_opening_panel_target(app.handle(), &panel_input, label);
                        panel_position(&panel_input, label, width, height)
                    };
                    #[cfg(target_os = "windows")]
                    let height = panel_window::windows_panel_height(app.handle(), label, height);
                    #[cfg(target_os = "windows")]
                    let position = {
                        remember_opening_panel_target(app.handle(), &panel_input, label);
                        windows_panel_position(width, height, surface.placement)
                    };
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.hide();
                    }
                    panel_window::open_panel_window(
                        app.handle(), label, route, title, width, height, position,
                    )
                    .map_err(|_| "Cannot open requested panel".to_string())?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            host_capabilities,
            notices::notices_list,
            notices::notice_dismiss,
            list_voice_capture_devices,
            capture_voice_pcm,
            supports_font_catalog,
            initial_settings_page,
            list_font_families,
            resolve_font_families,
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(test, not(target_os = "android"), not(target_os = "linux"))
            ))]
            ai::ai_models,
            #[cfg(target_os = "linux")]
            ai_models,
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(test, not(target_os = "android"), not(target_os = "linux"))
            ))]
            ai::ai_test,
            #[cfg(target_os = "linux")]
            ai_test,
            load_preferences,
            restored_default_preferences,
            #[cfg(not(target_os = "ios"))]
            recover_preferences,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            open_preferences_directory,
            load_custom_skin_library,
            mutate_custom_skin_library,
            load_typing_statistics,
            set_typing_statistics_enabled,
            set_typing_statistics_retention,
            reset_typing_statistics,
            open_typing_statistics_directory,
            open_diagnostic_log_directory,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_plugins::plugin_catalog,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_plugins::import_plugin_pack,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_plugins::remove_plugin_pack,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_plugins::load_plugin_mentions,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_plugins::save_plugin_mentions,
            vocabulary::load_vocabulary_review,
            vocabulary::answer_vocabulary_card,
            vocabulary::set_vocabulary_settings,
            vocabulary::import_vocabulary_wordbook,
            vocabulary::remove_vocabulary_wordbook,
            vocabulary::reset_vocabulary_review,
            save_export,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            mcp_server_status,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            install_mcp_client,
            scan_skin_catalog,
            list_helpcode_schemas,
            resolve_theme,
            read_skin_image,
            read_skin_font,
            read_skin_stylesheet,
            read_skin_toolbar_stylesheet,
            read_custom_translations,
            write_custom_translations,
            open_skin_directory,
            test_api_credential,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::provider_credentials_status,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::save_ai_provider_credential,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::clear_ai_provider_credential,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::save_tencent_provider_credential,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::clear_tencent_provider_credential,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::save_voice_provider_credential,
            #[cfg(target_os = "linux")]
            linux_provider_credentials::clear_voice_provider_credential,
            save_preferences,
            clipboard_history::list_clipboard_history,
            clipboard_history::clear_clipboard_history,
            clipboard_history::remove_clipboard_history,
            clipboard_history::set_clipboard_history_pinned,
            clipboard_history::sync_clipboard_history,
            clipboard_history::copy_text,
            remember_input_target,
            send_key,
            send_text,
            send_voice_text,
            paste_clipboard_text,
            supports_clipboard_paste,
            voice_input_language,
            recognize_handwriting,
            voice::recognize_voice,
            voice::cancel_voice,
            voice::stop_voice,
            voice::local_models::voice_local_models,
            voice::local_models::voice_local_model_install,
            voice::local_models::voice_local_model_cancel,
            voice::local_models::voice_local_model_remove,
            submit_handwriting_candidate,
            open_external_url,
            #[cfg(target_os = "macos")]
            open_third_party_licenses,
            panel_window::open_keyboard_panel,
            panel_window::open_handwriting_panel,
            panel_window::open_emoji_panel,
            panel_window::open_voice_panel,
            panel_window::open_vocabulary_panel,
            panel_window::open_cloud_clipboard_panel,
            panel_window::open_cloud_dictionary_panel,
            panel_window::close_panel,
            dictionary_request,
            cloud_clipboard_request,
            cloud_clipboard_can_send_text,
            cloud_dictionary_request,
            load_emoji_catalog,
            restart_input_method,
            #[cfg(target_os = "macos")]
            install_input_source,
            #[cfg(target_os = "macos")]
            input_source_startup_status,
            #[cfg(target_os = "macos")]
            first_install_window_pending,
            #[cfg(target_os = "macos")]
            run_first_input_source_install,
            #[cfg(target_os = "macos")]
            leave_first_install_window,
            #[cfg(target_os = "macos")]
            open_input_source_settings,
            #[cfg(target_os = "macos")]
            data_directory_status,
            #[cfg(target_os = "macos")]
            pick_data_directory,
            #[cfg(target_os = "macos")]
            move_data_directory,
            #[cfg(target_os = "linux")]
            linux_data_directory::data_directory_status,
            #[cfg(target_os = "linux")]
            linux_data_directory::pick_data_directory,
            #[cfg(target_os = "linux")]
            linux_data_directory::move_data_directory,
            #[cfg(target_os = "macos")]
            load_macos_shuangpin_keymap,
            #[cfg(target_os = "macos")]
            save_macos_shuangpin_keymap,
            #[cfg(target_os = "macos")]
            load_macos_wubi_auto_commit_unique,
            #[cfg(target_os = "macos")]
            save_macos_wubi_auto_commit_unique,
            #[cfg(target_os = "macos")]
            on_device_translation_downloadable_languages,
            #[cfg(target_os = "macos")]
            open_translation_language_settings,
            #[cfg(target_os = "macos")]
            uninstall_input_source,
            #[cfg(target_os = "macos")]
            pick_voice_model_path,
            #[cfg(target_os = "android")]
            android_account::account_status,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_status,
            #[cfg(target_os = "linux")]
            linux_setup::linux_setup_status,
            #[cfg(target_os = "linux")]
            linux_setup::run_linux_setup,
            #[cfg(target_os = "android")]
            android_account::android_open_input_method_settings,
            #[cfg(target_os = "android")]
            android_account::android_open_keyboard_tryout,
            #[cfg(target_os = "android")]
            android_account::android_show_input_method_picker,
            #[cfg(target_os = "android")]
            android_account::android_bootstrap_status,
            #[cfg(target_os = "android")]
            android_account::android_prepare_bootstrap,
            #[cfg(target_os = "android")]
            android_account::ai_models,
            #[cfg(target_os = "android")]
            android_account::ai_test,
            #[cfg(target_os = "android")]
            android_account::account_providers,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_providers,
            #[cfg(target_os = "android")]
            android_account::account_request_code,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_request_code,
            #[cfg(target_os = "android")]
            android_account::account_login,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_login,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_google_login,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_google_cancel,
            #[cfg(target_os = "android")]
            android_account::account_profile,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_profile,
            #[cfg(target_os = "android")]
            android_account::account_chat_models,
            #[cfg(target_os = "android")]
            android_account::account_chat,
            #[cfg(target_os = "android")]
            android_account::account_rename,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_rename,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_avatar,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_choose_avatar,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_remove_avatar,
            #[cfg(target_os = "android")]
            android_account::account_logout,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_logout,
            #[cfg(target_os = "android")]
            android_account::account_delete,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_delete,
            #[cfg(target_os = "android")]
            android_account::account_forget,
            #[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
            desktop_account::account_forget,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_list,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_detail,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_preview,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_install,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_pack_preview,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_add_preview,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_add_license,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_publish,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_rate,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_unpublish,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_set_visibility,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_set_category,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_candidate_skin_community::candidate_skin_community_sync,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_community_report::community_report,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_list,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_detail,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_pack_preview,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_publish,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_install,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_rate,
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            desktop_plugin_community::plugin_community_delete,
            #[cfg(target_os = "android")]
            android_account::app_icon_info,
            #[cfg(target_os = "android")]
            android_account::app_icon_set,
            #[cfg(target_os = "android")]
            android_account::mobile_keyboard_feedback_load,
            #[cfg(target_os = "android")]
            android_account::mobile_keyboard_feedback_save,
            #[cfg(target_os = "android")]
            android_account::mobile_keyboard_feedback_preview,
            #[cfg(target_os = "ios")]
            open_system_keyboard_settings,
            #[cfg(target_os = "ios")]
            ios_onboarding_status,
            #[cfg(target_os = "ios")]
            ios_onboarding_complete,
            #[cfg(target_os = "ios")]
            app_icon_info,
            #[cfg(target_os = "ios")]
            app_icon_set,
            #[cfg(target_os = "ios")]
            ios_account::account_status,
            #[cfg(target_os = "ios")]
            ios_account::account_providers,
            #[cfg(target_os = "ios")]
            ios_account::account_request_code,
            #[cfg(target_os = "ios")]
            ios_account::account_login,
            #[cfg(target_os = "ios")]
            ios_account::account_apple_login,
            #[cfg(target_os = "ios")]
            ios_account::account_profile,
            #[cfg(target_os = "ios")]
            ios_account::account_chat_models,
            #[cfg(target_os = "ios")]
            ios_account::account_chat,
            #[cfg(target_os = "ios")]
            ios_account::ai_models,
            #[cfg(target_os = "ios")]
            ios_account::ai_test,
            #[cfg(target_os = "ios")]
            ios_account::account_rename,
            #[cfg(target_os = "ios")]
            ios_account::account_logout,
            #[cfg(target_os = "ios")]
            ios_account::account_delete,
            #[cfg(target_os = "ios")]
            ios_account::account_forget,
            #[cfg(target_os = "ios")]
            ios_account::account_preferences_schema,
            #[cfg(target_os = "ios")]
            ios_account::account_preferences_load,
            #[cfg(target_os = "ios")]
            ios_account::account_preferences_upload,
            #[cfg(target_os = "ios")]
            ios_account::account_preferences_apply,
            #[cfg(target_os = "ios")]
            ios_account::mobile_keyboard_feedback_load,
            #[cfg(target_os = "ios")]
            ios_account::mobile_keyboard_feedback_save,
            #[cfg(target_os = "ios")]
            ios_account::mobile_keyboard_feedback_preview,
            #[cfg(target_os = "android")]
            android_account::account_preferences_schema,
            #[cfg(target_os = "android")]
            android_account::account_preferences_load,
            #[cfg(target_os = "android")]
            android_account::account_preferences_upload,
            #[cfg(target_os = "android")]
            android_account::account_preferences_apply,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_report,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_list,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_detail,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_download,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_rate,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_publish,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_unpublish,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_set_category,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_skin_finish_trial,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::ai_skin_generate,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::ai_skin_cancel,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_list,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_detail,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_publish,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_apply,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_save,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_rate,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_unpublish,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_store_reply,
            #[cfg(any(target_os = "ios", target_os = "android"))]
            mobile_community::community_resource_remove_reply,
        ])
        .build(context)
        .expect("client application failed")
        .run(move |_app, _event| {
            #[cfg(target_os = "macos")]
            if matches!(_event, tauri::RunEvent::Ready) {
                if let Some(target) = keyboard_launch_target.take() {
                    let _ = msime_host_macos::restore_launch_target(target);
                }
            }
            #[cfg(target_os = "macos")]
            if matches!(_event, tauri::RunEvent::WindowEvent { event: tauri::WindowEvent::Destroyed, .. })
                && (macos_keyboard::startup_panel(requested_surface_route()).is_some()
                    || macos_panel_session::startup_panel_for_launch(requested_surface_route()).is_some()
                    || macos_cloud_clipboard::startup_panel(requested_surface_route()).is_some()
                    || macos_cloud_dictionary::startup_panel(requested_surface_route()).is_some())
                && !_app.webview_windows().values().any(|window| window.is_visible().unwrap_or(true))
            {
                // A panel-only launcher does not leave an invisible settings
                // process behind. Other visible panels keep the process alive.
                _app.exit(0);
            }
        });
}

#[cfg(test)]
mod tests;

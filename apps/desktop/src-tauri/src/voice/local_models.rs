//! On-device speech models for the `local` provider: the settings page lists, downloads, cancels and removes them here, and a starting voice session reads the user's dictionary words from here to pass along as hotwords.
//!
//! Models live in `<app data dir>/voice-models/<id>`, the directory `voice_input.asr_model_path` is set to when the user picks one. Installing is `msime_client_core::voice::local_models`, which downloads into a staging directory, verifies every checksum and renames the finished model into place; this module only runs it off the command thread, forwards its progress as the `voice-local-model-progress` event and keeps the cancellation flag of each running install.

use crate::*;
use msime_client_core::voice::hotwords::Hotword;
use msime_client_core::voice::local_models::{self, LocalModelError, LocalModelStatus};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Emitted with a [`LocalModelProgress`] while an install runs.
pub(crate) const LOCAL_MODEL_PROGRESS_EVENT: &str = "voice-local-model-progress";

/// Catalog ids are short ASCII slugs; anything much longer is not one and is refused before it reaches the map below.
const MAX_MODEL_ID_BYTES: usize = 128;

enum LocalModelOperation {
    Install(Arc<AtomicBool>),
    Remove,
}

/// The operations this process is running, by model id. One install or removal per id at a time.
#[derive(Default)]
pub(crate) struct LocalModelInstalls(Mutex<HashMap<String, LocalModelOperation>>);

impl LocalModelInstalls {
    /// Register an install of `id`, or `None` when one is already running.
    pub(crate) fn begin(&self, id: &str) -> Option<Arc<AtomicBool>> {
        let mut installs = self.0.lock().ok()?;
        if installs.contains_key(id) {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        installs.insert(
            id.to_owned(),
            LocalModelOperation::Install(Arc::clone(&cancel)),
        );
        Some(cancel)
    }

    pub(crate) fn begin_remove(&self, id: &str) -> bool {
        let mut installs = match self.0.lock() {
            Ok(installs) => installs,
            Err(_) => return false,
        };
        if installs.contains_key(id) {
            return false;
        }
        installs.insert(id.to_owned(), LocalModelOperation::Remove);
        true
    }

    pub(crate) fn finish(&self, id: &str) {
        if let Ok(mut installs) = self.0.lock() {
            installs.remove(id);
        }
    }

    /// Ask the install of `id` to stop; it notices between download chunks and fails with `local_model_cancelled`. Whether one was running.
    pub(crate) fn cancel(&self, id: &str) -> bool {
        self.0
            .lock()
            .ok()
            .and_then(|installs| {
                installs.get(id).and_then(|operation| match operation {
                    LocalModelOperation::Install(flag) => {
                        flag.store(true, Ordering::Release);
                        Some(())
                    }
                    LocalModelOperation::Remove => None,
                })
            })
            .is_some()
    }

    /// Whether any install or removal is running. A poisoned lock counts as busy, so a caller that would interrupt the work leaves it alone.
    #[cfg(target_os = "linux")]
    pub(crate) fn any_running(&self) -> bool {
        self.0
            .lock()
            .map(|installs| !installs.is_empty())
            .unwrap_or(true)
    }

    #[cfg(test)]
    pub(crate) fn running(&self, id: &str) -> bool {
        self.0
            .lock()
            .map(|installs| installs.contains_key(id))
            .unwrap_or(true)
    }
}

#[derive(serde::Serialize)]
pub(crate) struct LocalModelsResponse {
    pub(crate) models: Vec<LocalModelStatus>,
    /// The catalog's default model id.
    pub(crate) default: &'static str,
    /// The directory models are installed under.
    pub(crate) root: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct LocalModelProgress {
    pub(crate) id: String,
    /// `download`, `verify`, `extract` or `done`.
    pub(crate) stage: &'static str,
    pub(crate) downloaded: u64,
    pub(crate) total: u64,
}

pub(crate) fn local_model_root(app_data: &Path) -> PathBuf {
    app_data.join("voice-models")
}

fn app_model_root<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<PathBuf, HostActionError> {
    let directory = app.path().app_data_dir().map_err(|_| HostActionError {
        code: "local_model_invalid_root",
    })?;
    Ok(local_model_root(&directory))
}

fn valid_model_id(id: &str) -> Result<(), HostActionError> {
    if !msime_client_core::is_bounded_ascii_identifier(id, MAX_MODEL_ID_BYTES) {
        return Err(HostActionError {
            code: "local_model_unknown",
        });
    }
    Ok(())
}

/// The stable code the settings page shows a message for. The detail some variants carry (a URL, an HTTP status, a file name) is not passed to the page.
pub(crate) fn local_model_error_code(error: &LocalModelError) -> &'static str {
    match error {
        LocalModelError::UnknownModel => "local_model_unknown",
        LocalModelError::InvalidRoot => "local_model_invalid_root",
        LocalModelError::InvalidMirror => "local_model_invalid_mirror",
        LocalModelError::Cancelled => "local_model_cancelled",
        LocalModelError::Network(_) => "local_model_network",
        LocalModelError::HttpStatus(_) => "local_model_http_status",
        LocalModelError::SizeMismatch(_) | LocalModelError::ChecksumMismatch(_) => {
            "local_model_checksum_mismatch"
        }
        LocalModelError::UnsafeArchive(_) | LocalModelError::MissingFile(_) => {
            "local_model_invalid_archive"
        }
        LocalModelError::MissingImportFile(_) => "local_model_import_missing",
        LocalModelError::UnreadableImportFile(_) => "local_model_import_unreadable",
        LocalModelError::Io(_) => "local_model_io",
    }
}

#[tauri::command]
pub(crate) async fn voice_local_models<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<LocalModelsResponse, HostActionError> {
    let root = app_model_root(&app)?;
    tauri::async_runtime::spawn_blocking(move || LocalModelsResponse {
        models: local_models::list(&root),
        default: local_models::default_model_id(),
        root: root.to_string_lossy().into_owned(),
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })
}

/// 读取已保存的 `voice_input.asr_model_mirror`。语音模型和 macOS 资源包下载都用这个镜像前缀，页面上填写的镜像要保存偏好后才生效。
pub(crate) async fn saved_model_mirror(
    store: Arc<PreferencesStore>,
) -> Result<String, HostActionError> {
    tauri::async_runtime::spawn_blocking(move || {
        store
            .load()
            .map(|snapshot| snapshot.preferences.voice_input.asr_model_mirror)
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?
    .map_err(|_| HostActionError {
        code: "unavailable",
    })
}

/// 在 `installs` 里以 `key` 登记一次安装（已有同 key 的安装或删除在跑时返回 `busy`），在阻塞线程上运行 `job`，把它的进度以 `event` 事件、`progress_id` 为 id 发给页面，结束后无论成败都注销登记，错误映射成设置页认识的错误码。
pub(crate) async fn run_install<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    installs: &LocalModelInstalls,
    key: &str,
    event: &'static str,
    progress_id: String,
    job: impl FnOnce(
            &mut dyn FnMut(local_models::InstallProgress),
            &AtomicBool,
        ) -> Result<PathBuf, LocalModelError>
        + Send
        + 'static,
) -> Result<PathBuf, HostActionError> {
    let cancel = installs
        .begin(key)
        .ok_or(HostActionError { code: "busy" })?;
    let worker_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut progress = |update: local_models::InstallProgress| {
            let _ = worker_app.emit(
                event,
                LocalModelProgress {
                    id: progress_id.clone(),
                    stage: update.stage,
                    downloaded: update.downloaded,
                    total: update.total,
                },
            );
        };
        job(&mut progress, &cancel)
    })
    .await;
    installs.finish(key);
    result
        .map_err(|_| HostActionError {
            code: "unavailable",
        })?
        .map_err(|error| HostActionError {
            code: local_model_error_code(&error),
        })
}

/// Download and install one catalog model, resolving to the installed directory. The mirror is the saved `voice_input.asr_model_mirror`, so a mirror typed into the page applies once the preferences are saved.
#[tauri::command]
pub(crate) async fn voice_local_model_install<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    installs: tauri::State<'_, LocalModelInstalls>,
    store: tauri::State<'_, Arc<PreferencesStore>>,
    id: String,
) -> Result<String, HostActionError> {
    valid_model_id(&id)?;
    let root = app_model_root(&app)?;
    let mirror = saved_model_mirror(store.inner().clone()).await?;
    let worker_id = id.clone();
    let path = run_install(
        &app,
        &installs,
        &id,
        LOCAL_MODEL_PROGRESS_EVENT,
        id.clone(),
        move |progress, cancel| local_models::install(&root, &worker_id, &mirror, progress, cancel),
    )
    .await?;
    // On Linux the recording runs in the user's voice service, which on-device recognition needs even when no cloud credential was ever saved, the one other step that enables its socket. `msime-linux-setup` enables it too; this covers a socket an earlier version disabled. Without a user service manager the model is installed all the same.
    #[cfg(target_os = "linux")]
    let _ = tauri::async_runtime::spawn_blocking(
        crate::platform::linux::linux_provider_credentials::enable_voice_service,
    )
    .await;
    Ok(path.to_string_lossy().into_owned())
}

/// 「从文件导入」：用系统的打开对话框让用户多选自己下载好的模型文件，再按目录里的长度和 SHA-256 对上并安装，结束时解析为安装好的目录；用户关掉对话框时为 `None`。文件路径只在宿主这边经过，页面拿不到也给不出路径。和下载共用同一个按 id 的登记，所以取消、互斥、进度事件都一样，进度阶段是 `import`。
#[cfg(any(target_os = "linux", target_os = "windows", target_os = "macos"))]
#[tauri::command]
pub(crate) async fn voice_local_model_import(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    installs: tauri::State<'_, LocalModelInstalls>,
    id: String,
) -> Result<Option<String>, HostActionError> {
    use tauri_plugin_dialog::DialogExt;

    valid_model_id(&id)?;
    let root = app_model_root(&app)?;
    let dialog = app.dialog().file().set_parent(&window);
    // 对话框在主线程上弹出，这个阻塞线程只等结果。不按扩展名过滤：下载工具可能改过名，认文件靠内容。
    let picked = tauri::async_runtime::spawn_blocking(move || {
        dialog
            .set_title("选择下载好的模型文件")
            .blocking_pick_files()
    })
    .await
    .map_err(|_| HostActionError {
        code: "unavailable",
    })?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let files: Vec<PathBuf> = picked
        .into_iter()
        .filter_map(|file| file.into_path().ok())
        .collect();
    let worker_id = id.clone();
    let path = run_install(
        &app,
        &installs,
        &id,
        LOCAL_MODEL_PROGRESS_EVENT,
        id.clone(),
        move |progress, cancel| local_models::import(&root, &worker_id, &files, progress, cancel),
    )
    .await?;
    // 和下载安装一样，Linux 上顺带启用用户级语音服务，见 `voice_local_model_install`。
    #[cfg(target_os = "linux")]
    let _ = tauri::async_runtime::spawn_blocking(
        crate::platform::linux::linux_provider_credentials::enable_voice_service,
    )
    .await;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// Stop a running install. Resolves to whether one was running; the install command itself then fails with `local_model_cancelled`.
#[tauri::command]
pub(crate) fn voice_local_model_cancel(
    installs: tauri::State<'_, LocalModelInstalls>,
    id: String,
) -> Result<bool, HostActionError> {
    valid_model_id(&id)?;
    Ok(installs.cancel(&id))
}

/// Delete an installed model. A model still being installed is `busy`: cancel it first.
#[tauri::command]
pub(crate) async fn voice_local_model_remove<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    installs: tauri::State<'_, LocalModelInstalls>,
    id: String,
) -> Result<(), HostActionError> {
    valid_model_id(&id)?;
    let root = app_model_root(&app)?;
    if !installs.begin_remove(&id) {
        return Err(HostActionError { code: "busy" });
    }
    let worker_id = id.clone();
    let result =
        match tauri::async_runtime::spawn_blocking(move || local_models::remove(&root, &worker_id))
            .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(HostActionError {
                code: local_model_error_code(&error),
            }),
            Err(_) => Err(HostActionError {
                code: "unavailable",
            }),
        };
    installs.finish(&id);
    result
}

/// Rows read per dictionary page, the most one list request accepts.
#[cfg(unix)]
const HOTWORD_PAGE: usize = 1_000;
/// Enough rows that the heaviest words can be picked even from a large dictionary, without reading the whole store for every voice session. Same bound as `msime_client_voice_hotwords`.
#[cfg(unix)]
const HOTWORD_MAX_ROWS: usize = 5_000;

/// Hotwords for an on-device session from the user's own pinyin dictionary words, heaviest first, at most `limit`.
///
/// The same selection `msime_client_voice_hotwords` makes, read through the list route this host's dictionary page uses. A dictionary that cannot be read right now (maintenance holds it, the store is missing) gives the words read so far, possibly none: recognition without hotwords is still recognition.
#[cfg(unix)]
pub(crate) fn dictionary_hotwords(options: &Value, limit: usize) -> Vec<Hotword> {
    dictionary_hotwords_with(limit, |action| list_dictionary_page(options, action))
}

#[cfg(unix)]
pub(crate) fn dictionary_hotwords_with(
    limit: usize,
    mut list: impl FnMut(&Value) -> Option<Value>,
) -> Vec<Hotword> {
    msime_client_core::voice::hotwords::hotwords_from_dictionary_pages(
        limit,
        HOTWORD_PAGE,
        HOTWORD_MAX_ROWS,
        |offset, page_size| {
            let action = serde_json::json!({
                "operation": "list",
                "offset": offset,
                "limit": page_size,
                "kind": "pinyin",
                "user_only": true,
            });
            Ok::<_, ()>(list(&action).map(|page| {
                let source_entries = page["entries"]
                    .as_array()
                    .map(|entries| entries.as_slice())
                    .unwrap_or(&[]);
                let mut entries = Vec::with_capacity(source_entries.len());
                entries.extend(source_entries.iter().filter_map(|entry| {
                    Some((
                        entry["value"].as_str()?.to_owned(),
                        entry["key"].as_str()?.to_owned(),
                        entry["weight"].as_i64().unwrap_or(0),
                    ))
                }));
                msime_client_core::voice::hotwords::DictionaryHotwordPage {
                    entries,
                    has_more: page["has_more"].as_bool() == Some(true),
                }
            }))
        },
    )
    .unwrap_or_default()
}

/// One list page through the same store the dictionary page reads on this host. Listing never needs the input sessions released, so none of the maintenance handshakes `dictionary_request` performs for edits apply.
#[cfg(unix)]
fn list_dictionary_page(options: &Value, action: &Value) -> Option<Value> {
    let request = serde_json::json!({ "options": options, "action": action });
    #[cfg(target_os = "ios")]
    {
        crate::ios_personal_dictionary_request(&request).ok()
    }
    #[cfg(target_os = "android")]
    {
        let bytes = serde_json::to_vec(&request).ok()?;
        msime_host_api::personal_dictionary_request_json(&bytes).ok()
    }
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    {
        let bytes = serde_json::to_vec(&request).ok()?;
        msime_host_api::dictionary_request_json(&bytes).ok()
    }
}

/// Hotwords for a `local` session starting now, from the dictionary this host's settings page edits.
#[cfg(unix)]
pub(crate) fn session_hotwords(dictionary: &DictionaryHostOptions) -> Vec<Hotword> {
    match dictionary.snapshot() {
        Ok(options) => dictionary_hotwords(
            &options,
            msime_client_core::voice::hotwords::DEFAULT_HOTWORD_LIMIT,
        ),
        Err(_) => Vec::new(),
    }
}

/// Add `hotwords` to the provider options as the `voice_hotwords` option while the serialised options stay within `budget` bytes, heaviest first.
///
/// The provider takes only boolean and string options, so the words go the way the Linux IBus and Fcitx5 hosts send them: one `text<TAB>pinyin` line per word in a single string. The provider socket refuses a whole request over 16 KiB, and the options already carry up to 8 KiB of polishing prompt, so the list is cut to what fits rather than risking the recording. Words carrying a tab or a line break would break the packing and are skipped.
#[cfg(all(unix, not(any(target_os = "ios", target_os = "android"))))]
pub(crate) fn add_hotwords_within(options: &mut Value, hotwords: &[Hotword], budget: usize) {
    let Some(object) = options.as_object_mut() else {
        return;
    };
    // `"voice_hotwords":""` plus the comma that joins it to the previous key.
    let mut size = serde_json::to_string(&*object).map_or(usize::MAX, |text| text.len())
        + r#","voice_hotwords":"""#.len();
    let mut packed = String::with_capacity(budget.saturating_sub(size));
    for hotword in hotwords {
        let breaks = |value: &str| value.contains(['\t', '\r', '\n']);
        if hotword.text.is_empty() || breaks(&hotword.text) || breaks(&hotword.pinyin) {
            continue;
        }
        let separator = if packed.is_empty() { "" } else { "\n" };
        let line = format!("{separator}{}\t{}", hotword.text, hotword.pinyin);
        // The line's encoded size inside a JSON string, without the quotes.
        let added = serde_json::to_string(&line).map_or(usize::MAX, |text| text.len() - 2);
        if size.saturating_add(added) > budget {
            break;
        }
        size += added;
        packed.push_str(&line);
    }
    if !packed.is_empty() {
        object.insert("voice_hotwords".to_owned(), Value::String(packed));
    }
}

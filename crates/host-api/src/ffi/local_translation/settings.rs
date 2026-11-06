//! 原生窗口共享的偏好读写，仅返回本地翻译字段，不暴露服务凭据。

use super::*;
use msime_client_core::local_translation::{save_selection, LocalTranslationPreferences};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsRequest {
    directory: PathBuf,
    update: Option<Update>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    expected_revision: u64,
    enabled: bool,
    target_language: String,
}

/// 读取或按修订号保存原生本地翻译设置；会读写磁盘，只能在后台线程调用。
/// # Safety
/// `request` 必须指向 `length` 个可读字节，JSON 不超过 16 KiB。
#[no_mangle]
pub unsafe extern "C" fn msime_client_local_translation_settings(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| unsafe {
        with_bounded_bytes(
            request,
            length,
            16_384,
            "local_translation_invalid_input",
            |bytes| {
                let request: SettingsRequest =
                    serde_json::from_slice(bytes).map_err(|_| "local_translation_invalid_input")?;
                if !request.directory.is_absolute() {
                    return Err("local_translation_invalid_input".into());
                }
                let store = PreferencesStore::new(request.directory);
                let snapshot = match request.update {
                    None => store.load(),
                    Some(update) => save_selection(
                        &store,
                        update.expected_revision,
                        LocalTranslationPreferences {
                            enabled: update.enabled,
                            target_language: update.target_language,
                        },
                    ),
                }
                .map_err(|error| error.to_string())?;
                Ok(
                    json!({"revision": snapshot.revision, "local_translation": snapshot.preferences.local_translation}),
                )
            },
        )
    })
}

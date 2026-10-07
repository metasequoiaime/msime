//! 移动端设置页用到的本地数据：无编码常用语、命名词库、诊断包，以及账号设置同步文档的导出与应用。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. 每个函数都读写文件，宿主在工作线程调用。

use crate::*;
use msime_client_core::account::settings_sync::{
    android_local_settings, apply_android_settings, custom_keyboard_skins, export_android_settings,
    insert_android_local_settings, insert_custom_keyboard_skins, HostKeyboardFeedback,
    CUSTOM_KEYBOARD_SKINS,
};
use msime_client_core::account::{
    merge_account_preferences, AccountPreferenceSchema, AccountPreferenceValue, AccountPreferences,
};
use msime_client_core::common_phrases::{CommonPhrasesAction, CommonPhrasesStore};
use msime_client_core::diagnostics::{build_bundle, DiagnosticBundleRequest};
use msime_client_core::dictionary::collections::{
    DictionaryCollectionsAction, DictionaryCollectionsStore, HanReadings,
};
use msime_client_core::dictionary::personal::PersonalDictionaryStore;
use msime_client_core::edition::{
    filter_downloaded_account_settings, filter_uploaded_account_settings,
};

/// 常用语请求的字节上限：安装一个社区短语包时请求里带着整个资源（200 条，每条最多 2000 个 UTF-16 单元）。
const COMMON_PHRASES_REQUEST_LIMIT: usize = 4 * 1024 * 1024;
/// 命名词库请求的字节上限：导入文本最多 16 MiB，外加请求本身。
const DICTIONARY_COLLECTIONS_REQUEST_LIMIT: usize = 17 * 1024 * 1024;
/// 诊断包请求的字节上限：只有几个路径和开关。
const DIAGNOSTIC_BUNDLE_REQUEST_LIMIT: usize = 64 * 1024;
/// 设置同步请求的字节上限：云端文档、字段表和自定义键盘皮肤库（最多 768 KiB）各一份。
const ACCOUNT_SETTINGS_REQUEST_LIMIT: usize = 4 * 1024 * 1024;

fn absolute_directory(path: &str) -> Result<&Path, String> {
    let path = Path::new(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err("directory must be absolute".into())
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommonPhrasesRequest {
    directory: String,
    action: CommonPhrasesAction,
}

/// 无编码常用语。请求 `{directory: 偏好目录的绝对路径, action: {operation, ...}}`，操作见 client-core `common_phrases`。成功时返回整份文档 `{phrases, packs, skipped?}`；失败时 `error` 是稳定错误码（`common_phrases_*`）。
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_common_phrases(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the caller contract above.
        unsafe {
            super::with_bounded_bytes(
                request,
                length,
                COMMON_PHRASES_REQUEST_LIMIT,
                "invalid common phrases buffer",
                |bytes| {
                    let request: CommonPhrasesRequest = serde_json::from_slice(bytes)
                        .map_err(|_| "common_phrases_invalid".to_owned())?;
                    let directory = absolute_directory(&request.directory)?;
                    let outcome = CommonPhrasesStore::new(directory)
                        .perform(request.action)
                        .map_err(|error| error.code().to_owned())?;
                    serde_json::to_value(outcome).map_err(|error| error.to_string())
                },
            )
        }
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DictionaryCollectionsRequest {
    options: HostOptions,
    action: DictionaryCollectionsAction,
}

/// `hans` 导入的读音，来自 Engine 的汉字转拼音。
struct EngineReadings<'a>(&'a EngineOptions);

impl HanReadings for EngineReadings<'_> {
    fn pinyin(&self, word: &str) -> Option<String> {
        let key = msime_engine::host::hanzi_to_pinyin(self.0, word);
        (!key.is_empty()).then_some(key)
    }
}

/// 命名词库。请求 `{options: 宿主选项（须带 preferences_directory）, action: {operation, ...}}`，操作见 client-core `dictionary::collections`。词条经个人词库队列（`<preferences_directory>/PersonalDictionary`）送进 Engine，键盘下次同步时应用。成功时返回集合视图；失败时 `error` 是稳定错误码（`collections_*`、`builtin_locked`、`unsupported_format` 等）。
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_collections(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the caller contract above.
        unsafe {
            super::with_bounded_bytes(
                request,
                length,
                DICTIONARY_COLLECTIONS_REQUEST_LIMIT,
                "invalid dictionary collections buffer",
                |bytes| {
                    let request: DictionaryCollectionsRequest = serde_json::from_slice(bytes)
                        .map_err(|_| "collections_invalid".to_owned())?;
                    if request.options.api_version != 1 {
                        return Err("unsupported host API version".into());
                    }
                    request
                        .options
                        .preferences
                        .validate()
                        .map_err(|_| "collections_invalid".to_owned())?;
                    let directory = request
                        .options
                        .preferences_directory
                        .clone()
                        .filter(|path| Path::new(path).is_absolute())
                        .ok_or("personal dictionary shared directory unavailable")?;
                    let options = request.options.into_engine_options();
                    let directory = Path::new(&directory);
                    let store = DictionaryCollectionsStore::new(
                        directory,
                        PersonalDictionaryStore::new(directory.join("PersonalDictionary")),
                    );
                    let readings = EngineReadings(&options);
                    let view = store
                        .perform(request.action, Some(&readings))
                        .map_err(|error| error.code().to_owned())?;
                    serde_json::to_value(view).map_err(|error| error.to_string())
                },
            )
        }
    })
}

/// 诊断包。请求 `{state_root: 偏好目录, include: {crash_logs, performance_logs, input_events, config_snapshot}, sources: {crash_logs, performance_logs, input_events: 绝对路径|null}, destination: 绝对路径|null}`。输入事件和性能记录逐行按白名单（只许 `t_ms`、`kind` 枚举、`duration_ms`）校验，不合规的行丢弃并计数；配置快照里的凭据换成 `"<redacted>"`。有 `destination` 时写出 zip，返回 `{path, bytes, counts}`；没有时返回 `{counts, sections}`，`sections` 就是上传 MCP 快照的那个对象。失败时 `error` 是 `diagnostics_*` 错误码。
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_diagnostic_bundle(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the caller contract above.
        unsafe {
            super::with_bounded_bytes(
                request,
                length,
                DIAGNOSTIC_BUNDLE_REQUEST_LIMIT,
                "invalid diagnostic bundle buffer",
                |bytes| {
                    let request: DiagnosticBundleRequest = serde_json::from_slice(bytes)
                        .map_err(|_| "diagnostics_invalid".to_owned())?;
                    let bundle = build_bundle(&request).map_err(|error| error.code().to_owned())?;
                    serde_json::to_value(bundle).map_err(|error| error.to_string())
                },
            )
        }
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountSettingsExportRequest {
    preferences_directory: String,
    #[serde(default)]
    feedback: Option<HostKeyboardFeedback>,
    #[serde(default)]
    custom_keyboard_skins: Option<String>,
    /// Android 本地设置文件里参与同步的值，按同步键给出。
    #[serde(default)]
    android_local: Option<std::collections::BTreeMap<String, AccountPreferenceValue>>,
    #[serde(default)]
    schema: Option<AccountPreferenceSchema>,
    #[serde(default)]
    cloud: Option<AccountPreferences>,
}

/// 把本机设置导出成账号设置文档的键值。请求 `{preferences_directory, feedback?: {soundEnabled, hapticsEnabled, hapticStrength}|null, custom_keyboard_skins?: JSON 数组字符串|null, android_local?: {同步键: 值}|null, schema?: 服务端字段表|null, cloud?: 云端文档 {revision, settings}|null}`。`android_local` 是 Android 本地设置文件里参与同步的值（应用主题、单手、按键细节、工具栏的常用语/输入方式/隐藏、手写、离线语音），不在表里或取值不合规的键不导出。结果 `{settings, merged?}`：`settings` 已按本机版本过滤，带 `schema` 时只留字段表收录的键；同时带 `schema` 和 `cloud` 时 `merged` 是合并后的整份文档，宿主拿它按 `revision` 做 CAS 上传。凭据和诊断日志永远不导出；隐私模式、开发者选项和语音数据贡献只在 Android 本地设置里，也不在可同步的键里。
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_settings_export(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the caller contract above.
        unsafe {
            super::with_bounded_bytes(
                request,
                length,
                ACCOUNT_SETTINGS_REQUEST_LIMIT,
                "invalid account settings buffer",
                |bytes| {
                    let request: AccountSettingsExportRequest = serde_json::from_slice(bytes)
                        .map_err(|_| "invalid account settings request".to_owned())?;
                    let store =
                        PreferencesStore::new(absolute_directory(&request.preferences_directory)?);
                    let snapshot = store.load().map_err(|error| error.to_string())?;
                    let mut settings =
                        export_android_settings(&snapshot.preferences, request.feedback.as_ref())
                            .map_err(|error| error.to_string())?;
                    if let Some(library) = &request.custom_keyboard_skins {
                        insert_custom_keyboard_skins(&mut settings, library);
                    }
                    if let Some(local) = &request.android_local {
                        insert_android_local_settings(&mut settings, local);
                    }
                    filter_uploaded_account_settings(Some(store.edition()), &mut settings);
                    if let Some(schema) = &request.schema {
                        settings.retain(|key, _| schema.fields.contains_key(key));
                    }
                    let merged = match (&request.schema, &request.cloud) {
                        (Some(schema), Some(cloud)) => Some(
                            merge_account_preferences(cloud, &settings, schema)
                                .map_err(|error| error.to_string())?,
                        ),
                        _ => None,
                    };
                    let mut value = json!({ "settings": settings });
                    if let Some(merged) = merged {
                        value["merged"] =
                            serde_json::to_value(merged).map_err(|error| error.to_string())?;
                    }
                    Ok(value)
                },
            )
        }
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountSettingsApplyRequest {
    preferences_directory: String,
    cloud: AccountPreferences,
    schema: AccountPreferenceSchema,
    #[serde(default)]
    feedback: Option<HostKeyboardFeedback>,
}

/// 把云端设置文档应用到本机偏好并保存。请求 `{preferences_directory, cloud: {revision, settings}, schema, feedback?: 宿主当前的按键反馈|null}`；文档里有按键反馈的键而没有传 `feedback` 时失败。取值不认识或超出本机范围的键只跳过它自己。结果 `{preferences: 保存后的快照, feedback: 应用后的按键反馈|null（宿主写回自己的存储）, custom_keyboard_skins: 云端的皮肤库 JSON|null（宿主合并进自己的皮肤库）, android_local: {同步键: 值}（宿主写回本地设置文件）, skipped: [键名]}`。偏好在锁内按读到的修订号比较并交换写回，期间被别处改过时以 `preferences changed; reload before saving` 失败。
/// # Safety
/// `request` must point to `length` readable bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_settings_apply(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: guaranteed by the caller contract above.
        unsafe {
            super::with_bounded_bytes(
                request,
                length,
                ACCOUNT_SETTINGS_REQUEST_LIMIT,
                "invalid account settings buffer",
                |bytes| {
                    let mut request: AccountSettingsApplyRequest = serde_json::from_slice(bytes)
                        .map_err(|_| "invalid account settings request".to_owned())?;
                    let store =
                        PreferencesStore::new(absolute_directory(&request.preferences_directory)?);
                    filter_downloaded_account_settings(
                        Some(store.edition()),
                        &mut request.cloud.settings,
                    );
                    let local = store.load().map_err(|error| error.to_string())?;
                    let mut applied = apply_android_settings(
                        &local.preferences,
                        &request.cloud,
                        &request.schema,
                        request.feedback,
                    )
                    .map_err(|error| error.to_string())?;
                    let skins = match custom_keyboard_skins(&request.cloud, &request.schema)
                        .map_err(|error| error.to_string())?
                    {
                        Some(Ok(library)) => Some(library),
                        Some(Err(())) => {
                            applied.skipped.push(CUSTOM_KEYBOARD_SKINS.to_owned());
                            applied.skipped.sort();
                            None
                        }
                        None => None,
                    };
                    let (android_local, local_skipped) =
                        android_local_settings(&request.cloud, &request.schema)
                            .map_err(|error| error.to_string())?;
                    if !local_skipped.is_empty() {
                        applied.skipped.extend(local_skipped);
                        applied.skipped.sort();
                    }
                    let snapshot = if applied.preferences == local.preferences {
                        local
                    } else {
                        store
                            .save(local.revision, applied.preferences)
                            .map_err(|error| error.to_string())?
                    };
                    Ok(json!({
                        "preferences": serde_json::to_value(snapshot).map_err(|error| error.to_string())?,
                        "feedback": applied.feedback,
                        "custom_keyboard_skins": skins,
                        "android_local": android_local,
                        "skipped": applied.skipped,
                    }))
                },
            )
        }
    })
}

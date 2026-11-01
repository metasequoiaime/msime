//! Host configuration, preferences, the skin catalog and clipboard history.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;
use msime_client_core::is_bounded_text;

/// Candidate and commit text normally stays far below this bound. Keep the direct conversion ABI
/// bounded as well so a malformed native length cannot make it scan an unbounded buffer or allocate
/// an arbitrarily large converted string.
const MAX_TRADITIONAL_CONVERSION_BYTES: usize = 1 << 20;

/// 规范化快捷符号列表，默认值与偏好校验都由 client-core 持有。
/// # Safety
/// `value` 指向 `length` 字节的 JSON `{symbols: string[]}`，最多 64 KiB；结果用 string_free 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_touch_quick_symbols(
    value: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if value.is_null() || length > 64 * 1024 {
            return Err("invalid_touch_symbols".into());
        }
        // 安全：调用方保证缓冲区可读，长度已检查。
        let bytes = unsafe { std::slice::from_raw_parts(value, length) };
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Request {
            symbols: Vec<String>,
        }
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid_touch_symbols")?;
        Ok(json!(
            msime_client_core::preferences::normalize_touch_quick_symbols(&request.symbols)
        ))
    })
}

fn clipboard_history_limit(directory: &std::path::Path) -> usize {
    msime_client_core::preferences::PreferencesStore::new(directory)
        .load_for_runtime()
        .map(|runtime| runtime.snapshot.preferences.clipboard_history_limit)
        .unwrap_or(msime_client_core::clipboard::DEFAULT_CLIPBOARD_HISTORY_LIMIT)
}

#[no_mangle]
pub extern "C" fn msime_client_abi_version() -> u32 {
    3
}

/// Convert Simplified Chinese text to Traditional Chinese with the shared OpenCC `s2t` tables.
///
/// Returns the converted text itself rather than the standard JSON response: hosts call this for every candidate on a page and every commit, and wrapping each string in a document only to parse it back out would put a JSON round trip on the typing path. Returns null for a null pointer, invalid UTF-8 or an interior NUL, so the caller keeps its own text.
/// # Safety
/// `text` points to `length` readable bytes. The returned string must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_simplified_to_traditional(
    text: *const u8,
    length: usize,
) -> *mut c_char {
    if text.is_null() || length > MAX_TRADITIONAL_CONVERSION_BYTES {
        return std::ptr::null_mut();
    }
    // SAFETY: guaranteed by the documented caller contract.
    let bytes = unsafe { std::slice::from_raw_parts(text, length) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    let converted = msime_client_core::chinese_conversion::simplified_to_traditional(text);
    CString::new(converted).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// 与 `msime_client_simplified_to_traditional` 相同，但按 `variant` 选 OpenCC 配置：0 `s2t`、1 `s2tw`、2 `s2twp`，即偏好 `traditional_chinese_variant` 的三个取值。未知编号和其它非法输入一样返回 null，调用方保留原文。
/// # Safety
/// `text` points to `length` readable bytes. The returned string must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_convert_to_traditional(
    text: *const u8,
    length: usize,
    variant: u32,
) -> *mut c_char {
    let Some(variant) =
        msime_client_core::chinese_conversion::TraditionalVariant::from_code(variant)
    else {
        return std::ptr::null_mut();
    };
    if text.is_null() || length > MAX_TRADITIONAL_CONVERSION_BYTES {
        return std::ptr::null_mut();
    }
    // SAFETY: guaranteed by the documented caller contract.
    let bytes = unsafe { std::slice::from_raw_parts(text, length) };
    let Ok(text) = std::str::from_utf8(bytes) else {
        return std::ptr::null_mut();
    };
    let converted = msime_client_core::chinese_conversion::convert_to_traditional(text, variant);
    CString::new(converted).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Resolve display font families using the same adapter as the shared preview.
/// # Safety
/// `value` points to `length` readable bytes containing a JSON string array.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resolve_font_families(
    value: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if value.is_null() || length > 32 * 1024 {
            return Err("font_family".into());
        }
        // SAFETY: guaranteed by the caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(value, length) };
        let names: Vec<String> = serde_json::from_slice(bytes).map_err(|_| "font_family")?;
        let resolved = system_fonts::resolve_css_families(names).map_err(str::to_owned)?;
        Ok(json!(resolved))
    })
}

/// The installed font family names, from the same catalog the shared app's 刷新字体列表 reads
/// (`list_font_families` in the Tauri shell): sorted, without repeats, each a bounded display name.
/// Fails with `unsupported` on a host without a catalog and `font_catalog` / `font_catalog_limit`
/// when enumeration fails or exceeds its bound. Enumerates the system's fonts, so call it off a
/// UI thread.
#[no_mangle]
pub extern "C" fn msime_client_list_font_families() -> *mut c_char {
    response(|| {
        system_fonts::list()
            .map(|names| json!(names))
            .map_err(str::to_owned)
    })
}

/// Resolve a shared surface route so a native host launches the shared shell by
/// name instead of hardcoding window labels. Returns the canonical route plus
/// the panel label, query and geometry, or an error for an unknown route.
/// # Safety
/// `value` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_parse_surface_route(
    value: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if value.is_null() || length > 1024 {
            return Err("invalid surface route buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(value, length) };
        let text = std::str::from_utf8(bytes).map_err(|_| "invalid surface route encoding")?;
        let route = SurfaceRoute::parse(text).map_err(|e| e.to_string())?;
        let mut document = json!({ "route": route.as_arg(), "surface": route });
        if let Some(panel) = route.panel() {
            document["panel"] = json!({
                "label": panel.label,
                "query": panel.query,
                "title": panel.title,
                "width": panel.width,
                "height": panel.height,
            });
        }
        Ok(document)
    })
}

/// Describe what the named host can do, so the shared UI renders from injected
/// capabilities instead of sniffing the user agent.
/// # Safety
/// `platform` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_host_capabilities(
    platform: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if platform.is_null() || length > 64 {
            return Err("invalid host platform buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(platform, length) };
        let text = std::str::from_utf8(bytes).map_err(|_| "invalid host platform encoding")?;
        let platform = HostPlatform::parse(text).map_err(|e| e.to_string())?;
        serde_json::to_value(HostCapabilities::for_platform(platform)).map_err(|e| e.to_string())
    })
}

/// Verify packaged resources and prepare an isolated new host. No live sessions
/// may use the state root while this runs. Returns a HostOptions object.
/// # Safety
/// `options` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_prepare_host(
    options: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Bootstrap {
        resources: String,
        state_root: String,
        /// 版本 id，缺省是 full；见 `prepare_host_configuration_for_edition`。
        #[serde(default)]
        edition: Option<String>,
        /// 已发布的 runtime-options.json；给了时把这次准备的代次写进去，见 [`publish_prepared_dictionaries`]。
        #[serde(default)]
        runtime_options: Option<String>,
    }
    response(|| {
        if options.is_null() || length > 16384 {
            return Err("invalid bootstrap buffer".into());
        }
        // SAFETY: guaranteed by the caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(options, length) };
        let options: Bootstrap =
            serde_json::from_slice(bytes).map_err(|_| "invalid bootstrap document")?;
        let resources = std::path::Path::new(&options.resources);
        let state = std::path::Path::new(&options.state_root);
        let published = options.runtime_options.as_deref().map(std::path::Path::new);
        if !resources.is_absolute()
            || !state.is_absolute()
            || published.is_some_and(|path| !path.is_absolute())
        {
            return Err("bootstrap paths must be absolute".into());
        }
        let edition = match options.edition.as_deref() {
            None => msime_client_core::edition::Edition::full(),
            Some(id) => msime_client_core::edition::Edition::by_id(id).ok_or("unknown edition")?,
        };
        let document = prepare_host_configuration_for_edition(resources, state, edition)
            .map_err(|e| e.to_string())?;
        let document: Value = serde_json::from_str(&document).map_err(|e| e.to_string())?;
        // 发布失败不挡住宿主启动：会话照样用这份文档，设置窗口和 TIP 留在旧代次上，下次启动再试。
        if let Some(path) = published {
            if let Err(error) = publish_prepared_dictionaries(path, &document) {
                crate::diagnostics::report(
                    "runtime options generation not published",
                    &error.to_string(),
                );
            }
        }
        Ok(document)
    })
}

/// Re-prepare a published runtime options file whose working dictionaries belong to an older resource generation, as after a package upgrade, and record the language dictionaries installed beside the resources (see [`refresh_host_options_with_language_dictionaries`]). Returns whether the file was rewritten. Only the input method host calls this, before creating any session from that file.
/// # Safety
/// `path` points to `length` readable UTF-8 bytes naming an absolute file. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_refresh_host(path: *const u8, length: usize) -> *mut c_char {
    response(|| {
        if path.is_null() || length > 4096 {
            return Err("invalid options path buffer".into());
        }
        // SAFETY: guaranteed by the caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(path, length) };
        let path = std::path::Path::new(
            std::str::from_utf8(bytes).map_err(|_| "invalid options path encoding")?,
        );
        if !path.is_absolute() {
            return Err("options path must be absolute".into());
        }
        refresh_host_options_with_language_dictionaries(path)
            .map(Value::Bool)
            .map_err(|e| e.to_string())
    })
}

/// Register the device's anonymous MSIME account under `directory` (`anonymous-account.json` and `anonymous-session.json`) unless a session is already there. Blocks on the network for up to about a minute: call from a background thread. Value is true once a session exists.
/// # Safety
/// `directory` points to `length` readable UTF-8 bytes naming an absolute directory. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ensure_anonymous_account(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 4096 {
            return Err("invalid account directory buffer".into());
        }
        // SAFETY: guaranteed by the caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = std::path::Path::new(
            std::str::from_utf8(bytes).map_err(|_| "invalid account directory encoding")?,
        );
        if !directory.is_absolute() {
            return Err("account directory must be absolute".into());
        }
        msime_client_services::account::ensure_anonymous_account(directory)
            .map(|()| Value::Bool(true))
            .map_err(|e| e.to_string())
    })
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AccountAccessTokenRequest {
    /// 与 `session_store` 恰好给一个。
    #[serde(default)]
    directory: Option<String>,
    /// `msime_client_session_store_register` 给的 id：登录会话和匿名会话都在宿主注入的存储里。
    #[serde(default)]
    session_store: Option<u64>,
    #[serde(default)]
    rejected_token: Option<String>,
}

/// 输入法进程取账号服务的访问令牌（[`msime_client_services::account::shared_access_token`]）：设置应用登录的账号优先，没有登录时用本机匿名账号。`request` 是 UTF-8 JSON `{"directory": "<绝对路径>", "rejected_token": "<可选，服务端刚以 401 拒绝的令牌>"}`，`directory` 是设置应用和输入法共用的账号目录，Windows 上是 `%LOCALAPPDATA%\<本版本的用户目录>\account`，与 [`msime_client_ensure_anonymous_account`] 的目录相同。成功时 value 是 `{"access_token", "user_id", "anonymous"}`，失败时 error 是账号错误码（例如没有任何会话时的 `account_unauthorized`）。响应里有凭据：不要写进日志，用完交给 `msime_client_string_free`。刷新令牌时会阻塞网络，调用方放在输入线程以外。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 字节，空指针被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_access_token(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > 16_384 {
            return Err("account_invalid".into());
        }
        // SAFETY: 由调用方契约保证。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: AccountAccessTokenRequest =
            serde_json::from_slice(bytes).map_err(|_| "account_invalid")?;
        let rejected = request.rejected_token.as_deref();
        let token = match super::session_store::session_source(
            request.directory.as_deref(),
            false,
            request.session_store,
        )
        .ok_or("account_invalid")?
        {
            super::session_store::Source::Directory(directory) => {
                let directory = std::path::Path::new(directory);
                if !directory.is_absolute() {
                    return Err("account_invalid".into());
                }
                msime_client_services::account::shared_access_token(directory, rejected)
            }
            // 登录的优先，没有时用匿名的，与按目录时相同。
            super::session_store::Source::Store(entry) => {
                msime_client_services::account::shared_access_token_in(
                    &entry.account,
                    &entry.anonymous,
                    rejected,
                    None,
                )
            }
        };
        token
            .map_err(|error| error.code().to_owned())
            .and_then(|token| serde_json::to_value(token).map_err(|_| "account_unavailable".into()))
    })
}

/// The shared preference defaults, as the document a host would have to produce.
///
/// A host that patches one key into a nested preference object needs the rest of
/// that object's fields, because the nested structures require all of them - only
/// the object as a whole is optional. Writing those defaults into a platform host
/// would put a second copy of this contract in another language, so they are
/// published here instead.
#[no_mangle]
pub extern "C" fn msime_client_default_preferences() -> *mut c_char {
    response(|| {
        serde_json::to_value(msime_client_core::preferences::Preferences::default())
            .map_err(|e| e.to_string())
    })
}

/// 触控板设置控件的默认值和范围，宿主只负责展示；结果用 `msime_client_string_free` 释放。
#[no_mangle]
pub extern "C" fn msime_client_text_trackpad_options() -> *mut c_char {
    use msime_client_core::preferences::TextTrackpadPreferences as Trackpad;
    response(|| {
        Ok(json!({
            "defaults": Trackpad::default(),
            "horizontal_dp": {"minimum": Trackpad::HORIZONTAL_MIN_DP, "maximum": Trackpad::HORIZONTAL_MAX_DP},
            "vertical_dp": {"minimum": Trackpad::VERTICAL_MIN_DP, "maximum": Trackpad::VERTICAL_MAX_DP},
        }))
    })
}

/// The transcription provider and the optional rewrite, resolved from a preferences directory.
///
/// The Android keyboard has its own voice entry and never goes through the desktop shell, so the
/// resolution it needs is here rather than in that shell. Contains credentials: never log the
/// response; release with `msime_client_string_free`.
/// # Safety
/// `directory` points to `length` readable UTF-8 bytes naming an absolute path. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_mobile_voice_configuration(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length == 0 || length > 16_384 {
            return Err("invalid preferences directory".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = std::str::from_utf8(bytes).map_err(|_| "invalid preferences directory")?;
        if !std::path::Path::new(directory).is_absolute() {
            return Err("invalid preferences directory".into());
        }
        let store = msime_client_core::preferences::PreferencesStore::new(directory);
        let snapshot = store.load().map_err(|_| "preferences unavailable")?;
        let provider =
            msime_client_providers::voice::provider::mobile_voice_provider_configuration(
                &snapshot.preferences,
            )
            .map(|value| {
                json!({
                    "provider": value.provider,
                    "requestFormat": value.request_format,
                    "endpoint": value.endpoint,
                    "model": value.model,
                    "token": value.token,
                    "headers": value
                        .headers
                        .iter()
                        .map(|header| json!({"name": header.name, "value": header.value}))
                        .collect::<Vec<_>>(),
                    "enableItn": value.enable_itn,
                    "enablePunctuation": value.enable_punctuation,
                    "enableDdc": value.enable_ddc,
                    "boostingTableId": value.boosting_table_id,
                    "modelPath": value.model_path,
                })
            });
        let polish = msime_client_providers::voice::provider::mobile_voice_polish_configuration(
            &snapshot.preferences,
        )
        .map(|value| {
            json!({
                "endpoint": value.endpoint,
                "model": value.model,
                "token": value.token,
                "promptId": value.prompt_id,
                "promptCustom1": value.prompt_custom_1,
                "promptCustom2": value.prompt_custom_2,
                "promptCustom3": value.prompt_custom_3,
            })
        });
        Ok(json!({ "provider": provider, "polish": polish }))
    })
}

/// 本库编译到的宿主平台。主题目录按它列出主题、会话按它截断每页候选数：这些接口不带平台参数，而每个原生宿主链接的都是为自己的平台编出的库。
pub(crate) fn compiled_platform() -> HostPlatform {
    if cfg!(target_os = "ios") {
        HostPlatform::Ios
    } else if cfg!(target_os = "android") {
        HostPlatform::Android
    } else if cfg!(target_env = "ohos") {
        HostPlatform::Harmony
    } else if cfg!(target_os = "macos") {
        HostPlatform::Macos
    } else if cfg!(windows) {
        HostPlatform::Windows
    } else {
        HostPlatform::Linux
    }
}

/// 全局主题选择器：本平台提供的主题 id，按选择器顺序，带标题和调色板，以及默认 id。
///
/// 只列出本库编译到的平台提供的主题（`theme::catalog_for`）：`native` 只在 iOS 的库里出现，其他宿主的选择器因此不必各自过滤。`system`、`native` 和 `custom` 在这里不带调色板：`system` 和 `native` 由宿主自己取色，`custom` 要按用户的 `custom_theme` 解析后才知道。宿主据此画选择器，不保存 id、标题或颜色的副本。
#[no_mangle]
pub extern "C" fn msime_client_theme_catalog() -> *mut c_char {
    response(|| {
        Ok(serde_json::json!({
            "themes": msime_client_services::skin::theme::catalog_for(compiled_platform()),
            "default": msime_client_services::skin::theme::GlobalTheme::default(),
        }))
    })
}

/// Largest resolve request. A custom keyboard design may carry a photo of up to 682,668 base64 bytes, and the rest of the request is small.
const MAX_THEME_REQUEST_BYTES: usize = 1 << 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveThemeRequest {
    global_theme: msime_client_services::skin::theme::GlobalTheme,
    #[serde(default)]
    custom_theme: msime_client_core::preferences::CustomTheme,
    dark: bool,
    layout: msime_client_core::preferences::CandidateLayout,
    skins_directory: Option<String>,
    package: Option<serde_json::Value>,
    /// 宿主确认当前面板贴图已解码且将绘制，才使用贴图原配色；旧宿主缺省使用可读回退。
    #[serde(default)]
    canvas_panel_drawn: bool,
}

/// 应用主题请求里的月份所在的季节；没传月份时按 UTC 月份，月份不在 1..=12 时请求失败。
fn requested_season(
    month: Option<u8>,
    invalid: &str,
) -> Result<msime_client_services::skin::season::Season, String> {
    match month {
        Some(month) => msime_client_services::skin::season::season_for_month(month)
            .ok_or_else(|| invalid.to_owned()),
        None => Ok(msime_client_services::skin::season::current_utc_season()),
    }
}

/// 解析宿主绘制全局主题所用的颜色。
///
/// 请求带着宿主读到的两个偏好字段，所以这里不需要偏好目录，也不做偏好 I/O：`global_theme`（`GlobalTheme::ALL` 的八个 id 之一，本平台目录里没有的 `native` 也接受，解析结果与 `system` 相同、只是 `id` 为 `native`；其他 id，包括已停用的皮肤 id，都以 `invalid theme request` 失败）和 `custom_theme`（可省略，按偏好本身同样严格地校验）。`dark` 是宿主正在绘制的界面当前的明暗模式，`layout`（`horizontal` 或 `vertical`）是它的候选排列：皮肤包只在清单声明的排列和模式下绘制，宿主不另设一道关卡。`dark` 同时决定取哪个槽位：深色模式取 `custom_theme.candidate_skin_dark`，没设时与浅色模式一样取 `custom_theme.candidate_skin`；选中的包只在它清单 `base` 的明暗下绘制（见 `theme::resolve`）。`custom` 主题要用皮肤包时，宿主最多用下面两者之一说明包从哪里来：`skins_directory`（皮肤根目录的绝对路径；包按 `msime_client_skin_package` 的规则加载和校验；直接扫描皮肤的宿主及已准备 canvas 面板的 Fcitx5 使用）或 `package`（已发布的 `candidate_skin_catalog` 中的一项，只有 Linux 宿主读它，宿主要传当前模式槽位指名的那一项）。`package` 不是这样的一项时（包括 `msime_client_skin_catalog` 给出的 `SkinSummary`）调用失败：那是宿主的缺陷，照读会悄悄丢掉它声明的模式和选中条。根目录里没有、磁盘上无效、或不是当前槽位指名的那个包会被略过而不是让调用失败：主题照常在它的底上解析。
/// `canvas_panel_drawn` 缺省为 false。只有已成功解码且实际绘制当前 canvas 面板的宿主才传 true；共享层在包配色上套用原有取色器规则，不能由宿主另行覆盖最终配色。此标志只作用于 `skins_directory` 读取的含面板包。Linux 公共 catalog 的配色仍是可读回退；Fcitx5 准备好面板后改用根目录协商。
/// `candidate_details` 可选提供规范化的预编辑文字／光标／背景／分隔线、选中条、释义及词性／义项分隔符的普通与选中色、菜单四槽位；宿主忽略不支持的键。文字取色器同时覆盖预编辑文字，宿主不从原始包重新取色。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，不接受空指针。
/// 返回的响应必须通过 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_resolve_theme(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > MAX_THEME_REQUEST_BYTES {
            return Err("invalid theme request".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: ResolveThemeRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid theme request")?;
        request
            .custom_theme
            .validate()
            .map_err(|error| error.to_string())?;
        if request.skins_directory.is_some() && request.package.is_some() {
            return Err("theme request names both a skins directory and a package".into());
        }
        if request
            .skins_directory
            .as_deref()
            .is_some_and(|directory| !Path::new(directory).is_absolute())
        {
            return Err("skin directory must be absolute".into());
        }
        let theme = request.global_theme;
        // 按当前明暗取槽位：深色模式先取 `candidate_skin_dark`。
        let wanted = request
            .custom_theme
            .candidate_skin_for(request.dark)
            .filter(|_| theme == msime_client_services::skin::theme::GlobalTheme::Custom);
        let entry = request
            .package
            .map(msime_client_services::skin::theme::ThemePackage::from_host_catalog_entry)
            .transpose()?;
        let package = match (wanted, request.skins_directory, entry) {
            (Some(id), Some(directory), _) => {
                msime_client_services::skin::catalog::load_package(&directory, id)
                    .ok()
                    .map(|mut summary| {
                        if request.canvas_panel_drawn {
                            if let Some(colors) = summary.panel_candidate.take() {
                                summary.candidate = colors;
                            }
                        }
                        msime_client_services::skin::theme::ThemePackage::from(&summary)
                    })
            }
            (Some(_), None, entry) => entry,
            _ => None,
        };
        let resolved = msime_client_services::skin::theme::resolve(
            theme,
            &request.custom_theme,
            request.dark,
            request.layout,
            package.as_ref(),
        );
        serde_json::to_value(resolved).map_err(|error| error.to_string())
    })
}

/// 应用主题的选择器：每个应用主题的 ID、标题、固定的季节和浅色、深色的颜色，以及默认 ID。
///
/// `siji` 的 `season` 为 `null`、`seasonal` 为真，它的颜色固定画秋杉，所以这份目录不随时钟变化；宿主画当季颜色时调 `msime_client_resolve_app_theme`。宿主不保存 ID、标题或色值的副本。
#[no_mangle]
pub extern "C" fn msime_client_app_theme_catalog() -> *mut c_char {
    response(|| {
        Ok(json!({
            "app_themes": msime_client_services::skin::app_theme::catalog(),
            "default": msime_client_services::skin::app_theme::AppTheme::default(),
        }))
    })
}

/// 解析应用主题请求的上限。请求只有三个短字段。
const MAX_APP_THEME_REQUEST_BYTES: usize = 4096;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveAppThemeRequest {
    app_theme: msime_client_services::skin::app_theme::AppTheme,
    #[serde(default)]
    month: Option<u8>,
    dark: bool,
}

/// 应用主题在宿主当前月份和明暗模式下的颜色：`{id, season, accent, accent_soft, on_accent, background, card, hair, candidate_accent}`。纯计算，不读写文件。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_resolve_app_theme(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > MAX_APP_THEME_REQUEST_BYTES {
            return Err("invalid app theme request".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: ResolveAppThemeRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid app theme request")?;
        let season = requested_season(request.month, "invalid app theme request")?;
        let resolved = msime_client_services::skin::app_theme::resolve_app_theme(
            request.app_theme,
            season,
            request.dark,
        );
        serde_json::to_value(resolved).map_err(|error| error.to_string())
    })
}

/// 「重置所有设置」：在偏好锁里读出当前文档，换成本存储所属版本的默认偏好（服务凭据和 `fuzzy_pinyin.seeded` 保留，见 `Preferences::restored_to_defaults_for`），再按 `expected_revision` 比较并交换写回，返回新的快照。修订号不符时以 `preferences changed; reload before saving` 失败，什么也不写。词库、统计和剪贴板历史不受影响；默认关闭剪贴板历史时与保存偏好一样清空已存的历史。
/// # Safety
/// `directory` must point to `length` readable bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_restore_default_preferences(
    directory: *const u8,
    length: usize,
    expected_revision: u64,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = super::parse_absolute_path(
            bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let store = PreferencesStore::new(directory);
        let restored = store
            .restore_defaults(expected_revision)
            .map_err(|e| e.to_string())?;
        if !restored.preferences.clipboard_history {
            store
                .clear_disabled_clipboard_history()
                .map_err(|e| e.to_string())?;
        }
        serde_json::to_value(restored).map_err(|e| e.to_string())
    })
}

/// 「导出设置」：读出偏好目录里的当前偏好，返回设置文件的全文（字符串，见 `msime_client_core::settings_document`）。语音、AI 辅助和翻译服务的配置与密钥、诊断日志、使用统计和剪贴板历史开关不写进文件。宿主负责让用户选位置并写盘。
/// # Safety
/// `directory` 必须指向 `length` 个可读字节，空指针被拒绝。
/// 返回的响应必须交给 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_export_settings(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        // SAFETY: 由上面写明的调用方契约保证。
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = super::parse_absolute_path(
            bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let snapshot = PreferencesStore::new(directory)
            .load()
            .map_err(|e| e.to_string())?;
        msime_client_core::settings_document::export_document(&snapshot.preferences)
            .map(Value::String)
            .map_err(|e| e.to_string())
    })
}

/// 「导入设置」：把用户选的设置文件换算成偏好（本机的服务配置与密钥、诊断日志、使用统计和剪贴板历史开关保留，按本目录所属的版本收窄输入方案），再按 `expected_revision` 比较并交换写回，返回新的快照。剪贴板历史开关不随文件变，所以导入不会像保存偏好那样清空已存的剪贴板历史，保存成功就是整个导入成功。失败时的错误是 `settings_document_invalid`（不是设置文件）、`settings_document_macos`（macOS 原生设置窗口导出的文件）、`settings_document_unsupported`（更新版本的文件或读不懂的设置）、`settings_conflict`（设置已在别处更新）之一，或者存储错误的说明；失败时什么也不写。
/// # Safety
/// `directory` 必须指向 `directory_length` 个可读字节，`document` 必须指向 `document_length` 个可读字节，空指针被拒绝。
/// 返回的响应必须交给 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_import_settings(
    directory: *const u8,
    directory_length: usize,
    expected_revision: u64,
    document: *const u8,
    document_length: usize,
) -> *mut c_char {
    use msime_client_core::settings_document;
    response(|| {
        if directory.is_null() || document.is_null() || directory_length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        if document_length > settings_document::MAX_DOCUMENT_BYTES {
            return Err(
                settings_document::SettingsDocumentError::NotSettingsDocument
                    .code()
                    .into(),
            );
        }
        // SAFETY: 由上面写明的调用方契约保证。
        let directory_bytes = unsafe { std::slice::from_raw_parts(directory, directory_length) };
        let directory = super::parse_absolute_path(
            directory_bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        // SAFETY: 由上面写明的调用方契约保证。
        let document = unsafe { std::slice::from_raw_parts(document, document_length) };
        let store = PreferencesStore::new(directory);
        let current = store.load().map_err(|e| e.to_string())?;
        let imported =
            settings_document::import_document(document, &current.preferences, store.edition())
                .map_err(|error| error.code().to_owned())?;
        let saved = store
            .save(expected_revision, imported)
            .map_err(|error| match error {
                msime_client_core::preferences::PreferencesError::Conflict => {
                    "settings_conflict".to_owned()
                }
                error => error.to_string(),
            })?;
        serde_json::to_value(saved).map_err(|e| e.to_string())
    })
}

/// Per-key double-pinyin hint text for one profile, read out of the Engine's own profile tables.
///
/// A touch keyboard labels its letter keys with the units they carry, and a face that keeps its own copy of that keymap drifts from the scheme the session actually runs. The hints depend only on the profile, not on session state, so this takes no handle. The reserved name `cantonese` gives the Cantonese shuangpin face, read out of the same table its encoder uses. An unknown name yields an empty object rather than the default profile's hints: labelling the keys with a scheme the session is not running is worse than labelling nothing.
/// # Safety
/// `profile` points to `length` readable UTF-8 bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_shuangpin_key_hints(
    profile: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if profile.is_null() || length > 64 {
            return Err("invalid shuangpin profile buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(profile, length) };
        let name = std::str::from_utf8(bytes).map_err(|_| "invalid shuangpin profile encoding")?;
        let entries = msime_engine::host::shuangpin_key_hints(name);
        let mut hints = serde_json::Map::with_capacity(entries.len());
        hints.extend(
            entries
                .into_iter()
                .map(|entry| (entry.key, serde_json::Value::String(entry.hint))),
        );
        Ok(serde_json::Value::Object(hints))
    })
}

/// The double-pinyin codes of the whole zero-initial syllables for one profile, read out of the Engine's own profile tables, as a JSON object such as `{"a":"aa","ang":"ah",...}`.
///
/// A keymap panel shows these next to the key face, and like the key hints they depend only on the profile, so this takes no handle. An unknown name yields an empty object.
/// # Safety
/// `profile` points to `length` readable UTF-8 bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_shuangpin_zero_initials(
    profile: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if profile.is_null() || length > 64 {
            return Err("invalid shuangpin profile buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(profile, length) };
        let name = std::str::from_utf8(bytes).map_err(|_| "invalid shuangpin profile encoding")?;
        let entries = msime_engine::host::shuangpin_zero_initials(name);
        let mut codes = serde_json::Map::with_capacity(entries.len());
        codes.extend(entries.into_iter().map(|(syllable, code)| {
            (
                syllable.to_string(),
                serde_json::Value::String(code.to_string()),
            )
        }));
        Ok(serde_json::Value::Object(codes))
    })
}

/// 九键双拼方案在宿主看来的样子，见 [`crate::nine_key_shuangpin_layout`]。键面和设置页的编辑器都从这里取，不另存键位。
/// # Safety
/// `request` points to `length` readable UTF-8 bytes holding `{"profile": ..., "table"?: ...}`. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_nine_key_shuangpin_layout(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        profile: msime_client_core::preferences::NineKeyShuangpinProfile,
        #[serde(default)]
        table: msime_client_core::preferences::ShuangpinCustomProfile,
    }
    response(|| {
        if request.is_null() || length > 16384 {
            return Err("invalid nine-key shuangpin request buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request = serde_json::from_slice(bytes)
            .map_err(|error| format!("invalid nine-key shuangpin request: {error}"))?;
        serde_json::to_value(crate::nine_key_shuangpin_layout(
            request.profile,
            &request.table,
        ))
        .map_err(|error| error.to_string())
    })
}

/// 进程里 `msime_client_load_preferences` 最近读到的一份偏好，连同它所在的目录：Android 键盘显示期间每秒在工作线程上读一次，偏好文件没变时不再解析和校验，见 [`PreferencesStore::load_reusing`]。
static LOADED_PREFERENCES: Mutex<
    Option<(String, msime_client_core::preferences::LoadedPreferences)>,
> = Mutex::new(None);

/// Load the shared store on a worker thread; no session handle is accessed.
/// # Safety
/// `directory` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_load_preferences(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = super::parse_absolute_path(
            bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        // 读文件时不持这把锁：同时来的另一次读拿不到上一份，照常整读一遍。
        let previous = LOADED_PREFERENCES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .filter(|(cached, _)| cached == directory)
            .map(|(_, loaded)| loaded);
        let loaded = PreferencesStore::new(directory)
            .load_reusing(previous)
            .map_err(|e| e.to_string())?;
        let value = serde_json::to_value(&loaded.snapshot).map_err(|e| e.to_string())?;
        *LOADED_PREFERENCES
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Some((directory.to_owned(), loaded));
        Ok(value)
    })
}

/// Android 旧布局迁移：在偏好锁内核对原始字段与修订号。
/// # Safety
/// `request` 指向最多 64 KiB 的可读 JSON；返回值通过 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_migrate_android_key_layout(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        revision: u64,
        long_space: bool,
        number_row: bool,
        /// 本机旧键里用户自定义的九键左侧符号表；缺省或空表表示不迁。
        #[serde(default)]
        nine_key_symbols: Vec<String>,
        /// 本机旧键里用户自定义的九键数字键盘左侧符号表；缺省或空表表示不迁。
        #[serde(default)]
        nine_key_digit_symbols: Vec<String>,
        /// 本机旧设置 `platform.android.nine_key_swipe` 的原值；本机没有这个键时不带。
        #[serde(default)]
        nine_key_swipe: Option<String>,
    }
    response(|| {
        if request.is_null() || length > 65536 {
            return Err("invalid layout migration request".into());
        }
        // SAFETY: 调用方保证缓冲区可读，上方已经检查长度。
        let request: Request =
            serde_json::from_slice(unsafe { std::slice::from_raw_parts(request, length) })
                .map_err(|_| "invalid layout migration request")?;
        let directory = super::parse_absolute_path(
            request.directory.as_bytes(),
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let saved = PreferencesStore::new(directory)
            .migrate_android_key_layout(
                request.revision,
                request.long_space,
                request.number_row,
                &request.nine_key_symbols,
                &request.nine_key_digit_symbols,
                // 不认识的旧值（不会出现，旧设置只存过 off、up、down）当作没有旧值，不迁。
                request
                    .nine_key_swipe
                    .as_deref()
                    .and_then(msime_client_core::preferences::TouchNineKeySwipeDigits::parse)
                    .unwrap_or_default(),
            )
            .map_err(|error| error.to_string())?;
        serde_json::to_value(saved).map_err(|error| error.to_string())
    })
}

/// 可重绑切换热键（`keybindings.toggle_english_candidates`、`toggle_full_width`、
/// `toggle_punctuation`、`toggle_voice_input`）按共享 `KeyChord` 契约解码：每个键给出 `null`（不绑定）或
/// `{"key": 主键规范名, "ctrl", "alt", "shift", "meta"}`，宿主按规范名认键，
/// 写法解析只在 Rust 一侧。keybindings 缺键或值为 null 时按出厂值。
/// # Safety
/// `request` 指向 `length` 字节的 JSON `{"keybindings": object}`，最多 64 KiB；
/// keybindings 不是表时返回错误。结果用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_toggle_hotkey_chords(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        #[serde(default)]
        keybindings: Value,
    }
    response(|| {
        if request.is_null() || length > 64 * 1024 {
            return Err("invalid toggle hotkey request buffer".into());
        }
        // SAFETY: 调用方保证缓冲区可读，上方已检查长度上限。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request = serde_json::from_slice(bytes)
            .map_err(|_| "invalid toggle hotkey request".to_owned())?;
        let keys = if request.keybindings.is_null() {
            msime_client_core::preferences::KeybindingPreferences::default()
        } else {
            serde_json::from_value(request.keybindings)
                .map_err(|_| "invalid keybindings".to_owned())?
        };
        Ok(json!({
            "english_candidates": toggle_hotkey_chord(keys.english_candidates_chord()),
            "full_width": toggle_hotkey_chord(keys.full_width_chord()),
            "punctuation": toggle_hotkey_chord(keys.punctuation_chord()),
            "voice_input": toggle_hotkey_chord(keys.toggle_voice_input.parse().ok()),
        }))
    })
}

/// 原生设置录键的试写检查：复用共享偏好的语法、规范化和冲突表，不写入配置。
/// # Safety
/// `request` 指向不超过 64 KiB 的有效 JSON；返回字符串交由 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_check_toggle_hotkey(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_core::preferences::{
        KeybindingPreferences, ToggleHotkeyRejection, ToggleHotkeySlot,
    };
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        field: ToggleHotkeySlot,
        chord: String,
        #[serde(default)]
        keybindings: KeybindingPreferences,
        voice_ctrl_f9: bool,
    }
    response(|| {
        if request.is_null() || length > 64 * 1024 {
            return Err("invalid toggle hotkey request buffer".into());
        }
        // SAFETY: 调用方保证缓冲区可读，上方已检查长度上限。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request = serde_json::from_slice(bytes)
            .map_err(|_| "invalid toggle hotkey request".to_owned())?;
        Ok(
            match request.keybindings.check_toggle_hotkey(
                request.field,
                &request.chord,
                request.voice_ctrl_f9,
            ) {
                Ok(chord) => json!({"ok": true, "chord": chord}),
                Err(ToggleHotkeyRejection::Invalid(_)) => json!({"ok": false, "kind": "invalid"}),
                Err(ToggleHotkeyRejection::Conflict { owner, .. }) => json!({
                    "ok": false, "kind": "conflict", "owner": owner.id(),
                }),
            },
        )
    })
}

fn toggle_hotkey_chord(chord: Option<msime_client_core::key_chord::KeyChord>) -> Value {
    chord.map_or(Value::Null, |chord| {
        json!({
            "key": chord.key.to_string(),
            "ctrl": chord.modifiers.ctrl,
            "alt": chord.modifiers.alt,
            "shift": chord.modifiers.shift,
            "meta": chord.modifiers.meta,
        })
    })
}

/// Read or update private aggregate typing statistics without retaining submitted text.
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_typing_statistics(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        action: StatisticsAction,
        /// 设置页要的是状态：load、set_enabled、set_retention 和 reset 回答 `{statistics, availability, lastWrittenMs}`（client-services 的 `TypingStatisticsStatus`），而不只是统计本身。
        #[serde(default)]
        status: bool,
    }
    #[derive(Deserialize)]
    #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
    enum StatisticsAction {
        Load,
        Record {
            text: String,
            source: TypingSource,
            day: String,
            /// Local hour of the commit, 0-23. Absent from hosts that have not been taught to
            /// send one, whose days then have characters but no hourly breakdown - which is the
            /// honest result, since this layer cannot resolve the host's timezone itself.
            #[serde(default)]
            hour: Option<u8>,
            /// Set by a host that must keep effect sounds quiet right now (Windows while a full-screen application is in front): the commit is still counted and the milestone still reported, but no jingle plays.
            #[serde(default)]
            quiet: bool,
        },
        /// 把本进程为 `directory` 攒着的上屏写进统计文件，答 null。`record` 不再每次上屏都写文件，宿主在键盘收起、输入框失焦和进程或服务退出前，在它平时调统计的那个线程上调用这一项。
        Flush,
        SetEnabled {
            enabled: bool,
        },
        SetRetention {
            retention: String,
            /// The caller's local day, for the same reason `record` takes one: only the host
            /// knows which day the window is counted back from.
            day: String,
        },
        /// Key presses a host batched in memory, as `{"KeyA": 3, ...}` press counts for the local day they happened on. Ids outside `KEY_IDS` or zero counts reject the whole batch.
        RecordKeys {
            day: String,
            keys: std::collections::BTreeMap<String, u64>,
        },
        /// 派生指标（概览、习惯、按键、徽章），新解锁的徽章在锁内写进 `achievements`。隐私模式不影响它：它只读已经记下的数据。
        Summary {
            day: String,
            #[serde(default)]
            user_words: Option<u64>,
        },
        /// 一次语音输入的时长，计在宿主的本地日 `day` 上。
        RecordVoice {
            day: String,
            milliseconds: u64,
        },
        /// 用过一款皮肤（徽章「换装达人」）。
        RecordSkin {
            id: String,
        },
        Reset,
        /// 「上传到云端」的本机状态：开关和上次上传的时间。
        UploadStatus,
        /// 打开或关闭「上传到云端」。只改本机状态，不联网：关闭时宿主自己再发 `DELETE /v1/users/me/typing-statistics`。
        SetUploadEnabled {
            enabled: bool,
        },
        /// 自己发 HTTP 的宿主准备一次上传：交回要发的请求，或者不发的原因。`day` 是宿主的本地日期，`platform` 是服务端认的平台 id。
        UploadRequest {
            day: String,
            platform: String,
            #[serde(default)]
            force: bool,
        },
        /// 上一步交回的请求发成功了（服务端回 204），记下时间；`device` 是那个请求里的设备标识。
        UploadSucceeded {
            device: String,
        },
    }
    response(|| {
        if request.is_null() || length > 65_536 {
            return Err("invalid typing statistics buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request = serde_json::from_slice(bytes)
            .map_err(|_| "invalid typing statistics request".to_owned())?;
        if request.directory.len() > 16_384
            || !std::path::Path::new(&request.directory).is_absolute()
        {
            return Err("invalid typing statistics directory".into());
        }
        let store = TypingStatisticsStore::new(request.directory);
        let status = request.status;
        if status
            && !matches!(
                request.action,
                StatisticsAction::Load
                    | StatisticsAction::SetEnabled { .. }
                    | StatisticsAction::SetRetention { .. }
                    | StatisticsAction::Reset
            )
        {
            return Err(
                "typing statistics status applies to load, set_enabled, set_retention and reset"
                    .into(),
            );
        }
        // 统计本身，或者设置页要的状态。
        let answer = |statistics: msime_client_services::typing_statistics::TypingStatistics| {
            if status {
                serde_json::to_value(
                    store
                        .status(statistics)
                        .map_err(|error| error.to_string())?,
                )
            } else {
                serde_json::to_value(statistics)
            }
            .map_err(|_| "typing statistics response failed".to_owned())
        };
        match request.action {
            StatisticsAction::Load => answer(store.load().map_err(|error| error.to_string())?),
            StatisticsAction::Record {
                text,
                source,
                day,
                hour,
                quiet,
            } => {
                // 累计字数是文件里的加上本进程攒着的，由 record 一并答出：不能再先 load 一遍，load 会把攒着的上屏写出去，成就音效开着时就又变回每次上屏写一次文件。
                let (recorded, total) = store
                    .record_with_total(&text, source, &day, hour)
                    .map_err(|error| error.to_string())?;
                let milestone = key_sound::achievements_armed()
                    .then(|| {
                        msime_client_services::plugins::achievement_milestone(
                            total.saturating_sub(recorded),
                            total,
                        )
                    })
                    .flatten();
                if milestone.is_some() && !quiet {
                    key_sound::achievement();
                }
                Ok(json!({"recorded": recorded, "milestone": milestone}))
            }
            StatisticsAction::Flush => {
                store.flush().map_err(|error| error.to_string())?;
                Ok(Value::Null)
            }
            StatisticsAction::RecordKeys { day, keys } => {
                let recorded = store
                    .record_keys(&day, &keys)
                    .map_err(|error| error.to_string())?;
                Ok(json!({"recorded": recorded}))
            }
            StatisticsAction::Summary { day, user_words } => serde_json::to_value(
                store
                    .summary(
                        &day,
                        &msime_client_services::typing_statistics::SummaryInputs { user_words },
                    )
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|_| "typing statistics response failed".to_owned()),
            StatisticsAction::RecordVoice { day, milliseconds } => {
                let recorded = store
                    .record_voice(&day, milliseconds)
                    .map_err(|error| error.to_string())?;
                Ok(json!({"recorded": recorded}))
            }
            StatisticsAction::RecordSkin { id } => {
                let recorded = store.record_skin(&id).map_err(|error| error.to_string())?;
                Ok(json!({"recorded": recorded}))
            }
            StatisticsAction::SetEnabled { enabled } => answer(
                store
                    .set_enabled(enabled)
                    .map_err(|error| error.to_string())?,
            ),
            StatisticsAction::SetRetention { retention, day } => answer(
                store
                    .set_retention(
                        msime_client_services::typing_statistics::StatisticsRetention::parse(
                            &retention,
                        ),
                        &day,
                    )
                    .map_err(|error| error.to_string())?,
            ),
            StatisticsAction::Reset => answer(store.reset().map_err(|error| error.to_string())?),
            StatisticsAction::UploadStatus => {
                upload_status(&store.upload_state().map_err(|error| error.to_string())?)
            }
            StatisticsAction::SetUploadEnabled { enabled } => upload_status(
                &store
                    .set_upload_enabled(enabled)
                    .map_err(|error| error.to_string())?,
            ),
            StatisticsAction::UploadRequest {
                day,
                platform,
                force,
            } => upload_request(
                store
                    .prepare_upload(&day, &platform, unix_ms(), force)
                    .map_err(|error| error.to_string())?,
            ),
            StatisticsAction::UploadSucceeded { device } => upload_status(
                &store
                    .record_upload(&device, unix_ms())
                    .map_err(|error| error.to_string())?,
            ),
        }
    })
}

/// 「上传到云端」的本机状态：`{enabled, lastUploadMs}`，与设置页读的形状相同（client-services 的 `UploadStatus`）。
fn upload_status(
    state: &msime_client_services::typing_statistics::upload::UploadState,
) -> Result<Value, String> {
    serde_json::to_value(
        msime_client_services::typing_statistics::upload::UploadStatus::from(state),
    )
    .map_err(|_| "typing statistics response failed".to_owned())
}

/// `upload_request` 的回答：要发时是 `{method, path, device, body}`，`body` 是请求体对象，宿主序列化后以 `application/json` 发出；不发时是 `{skip}`，取 `disabled`（没有打开上传）、`not_due`（离上次上传不到十分钟）或 `nothing`（最近没有可传的日子）。
fn upload_request(
    prepared: Result<
        msime_client_services::typing_statistics::upload::TypingStatisticsUpload,
        msime_client_services::typing_statistics::upload::UploadOutcome,
    >,
) -> Result<Value, String> {
    use msime_client_services::typing_statistics::upload::UploadOutcome;
    match prepared {
        Ok(payload) => Ok(json!({
            "method": "PUT",
            "path": "/v1/users/me/typing-statistics",
            "device": payload.device,
            "body": serde_json::to_value(&payload)
                .map_err(|_| "typing statistics response failed".to_owned())?,
        })),
        Err(outcome) => Ok(json!({
            "skip": match outcome {
                UploadOutcome::Disabled => "disabled",
                UploadOutcome::NotDue => "not_due",
                UploadOutcome::Nothing | UploadOutcome::Uploaded(_) => "nothing",
            }
        })),
    }
}

/// 现在的 Unix 毫秒，记上传时间和算十分钟的间隔用。
fn unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Read the typing-statistics master switch without accepting committed text.
///
/// Native hosts cache this result and refresh it when settings change, so an opt-out can stop at
/// the capture boundary instead of serializing text merely to discover that recording is off.
/// # Safety
/// `directory` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_typing_statistics_enabled(
    directory: *const u8,
    length: usize,
) -> i32 {
    if directory.is_null() || length > 16_384 {
        return -1;
    }
    // SAFETY: guaranteed by the documented caller contract.
    let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
    let Ok(directory) = std::str::from_utf8(bytes) else {
        return -1;
    };
    if !std::path::Path::new(directory).is_absolute() {
        return -1;
    }
    match TypingStatisticsStore::new(directory).load() {
        Ok(statistics) => i32::from(statistics.enabled),
        Err(_) => -1,
    }
}

/// Scan a skin root so native presenters read the same catalog the settings
/// page edits. Unreadable roots return an empty catalog, not an error; a
/// package that fails validation is reported as an issue and never rendered.
/// # Safety
/// `directory` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_catalog(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid skin directory buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory =
            std::str::from_utf8(bytes).map_err(|_| "invalid skin directory encoding")?;
        if !std::path::Path::new(directory).is_absolute() {
            return Err("skin directory must be absolute".into());
        }
        serde_json::to_value(msime_client_services::skin::catalog::scan(directory))
            .map_err(|e| e.to_string())
    })
}

/// Scan the Engine resource directory for optional custom helper-code tables. Native settings
/// presenters use the same metadata as the shared settings page; the Engine remains responsible
/// for parsing and applying the table itself. An absent `helpcodes/custom` directory is an empty
/// catalog rather than an error.
/// # Safety
/// `resources` points to `length` readable UTF-8 bytes naming an absolute resource directory.
/// Null is rejected. The returned JSON must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_helpcode_schemas(
    resources: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if resources.is_null() || length > 16_384 {
            return Err("invalid helpcode resource buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(resources, length) };
        let resources =
            std::str::from_utf8(bytes).map_err(|_| "invalid helpcode resource encoding")?;
        let schemas = crate::list_custom_helpcode_schemas(resources)?;
        serde_json::to_value(schemas).map_err(|error| error.to_string())
    })
}

#[derive(Debug, Deserialize)]
struct SkinPackageRequest {
    directory: String,
    id: String,
}

/// Validate one installed skin package with the loader the settings page uses, so a native presenter resolving the selected skin accepts exactly the manifests the catalog lists (full TOML 1.0, the Windows toml++ baseline). The value is one camelCase entry of `msime_client_skin_catalog`'s `packages`; a package that fails validation answers `{ok:false,error}` with the loader's reason.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_package(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > 65_536 {
            return Err("invalid skin package request".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: SkinPackageRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid skin package request")?;
        if !Path::new(&request.directory).is_absolute() {
            return Err("skin directory must be absolute".into());
        }
        let package =
            msime_client_services::skin::catalog::load_package(&request.directory, &request.id)?;
        serde_json::to_value(package).map_err(|e| e.to_string())
    })
}

#[derive(Debug, Deserialize)]
struct SkinResourceRequest {
    directory: String,
    id: String,
    relative: String,
    kind: String,
    /// `"base64"` returns the bytes as one base64 string, `bytesBase64`, instead of the `bytes` array. Absent keeps the array for older callers; every msime host asks for base64.
    #[serde(default)]
    encoding: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkinToolbarImageLayoutRequest {
    toolbar: msime_client_services::skin::catalog::SkinToolbarImage,
    shown: Vec<[String; 2]>,
}

/// 根据扫描缓存里的 `toolbarImage` 与当前显示的 `[按钮键, 状态]` 计算图片工具栏。
/// 不读文件；状态切换时不应重新加载或解码整个皮肤。返回窗口尺寸及按钮矩形、底图和状态图。
/// # Safety
/// `request` 指向 `length` 字节可读 UTF-8 JSON，结果由 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_toolbar_image_layout(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > 65_536 {
            return Err("invalid toolbar image layout request".into());
        }
        // 安全性：调用方保证请求缓冲可读，上面已限制长度。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: SkinToolbarImageLayoutRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid toolbar image layout request")?;
        if request.shown.len() > msime_client_services::skin::catalog::TOOLBAR_BUTTON_KEYS.len()
            || request
                .shown
                .iter()
                .any(|[key, state]| key.len() > 32 || state.len() > 32)
        {
            return Err("invalid toolbar image button request".into());
        }
        let layout = request.toolbar.checked_layout(
            request
                .shown
                .iter()
                .map(|[key, state]| (key.as_str(), state.as_str())),
        )?;
        serde_json::to_value(layout).map_err(|error| error.to_string())
    })
}

#[derive(Debug, Deserialize)]
struct SkinStylesheetRequest {
    directory: String,
    id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SkinResourceResponse {
    content_type: &'static str,
    bytes: Vec<u8>,
}

/// Read one validated image or font from an external skin package.
///
/// The request keeps the root directory explicit because Harmony's settings
/// bridge and the input-method ability share the same C ABI but have different
/// lifetimes. The package manifest is revalidated by `read_resource` on every
/// call, so a stale catalog cannot turn this endpoint into an arbitrary file
/// reader. `kind` is deliberately checked here as well: a WebView image reader
/// must never receive CSS or a font by mistake.
///
/// The default `bytes` array costs about 64 bytes of heap per resource byte while the response is built (a `serde_json::Value` per byte, measured on a 1 MiB image), and a JSON parser on the host side spends tens more turning it back into bytes. The iOS keyboard extension reads candidate-skin images under the system's per-extension memory limit, where that spike for a single 1 MiB image is larger than everything else the keyboard holds, so it asks for `encoding: "base64"`. So do the Android and HarmonyOS keyboards, the HarmonyOS settings page and both Windows settings apps: at the 8 MiB resource limit the array is about 512 MiB on this side alone. The array stays the default so an older caller keeps working.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_resource(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > 65_536 {
            return Err("invalid skin resource request".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: SkinResourceRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid skin resource request")?;
        if request.kind != "image" && request.kind != "font" {
            return Err("unsupported skin resource kind".into());
        }
        let as_base64 = match request.encoding.as_deref() {
            None => false,
            Some("base64") => true,
            Some(_) => return Err("unsupported skin resource encoding".into()),
        };
        if !Path::new(&request.directory).is_absolute() {
            return Err("skin directory must be absolute".into());
        }
        let resource = msime_client_services::skin::catalog::read_resource(
            &request.directory,
            &request.id,
            &request.relative,
        )
        .map_err(|_| "skin resource unavailable")?;
        let expected = if request.kind == "image" {
            "image/"
        } else {
            "font/"
        };
        if !resource.content_type.starts_with(expected) {
            return Err("skin resource type mismatch".into());
        }
        if as_base64 {
            use base64::Engine as _;
            return Ok(json!({
                "contentType": resource.content_type,
                "bytesBase64": base64::engine::general_purpose::STANDARD.encode(&resource.bytes),
            }));
        }
        serde_json::to_value(SkinResourceResponse {
            content_type: resource.content_type,
            bytes: resource.bytes,
        })
        .map_err(|_| "skin resource response failed".into())
    })
}

/// Read the stylesheet declared by an external skin package.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_toolbar_stylesheet(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > 65_536 {
            return Err("invalid skin stylesheet request".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: SkinStylesheetRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid skin stylesheet request")?;
        if !Path::new(&request.directory).is_absolute() {
            return Err("skin directory must be absolute".into());
        }
        let stylesheet = msime_client_services::skin::catalog::read_toolbar_stylesheet(
            &request.directory,
            &request.id,
        )
        .map_err(|_| "skin stylesheet unavailable")?;
        serde_json::to_value(stylesheet).map_err(|_| "skin stylesheet response failed".into())
    })
}

#[derive(Debug, Deserialize)]
struct SkinImportRequest {
    source: String,
    directory: String,
}

/// Copy a skin folder the user picked into the host's skin root, for a host whose root no file manager reaches (the iOS App Group). Returns `{id}`, the folder name the catalog lists it under; a failure's message is the import's code (`skin_name`, `skin_manifest` or `storage`) so the host can explain it.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_skin_import(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length == 0 || length > 65_536 {
            return Err("invalid skin import request".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: SkinImportRequest =
            serde_json::from_slice(bytes).map_err(|_| "invalid skin import request")?;
        if !Path::new(&request.source).is_absolute() || !Path::new(&request.directory).is_absolute()
        {
            return Err("skin paths must be absolute".into());
        }
        let id = msime_client_services::skin::folder_import::import(
            Path::new(&request.source),
            Path::new(&request.directory),
        )
        .map_err(str::to_owned)?;
        Ok(json!({ "id": id }))
    })
}

/// 「制作皮肤」请求最大的长度：图片合计 2 MiB 换成 base64 后约 2.7 MiB，加上 64 KiB 的清单和 JSON 的键，留出余量。
const MAX_CANDIDATE_SKIN_EDITOR_REQUEST_BYTES: usize = 4 << 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CandidateSkinEditorRequest {
    operation: String,
    /// 只有 `create` 读它：皮肤根目录的绝对路径。
    #[serde(default)]
    directory: Option<String>,
    package_id: String,
    manifest: String,
    #[serde(default)]
    files: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    replace: bool,
}

/// 共享设置页「制作皮肤」的两个操作，规则全在 `msime_client_services::skin::candidate_editor`：`check` 不写皮肤目录，按目录扫描的规则读一遍草稿，回复扫描会列出的那一项（camelCase，与 `msime_client_skin_catalog` 的 `packages` 同形）；`create` 用社区下载和 MCP 同一个安装器把草稿装进 `directory`，回复 `{id,replaced}`。失败回复 `{ok:false,error,detail?}`：`error` 是 `candidate_skin_*` 或 `storage`，`detail` 是清单没通过扫描时扫描给出的原因。
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes, at most 4 MiB. Null is rejected. The returned JSON must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_candidate_skin_editor(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::skin::candidate_editor::{
        self, CandidateSkinDraft, CandidateSkinFailure,
    };
    candidate_skin_editor_response(|| {
        let invalid = || CandidateSkinFailure::code("candidate_skin_package");
        if request.is_null() || length == 0 || length > MAX_CANDIDATE_SKIN_EDITOR_REQUEST_BYTES {
            return Err(invalid());
        }
        // SAFETY: guaranteed by the documented caller contract; size checked above.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: CandidateSkinEditorRequest =
            serde_json::from_slice(bytes).map_err(|_| invalid())?;
        let draft = CandidateSkinDraft {
            package_id: request.package_id,
            manifest: request.manifest,
            files: request.files,
        };
        let storage = || CandidateSkinFailure::code("storage");
        match request.operation.as_str() {
            "check" => {
                serde_json::to_value(candidate_editor::check(&draft)?).map_err(|_| storage())
            }
            "create" => {
                let directory = request.directory.ok_or_else(invalid)?;
                if !Path::new(&directory).is_absolute() {
                    return Err(invalid());
                }
                serde_json::to_value(candidate_editor::create(
                    Path::new(&directory),
                    &draft,
                    request.replace,
                )?)
                .map_err(|_| storage())
            }
            _ => Err(invalid()),
        }
    })
}

/// 和插件的 `plugin_response` 一样把 `detail` 带给页面。
fn candidate_skin_editor_response(
    operation: impl FnOnce() -> Result<
        Value,
        msime_client_services::skin::candidate_editor::CandidateSkinFailure,
    >,
) -> *mut c_char {
    use msime_client_services::skin::candidate_editor::CandidateSkinFailure;
    let value = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => json!({ "ok": true, "value": value }),
        Ok(Err(CandidateSkinFailure {
            code,
            detail: Some(detail),
        })) => json!({ "ok": false, "error": code, "detail": detail }),
        Ok(Err(CandidateSkinFailure { code, detail: None })) => {
            json!({ "ok": false, "error": code })
        }
        Err(_) => json!({ "ok": false, "error": "internal runtime failure" }),
    };
    // JSON escapes embedded NUL bytes, so this cannot contain an interior NUL.
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}

#[derive(Debug, Deserialize)]
struct CustomSkinLibraryRequest {
    directory: String,
    action: Option<msime_client_services::skin::custom_library::CustomSkinLibraryAction>,
}

/// Read or change the named custom touch-keyboard designs.
///
/// The Tauri hosts hold `CustomSkinLibraryStore` as Rust and call it directly.
/// A host that reaches this crate only through the C ABI - the HarmonyOS
/// settings bridge is the one that does - would otherwise have to write a
/// second implementation of the same file: its locking, its atomic replace, its
/// name normalization and its twelve-item limit. Two stores for one library is
/// how the two of them start disagreeing about what is in it.
///
/// A request with no `action` reads; one with an action applies it. Both answer
/// with the whole library, because every caller redraws the list afterwards and
/// a mutation that returned only its own item would leave the page guessing
/// what the rename did to the ordering.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_custom_skin_library(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // A design can carry a bounded photo, so the ceiling is the store's own
        // file limit rather than the small one the other requests here use.
        if request.is_null() || length > 9_000_000 {
            return Err("community_storage".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: CustomSkinLibraryRequest =
            serde_json::from_slice(bytes).map_err(|_| "community_invalid")?;
        if !Path::new(&request.directory).is_absolute() {
            return Err("community_storage".into());
        }
        let store = msime_client_services::skin::custom_library::CustomSkinLibraryStore::new(
            &request.directory,
        );
        let items = match request.action {
            None => store.load(),
            Some(action) => store.mutate(action),
        }
        .map_err(custom_skin_library_code)?;
        serde_json::to_value(items).map_err(|_| "community_storage".into())
    })
}

/// 读取共用键盘皮肤目录，或显式重置、选择模板／背景及优化对比度；不写设置或命名库。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节；空指针和超过 1 MiB 的请求被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_keyboard_skin_design(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: 调用方保证指针有效，公共边界先检查长度和空指针。
        unsafe {
            with_bounded_bytes(
                request,
                length,
                1024 * 1024,
                "keyboard_skin_invalid",
                |bytes| {
                    let action = serde_json::from_slice(bytes)
                        .map_err(|_| "keyboard_skin_invalid".to_owned())?;
                    msime_client_core::keyboard_skin::edit(action).map_err(str::to_owned)
                },
            )
        }
    })
}

/// The codes the shared community pages already have a sentence for. A host that
/// forwarded the `Display` text instead would put an English sentence written
/// for a log into a Chinese dialog.
fn custom_skin_library_code(
    error: msime_client_services::skin::custom_library::CustomSkinLibraryError,
) -> &'static str {
    use msime_client_services::skin::custom_library::CustomSkinLibraryError as Failure;
    match error {
        Failure::Full => "community_skin_library_full",
        Failure::InvalidName => "community_skin_invalid_name",
        Failure::DuplicateName => "community_skin_duplicate_name",
        Failure::NotFound => "community_not_found",
        Failure::Json(_) | Failure::Invalid => "community_skin_library_format",
        Failure::Io(_) => "community_storage",
    }
}

#[derive(Debug, Deserialize)]
struct CommunitySkinInstallRequest {
    directory: String,
    id: String,
    name: String,
    design: msime_client_core::preferences::TouchKeyboardSkinDesign,
}

#[derive(Debug, Serialize)]
struct CommunitySkinInstallResponse {
    skin: msime_client_services::skin::custom_library::SavedTouchKeyboardSkin,
    trial: msime_client_services::skin::keyboard_trial::KeyboardSkinTrial,
}

/// Put a downloaded community design on the keyboard and into the library.
///
/// One entry point rather than two, because the two steps are not independent.
/// The trial has to start first - it is what remembers the skin the user was
/// using - and if the library then refuses the import, the trial must be
/// finished without keeping it or the user is left wearing a skin that was
/// never saved and has nothing to restore from. A host doing this in two calls
/// owns that rollback, and every host that owns it writes it slightly
/// differently.
///
/// The download itself is not here. Fetching the design is the one part that
/// has to go through the surrounding platform's HTTPS stack, so the caller
/// hands over a design it already has.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_community_skin_install(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 9_000_000 {
            return Err("community_storage".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: CommunitySkinInstallRequest =
            serde_json::from_slice(bytes).map_err(|_| "community_invalid")?;
        if !Path::new(&request.directory).is_absolute() {
            return Err("community_storage".into());
        }
        let id = msime_client_core::uuid::Uuid::parse_str(&request.id)
            .map_err(|_| "community_invalid")?;
        install_downloaded_keyboard_skin(
            Path::new(&request.directory),
            id,
            &request.name,
            request.design,
        )
        .map_err(Into::into)
    })
}

/// 把下载好的社区键盘皮肤装上：先开始试用、再导入皮肤库，导入失败就结束试用、不保留，返回 `{skin, trial}`。`directory` 是状态根目录的绝对路径。`msime_client_community_skin_install` 和 `msime_client_community` 的 `keyboard_skin_install` 共用它；错误是 `keyboard_skin_trial_code` 和 `custom_skin_library_code` 的码。
pub(super) fn install_downloaded_keyboard_skin(
    directory: &Path,
    id: msime_client_core::uuid::Uuid,
    name: &str,
    design: msime_client_core::preferences::TouchKeyboardSkinDesign,
) -> Result<Value, &'static str> {
    let library =
        msime_client_services::skin::custom_library::CustomSkinLibraryStore::new(directory);
    let trials = msime_client_services::skin::keyboard_trial::KeyboardSkinTrialStore::new(
        directory,
        std::sync::Arc::new(msime_client_core::preferences::PreferencesStore::new(
            directory,
        )),
    );
    let (trial, _) = trials
        .begin(name, design.clone())
        .map_err(keyboard_skin_trial_code)?;
    let skin = match library.import_download(id, name, design) {
        Ok(skin) => skin,
        Err(error) => {
            let _ = trials.finish(trial.id, false);
            return Err(custom_skin_library_code(error));
        }
    };
    serde_json::to_value(CommunitySkinInstallResponse { skin, trial })
        .map_err(|_| "community_storage")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
enum KeyboardSkinTrialAction {
    Finish { id: String, keep: bool },
    RestorePending,
}

#[derive(Debug, Deserialize)]
struct KeyboardSkinTrialRequest {
    directory: String,
    action: KeyboardSkinTrialAction,
}

/// End or recover a touch-keyboard skin trial.
///
/// A trial is what makes "try this skin" reversible: the record beside the
/// preference document remembers the skin that was in use, so declining puts it
/// back and a crash mid-trial does not leave the user stuck in someone else's
/// design. `restore_pending` is that recovery, and it is safe to call when
/// there is no trial - it answers with the current preferences.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_keyboard_skin_trial(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 16384 {
            return Err("community_storage".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: KeyboardSkinTrialRequest =
            serde_json::from_slice(bytes).map_err(|_| "community_invalid")?;
        if !Path::new(&request.directory).is_absolute() {
            return Err("community_storage".into());
        }
        let trials = msime_client_services::skin::keyboard_trial::KeyboardSkinTrialStore::new(
            &request.directory,
            std::sync::Arc::new(msime_client_core::preferences::PreferencesStore::new(
                &request.directory,
            )),
        );
        let snapshot = match request.action {
            KeyboardSkinTrialAction::Finish { id, keep } => {
                let id = msime_client_core::uuid::Uuid::parse_str(&id)
                    .map_err(|_| "community_invalid")?;
                trials.finish(id, keep)
            }
            KeyboardSkinTrialAction::RestorePending => trials.restore_pending(),
        }
        .map_err(keyboard_skin_trial_code)?;
        // The revision comes back so a caller holding the document can tell whether the
        // preferences it is showing are still the ones on disk.
        Ok(json!({"revision": snapshot.revision}))
    })
}

fn keyboard_skin_trial_code(
    error: msime_client_services::skin::keyboard_trial::KeyboardSkinTrialError,
) -> &'static str {
    use msime_client_services::skin::keyboard_trial::KeyboardSkinTrialError as Failure;
    match error {
        Failure::Io(_) | Failure::Preferences(_) => "community_storage",
        Failure::Json(_) | Failure::Invalid => "community_trial_format",
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
enum CommunityResourceLibraryAction {
    Load,
    SaveReply {
        item: Box<msime_client_services::community::resource::CommunityResource>,
    },
    Remove {
        id: String,
    },
}

#[derive(Debug, Deserialize)]
struct CommunityResourceLibraryRequest {
    file: String,
    action: CommunityResourceLibraryAction,
}

/// Read or change the reply templates the user explicitly kept.
///
/// This file is the one thing the settings surface and the keyboard process
/// share about the community: the keyboard rereads it when a reply is asked
/// for, and it holds only what the user chose to keep. Writing it needs the
/// store's validation - reply kind, a non-empty prompt, no dictionary entries,
/// the fifty-item ceiling - and a host that wrote the file itself would be a
/// second author of a format the keyboard parses strictly, which is a
/// disagreement waiting to happen rather than a saving.
///
/// Every operation answers with the whole library for the same reason the skin
/// library does: the caller redraws the list.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_community_resource_library(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 4_000_000 {
            return Err("community_storage".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: CommunityResourceLibraryRequest =
            serde_json::from_slice(bytes).map_err(|_| "community_invalid")?;
        if !Path::new(&request.file).is_absolute() {
            return Err("community_storage".into());
        }
        let store =
            msime_client_services::community::resource_library::CommunityResourceLibraryStore::new(
                &request.file,
            );
        match request.action {
            CommunityResourceLibraryAction::Load => {}
            CommunityResourceLibraryAction::SaveReply { item } => {
                store.save_reply(*item).map_err(resource_library_code)?;
            }
            CommunityResourceLibraryAction::Remove { id } => {
                let id = msime_client_core::uuid::Uuid::parse_str(&id)
                    .map_err(|_| "community_invalid")?;
                store.remove(id).map_err(resource_library_code)?;
            }
        }
        serde_json::to_value(store.load().map_err(resource_library_code)?)
            .map_err(|_| "community_storage".into())
    })
}

fn resource_library_code(
    error: msime_client_services::community::resource_library::CommunityResourceLibraryError,
) -> String {
    use msime_client_services::community::resource_library::CommunityResourceLibraryError as Failure;
    match error {
        Failure::Io(_) => "community_storage",
        Failure::Json(_) | Failure::Invalid => "community_resource_library_format",
    }
    .to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
enum AiSkinPlanRequest {
    Compose {
        prompt: String,
        model: String,
    },
    Parse {
        text: String,
    },
    Artwork {
        artwork: msime_client_services::skin::ai::AiSkinArtwork,
    },
}

/// The parts of AI skin generation that are a decision rather than a transfer.
///
/// `BackendAiSkinService::generate` does the whole pipeline, and the hosts that
/// can run it do. A host whose HTTP must go through the surrounding platform -
/// HarmonyOS, whose settings surface reaches the network through the system
/// stack - cannot, so it performs the four requests itself and asks here for
/// everything that is not the transfer: what to say to the model, whether the
/// answer is three usable designs, and whether a returned image is one this
/// client will show.
///
/// The split is the same one `msime_client_ai_request_for_query` and
/// `msime_client_parse_ai_response` already make for candidates. What must not
/// be split is the instruction from the parser: the system prompt names the
/// exact document `plan_ai_skins` refuses anything else for, so `compose`
/// returns it rather than letting a host write its own.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_ai_skin_plan(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // An artwork payload carries base64 image bytes, which the shared service bounds at 11 MB.
        if request.is_null() || length > 12 * 1024 * 1024 {
            return Err("ai_skin_invalid".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: AiSkinPlanRequest =
            serde_json::from_slice(bytes).map_err(|_| "ai_skin_invalid")?;
        match request {
            AiSkinPlanRequest::Compose { prompt, model } => {
                // The same bounds `generate` applies before it spends anything: a prompt this
                // refuses is one the service would refuse after four requests.
                if prompt.is_empty()
                    || prompt.chars().count() > 500
                    || msime_client_core::has_disallowed_control_with_options(&prompt, false)
                    || model.is_empty()
                    || !is_bounded_text(&model, 200)
                {
                    return Err("ai_skin_invalid".into());
                }
                Ok(json!({
                    "path": "/v1/chat/completions",
                    "body": {
                        "messages": [
                            {
                                "role": "system",
                                "content": msime_client_services::skin::ai::AI_SKIN_SYSTEM_PROMPT,
                            },
                            {"role": "user", "content": prompt},
                        ],
                        "model": model,
                        "max_tokens": 2048,
                        "stream": false,
                    },
                }))
            }
            AiSkinPlanRequest::Parse { text } => {
                let plans = msime_client_services::skin::ai::plan_ai_skins(&text)
                    .map_err(|_| "ai_skin_response")?;
                serde_json::to_value(plans).map_err(|_| "ai_skin_response".into())
            }
            AiSkinPlanRequest::Artwork { artwork } => {
                msime_client_services::skin::ai::validate_ai_skin_artwork(&artwork)
                    .map_err(|_| "ai_skin_response")?;
                Ok(json!({"valid": true}))
            }
        }
    })
}

/// What dictionary is installed, for the settings page to show.
///
/// The packaged dictionary ships with a manifest naming which specification it
/// was built to and which upstream commit it came from. Apple's settings read
/// it straight out of the app bundle; a host whose resources are staged into a
/// sandbox cannot, and the path is not something a settings page should be
/// told anyway.
///
/// Only two fields come back. The manifest also records journal modes, format
/// contracts and every third-party reference, none of which answers the
/// question the page is asking — which is "what do I have, and where did it
/// come from". A missing or unreadable manifest is reported rather than
/// guessed at: showing the wrong dictionary version is worse than showing
/// none.
/// # Safety
/// `resources` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_dictionary_manifest(
    resources: *const u8,
    length: usize,
) -> *mut c_char {
    #[derive(Deserialize)]
    struct Source {
        commit: String,
    }
    #[derive(Deserialize)]
    struct Manifest {
        profile: String,
        source: Source,
    }
    response(|| {
        if resources.is_null() || length > 16384 {
            return Err("dictionary_manifest_unavailable".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(resources, length) };
        let directory =
            std::str::from_utf8(bytes).map_err(|_| "dictionary_manifest_unavailable")?;
        let path = Path::new(directory);
        if !path.is_absolute() {
            return Err("dictionary_manifest_unavailable".into());
        }
        // Bounded before parsing: this is a packaged file, and one that has grown to megabytes is
        // not a manifest whatever it parses as.
        let file = path.join("msime-dictionary-manifest.json");
        let bytes = crate::bounded_file::read(
            crate::bounded_file::open_private(&file)
                .map_err(|_| "dictionary_manifest_unavailable")?,
            1024 * 1024,
        )
        .map_err(|_| "dictionary_manifest_unavailable")?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "dictionary_manifest_unavailable")?;
        let manifest: Manifest =
            serde_json::from_str(text).map_err(|_| "dictionary_manifest_unavailable")?;
        if manifest.profile.is_empty()
            || !is_bounded_text(&manifest.profile, 64)
            || !msime_client_core::is_ascii_hex(&manifest.source.commit, 40)
        {
            return Err("dictionary_manifest_unavailable".into());
        }
        Ok(json!({"profile": manifest.profile, "sourceCommit": manifest.source.commit}))
    })
}

/// Read saved clipboard history without observing or modifying the system clipboard.
/// # Safety
/// `directory` points to `length` readable UTF-8 bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_load_clipboard_history(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid history directory buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory =
            std::str::from_utf8(bytes).map_err(|_| "invalid history directory encoding")?;
        let path = std::path::Path::new(directory);
        if !path.is_absolute() {
            return Err("history directory must be absolute".into());
        }
        // 键盘的剪贴板面板和输入一样按运行时读法读偏好：文档里别的设置读不了时，面板照常可用；同意与否仍取文档里写的值，整份读不了时取默认的关闭。
        let preferences = PreferencesStore::new(path)
            .load_for_runtime()
            .map_err(|_| "history preferences unavailable")?
            .snapshot
            .preferences;
        let enabled = preferences.clipboard_history;
        if !enabled {
            return Ok(serde_json::json!({"enabled": false, "entries": []}));
        }
        let mut history = msime_client_core::clipboard::ClipboardHistoryStore::open_with_limit(
            path.join("clipboard_history.json"),
            preferences.clipboard_history_limit,
        );
        history
            .load()
            .map_err(|_| "clipboard history unavailable")?;
        let mut entries = Vec::with_capacity(history.entries().len());
        entries.extend(history.entries().iter().map(|entry| entry.text.as_str()));
        // This ABI returns plain entry text; the structured history is not exposed through it.
        Ok(serde_json::json!({"enabled": true, "entries": entries}))
    })
}

const MAX_MOBILE_CLIPBOARD_REQUEST_BYTES: usize = 524_288;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MobileClipboardRequest {
    directory: String,
    preferences_directory: Option<String>,
    action: MobileClipboardAction,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum MobileClipboardAction {
    Load,
    Categories,
    AddCategory {
        name: String,
    },
    RenameCategory {
        id: String,
        name: String,
    },
    RemoveCategory {
        id: String,
    },
    SetCategory {
        text: String,
        id: Option<String>,
    },
    ExportText,
    Capture {
        text: String,
    },
    SetPinned {
        text: String,
        pinned: bool,
    },
    Remove {
        text: String,
    },
    RemoveMany {
        texts: Vec<String>,
    },
    RestoreMany {
        entries: Vec<msime_client_core::clipboard::ClipboardHistoryEntry>,
    },
    Replace {
        text: String,
        replacement: String,
    },
    Clear,
}

/// 移动端结构化剪贴板历史：`directory/MSIME` 存历史；可选的 `preferences_directory` 单独指定容量偏好所在目录，省略时沿用 `directory`。移动端访问由宿主权限控制，不读取或改变桌面自动记录开关。
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_mobile_clipboard_history(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > MAX_MOBILE_CLIPBOARD_REQUEST_BYTES {
            return Err("invalid mobile clipboard request buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: MobileClipboardRequest = serde_json::from_slice(bytes)
            .map_err(|_| "invalid mobile clipboard request document")?;
        let root = std::path::Path::new(&request.directory);
        if !root.is_absolute() || request.directory.len() > 16384 {
            return Err("invalid mobile clipboard directory".into());
        }
        let preferences_directory = request
            .preferences_directory
            .as_deref()
            .unwrap_or(&request.directory);
        let preferences_root = std::path::Path::new(preferences_directory);
        if !preferences_root.is_absolute() || preferences_directory.len() > 16384 {
            return Err("invalid mobile clipboard preferences directory".into());
        }
        let path = root.join("MSIME").join("clipboard_history.json");
        let mut history = msime_client_core::clipboard::ClipboardHistoryStore::open_with_limit(
            path,
            clipboard_history_limit(preferences_root),
        );
        match request.action {
            MobileClipboardAction::Categories => {
                let categories = history
                    .categories()
                    .map_err(|_| "mobile clipboard categories unavailable")?;
                Ok(
                    json!({"categories": categories, "maxCategories": msime_client_core::clipboard::MAX_CATEGORIES,
                    "maxNameCharacters": msime_client_core::clipboard::MAX_CATEGORY_NAME_CHARACTERS}),
                )
            }
            MobileClipboardAction::AddCategory { name } => {
                history.add_category(&name).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::InvalidInput {
                        "invalid clipboard category name or category limit reached"
                    } else {
                        "mobile clipboard categories unavailable"
                    }
                })?;
                Ok(json!({"updated": true}))
            }
            MobileClipboardAction::RenameCategory { id, name } => {
                let updated = history
                    .rename_category(&id, &name)
                    .map_err(|_| "mobile clipboard category rename failed")?;
                Ok(json!({"updated": updated}))
            }
            MobileClipboardAction::RemoveCategory { id } => {
                let updated = history
                    .remove_category(&id)
                    .map_err(|_| "mobile clipboard category removal failed")?;
                Ok(json!({"updated": updated}))
            }
            MobileClipboardAction::SetCategory { text, id } => {
                if text.is_empty()
                    || text.len() > msime_client_core::clipboard::MAX_MOBILE_TEXT_BYTES
                {
                    return Err("invalid mobile clipboard entry".into());
                }
                let updated = history
                    .set_category(&text, id.as_deref())
                    .map_err(|_| "mobile clipboard category update failed")?;
                Ok(json!({"updated": updated}))
            }
            MobileClipboardAction::ExportText => {
                history
                    .load()
                    .map_err(|_| "mobile clipboard history unavailable")?;
                Ok(
                    json!({"text": history.export_text(), "count": history.entries().len(), "limit": history.limit()}),
                )
            }
            MobileClipboardAction::Load => {
                history
                    .load()
                    .map_err(|_| "mobile clipboard history unavailable")?;
                Ok(
                    json!({"entries": history.entries(), "limit": history.limit(),
                    "categories": history.categories().map_err(|_| "mobile clipboard categories unavailable")?}),
                )
            }
            MobileClipboardAction::Capture { text } => {
                if !msime_client_core::clipboard::mobile_text_is_valid(&text) {
                    return Ok(json!({"captured": false, "reason": "invalid"}));
                }
                let captured = history
                    .push_mobile(text)
                    .map_err(|_| "mobile clipboard capture failed")?;
                Ok(json!({
                    "captured": captured,
                    "reason": (!captured).then_some("full"),
                    "limit": history.limit(),
                    "entries": history.entries()
                }))
            }
            MobileClipboardAction::SetPinned { text, pinned } => {
                if text.is_empty()
                    || text.len() > msime_client_core::clipboard::MAX_MOBILE_TEXT_BYTES
                {
                    return Err("invalid mobile clipboard entry".into());
                }
                let updated = history
                    .set_pinned(&text, pinned)
                    .map_err(|_| "mobile clipboard pin update failed")?;
                Ok(json!({
                    "updated": updated,
                    "limit": history.limit(),
                    "entries": history.entries()
                }))
            }
            MobileClipboardAction::Remove { text } => {
                if text.is_empty()
                    || text.len() > msime_client_core::clipboard::MAX_MOBILE_TEXT_BYTES
                {
                    return Err("invalid mobile clipboard entry".into());
                }
                let removed = history
                    .remove(&text)
                    .map_err(|_| "mobile clipboard removal failed")?;
                Ok(json!({
                    "removed": removed,
                    "limit": history.limit(),
                    "entries": history.entries()
                }))
            }
            MobileClipboardAction::RemoveMany { texts } => {
                if texts.is_empty()
                    || texts.len() > msime_client_core::clipboard::MAX_ENTRIES
                    || texts.iter().any(|text| {
                        text.is_empty()
                            || text.len() > msime_client_core::clipboard::MAX_MOBILE_TEXT_BYTES
                    })
                {
                    return Err("invalid mobile clipboard entries".into());
                }
                let removed = history
                    .remove_many(&texts)
                    .map_err(|_| "mobile clipboard removal failed")?;
                Ok(
                    json!({"removed": removed, "limit": history.limit(), "entries": history.entries()}),
                )
            }
            MobileClipboardAction::RestoreMany { entries } => {
                if entries.len() > msime_client_core::clipboard::MAX_ENTRIES {
                    return Err("invalid mobile clipboard entries".into());
                }
                history
                    .restore_many(&entries)
                    .map_err(|_| "mobile clipboard restore failed")?;
                Ok(
                    json!({"restored": true, "limit": history.limit(), "entries": history.entries()}),
                )
            }
            MobileClipboardAction::Replace { text, replacement } => {
                if text.is_empty()
                    || text.len() > msime_client_core::clipboard::MAX_MOBILE_TEXT_BYTES
                {
                    return Err("invalid mobile clipboard entry".into());
                }
                use msime_client_core::clipboard::ReplaceOutcome;
                let outcome = history
                    .replace(&text, replacement)
                    .map_err(|_| "mobile clipboard replace failed")?;
                // 改成已存在的文字时两条合并成一条，宿主据 merged 告诉用户；旧条目不在了或新文字不合规时 replaced 为 false，reason 说明是哪一种。
                let reason = match outcome {
                    ReplaceOutcome::NotFound => Some("not_found"),
                    ReplaceOutcome::Invalid => Some("invalid"),
                    ReplaceOutcome::Replaced
                    | ReplaceOutcome::Merged
                    | ReplaceOutcome::Unchanged => None,
                };
                Ok(json!({
                    "replaced": reason.is_none(),
                    "merged": outcome == ReplaceOutcome::Merged,
                    "reason": reason,
                    "limit": history.limit(),
                    "entries": history.entries()
                }))
            }
            MobileClipboardAction::Clear => {
                history
                    .clear()
                    .map_err(|_| "mobile clipboard clear failed")?;
                Ok(json!({"cleared": true, "limit": history.limit(), "entries": []}))
            }
        }
    })
}

/// Save host-sampled text only while shared history is enabled.
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_capture_clipboard_history(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 131072 {
            return Err("invalid history capture buffer".into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Capture {
            directory: String,
            text: String,
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let capture: Capture =
            serde_json::from_slice(bytes).map_err(|_| "invalid history capture document")?;
        if !std::path::Path::new(&capture.directory).is_absolute()
            || capture.directory.len() > 16384
            || capture.text.len() > msime_client_core::clipboard::MAX_TEXT_BYTES
        {
            return Err("invalid history capture parameters".into());
        }
        let captured = PreferencesStore::new(&capture.directory)
            .capture_clipboard_text(capture.text)
            .map_err(|_| "clipboard history capture failed")?;
        Ok(json!({"captured": captured}))
    })
}

/// Remove one saved history entry by exact content, without touching the clipboard.
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_remove_clipboard_history(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if request.is_null() || length > 131072 {
            return Err("invalid history removal buffer".into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Removal {
            directory: String,
            text: String,
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let removal: Removal =
            serde_json::from_slice(bytes).map_err(|_| "invalid history removal document")?;
        let path = std::path::Path::new(&removal.directory);
        if !path.is_absolute() || removal.directory.len() > 16384 {
            return Err("invalid history directory".into());
        }
        if removal.text.is_empty()
            || removal.text.len() > msime_client_core::clipboard::MAX_TEXT_BYTES
        {
            return Err("invalid history entry".into());
        }
        if !PreferencesStore::new(path)
            .load_for_runtime()
            .map_err(|_| "history preferences unavailable")?
            .snapshot
            .preferences
            .clipboard_history
        {
            return Err("clipboard history disabled".into());
        }
        let mut history = msime_client_core::clipboard::ClipboardHistoryStore::open_with_limit(
            path.join("clipboard_history.json"),
            clipboard_history_limit(path),
        );
        let removed = history
            .remove(&removal.text)
            .map_err(|_| "clipboard history removal failed")?;
        Ok(json!({"removed": removed}))
    })
}

const MAX_DESKTOP_CLIPBOARD_REQUEST_BYTES: usize = 131_072;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DesktopClipboardRequest {
    directory: String,
    action: DesktopClipboardAction,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum DesktopClipboardAction {
    List,
    Sync,
    Clear,
    SetPinned { text: String, pinned: bool },
    Remove { text: String },
}

/// The desktop clipboard history as the 剪贴板 settings page manages it, with the timestamps and pins `msime_client_load_clipboard_history` leaves out. Every operation answers with the history as it now stands, empty while the preference is off; see the header for the operations and codes.
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_desktop_clipboard_history(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use crate::clipboard_history::{self, ClipboardHistoryError};
    use msime_client_core::clipboard::{ClipboardHistoryStore, MAX_TEXT_BYTES};
    response(|| {
        const INVALID: &str = "clipboard_history_invalid";
        const UNAVAILABLE: &str = "clipboard_history_unavailable";
        if request.is_null() || length > MAX_DESKTOP_CLIPBOARD_REQUEST_BYTES {
            return Err(INVALID.into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: DesktopClipboardRequest =
            serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        let directory = std::path::Path::new(&request.directory);
        if !directory.is_absolute() || request.directory.len() > 16384 {
            return Err(INVALID.into());
        }
        let failed = |error: ClipboardHistoryError| match error {
            ClipboardHistoryError::Disabled => "clipboard_history_disabled".to_owned(),
            ClipboardHistoryError::Unavailable => UNAVAILABLE.to_owned(),
        };
        let entry = |text: &str| {
            if text.is_empty() || text.len() > MAX_TEXT_BYTES {
                Err(INVALID.to_owned())
            } else {
                Ok(())
            }
        };
        let preferences = PreferencesStore::new(directory);
        let limit = clipboard_history_limit(directory);
        let mut history =
            ClipboardHistoryStore::open_with_limit(directory.join("clipboard_history.json"), limit);
        match &request.action {
            DesktopClipboardAction::List => {}
            DesktopClipboardAction::Sync => {
                #[cfg(windows)]
                let read = clipboard_history::windows_clipboard_text;
                // Only Windows reads its clipboard here; the other desktops sync in their own shells.
                #[cfg(not(windows))]
                let read = || Err::<Option<String>, ()>(());
                clipboard_history::capture_clipboard(&preferences, read).map_err(failed)?;
            }
            DesktopClipboardAction::Clear => history.clear().map_err(|_| UNAVAILABLE)?,
            DesktopClipboardAction::SetPinned { text, pinned } => {
                entry(text)?;
                history.set_pinned(text, *pinned).map_err(|_| UNAVAILABLE)?;
            }
            DesktopClipboardAction::Remove { text } => {
                entry(text)?;
                history.remove(text).map_err(|_| UNAVAILABLE)?;
            }
        }
        let enabled = clipboard_history::enabled(&preferences).map_err(failed)?;
        let entries = if enabled {
            clipboard_history::entries(&mut history).map_err(failed)?
        } else {
            Vec::new()
        };
        Ok(json!({ "enabled": enabled, "entries": entries }))
    })
}

/// 宿主给的云剪贴板设备名称：账号路径和中继路径同一条规则。没给、只有空白或超过 256 字节（含控制字符）时是 `None`，调用方改用计算机名或平台名；驱动登记前还会再清理一遍。
fn host_device_name(name: Option<String>) -> Option<String> {
    name.filter(|name| !name.trim().is_empty() && is_bounded_text(name, 256))
}

/// One cloud clipboard action over the signed-in account session stored in `directory`, for a settings window that holds no session of its own. Blocks on the network; see the header.
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_account_cloud_clipboard(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::account::{
        AccountSessionFileLayout, BackendAccountClient, BackendAccountSession,
        FileAccountSessionStorage,
    };
    response(|| {
        const INVALID: &str = "invalid_cloud_clipboard";
        if request.is_null() || length > MAX_DESKTOP_CLIPBOARD_REQUEST_BYTES {
            return Err(INVALID.into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct CloudRequest {
            #[serde(default)]
            directory: Option<String>,
            #[serde(default)]
            session_store: Option<u64>,
            #[serde(default)]
            preferences_directory: Option<String>,
            /// 端到端加密登记本机时给其他设备看的名称（#8201 的 P6，例如鸿蒙的 marketName），规则与中继路径相同，见 [`host_device_name`]。
            #[serde(default)]
            device_name: Option<String>,
            /// 为 `false` 时只用本机已有的设备身份，见头文件。
            #[serde(default)]
            create_identity: Option<bool>,
            action: Value,
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: CloudRequest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        let source = super::session_store::session_source(
            request.directory.as_deref(),
            false,
            request.session_store,
        )
        .ok_or(INVALID)?;
        if let super::session_store::Source::Directory(directory) = &source {
            if !std::path::Path::new(directory).is_absolute() || directory.len() > 16384 {
                return Err(INVALID.into());
            }
        }
        crate::cloud_clipboard::validate_account_request(&request.action).map_err(|_| INVALID)?;
        if let Some(directory) = &request.preferences_directory {
            if directory.len() > 16384 || !std::path::Path::new(directory).is_absolute() {
                return Err(INVALID.into());
            }
            if crate::cloud_clipboard::configured_relay(std::path::Path::new(directory))
                .map_err(str::to_owned)?
                .is_some()
            {
                return Err("clipboard_source_changed".into());
            }
        }
        let device_name = host_device_name(request.device_name);
        let existing_identity = request.create_identity == Some(false);
        match source {
            super::session_store::Source::Directory(directory) => {
                let client =
                    BackendAccountClient::new().map_err(|error| error.code().to_owned())?;
                let session = BackendAccountSession::new(
                    client,
                    FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native),
                );
                if existing_identity {
                    crate::cloud_clipboard::account_request_existing_identity(
                        &session,
                        &request.action,
                        device_name.as_deref(),
                    )
                } else {
                    crate::cloud_clipboard::account_request(
                        &session,
                        &request.action,
                        device_name.as_deref(),
                    )
                }
            }
            // 注入的存储用注册时建好的会话对象，不每次新建。
            super::session_store::Source::Store(entry) if existing_identity => {
                crate::cloud_clipboard::account_request_existing_identity(
                    &entry.account,
                    &request.action,
                    device_name.as_deref(),
                )
            }
            super::session_store::Source::Store(entry) => crate::cloud_clipboard::account_request(
                &entry.account,
                &request.action,
                device_name.as_deref(),
            ),
        }
        .map_err(str::to_owned)
    })
}

/// 经自建云剪贴板中继答一个动作；中继配置在偏好里。会联网，格式见头文件。
/// # Safety
/// `request` 指向 `length` 个可读的 JSON 字节，空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_relay_cloud_clipboard(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        const INVALID: &str = "invalid_cloud_clipboard";
        if request.is_null() || length > MAX_DESKTOP_CLIPBOARD_REQUEST_BYTES {
            return Err(INVALID.into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RelayRequest {
            preferences_directory: String,
            #[serde(default)]
            expected_revision: Option<u64>,
            directory: String,
            #[serde(default)]
            device_name: Option<String>,
            /// 为 `false` 时只用 `directory` 里已有的设备身份，见头文件。
            #[serde(default)]
            create_identity: Option<bool>,
            action: Value,
        }
        // SAFETY: 由文档写明的调用方约定保证。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: RelayRequest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        let absolute = |path: &str| path.len() <= 16384 && std::path::Path::new(path).is_absolute();
        if !absolute(&request.preferences_directory) || !absolute(&request.directory) {
            return Err(INVALID.into());
        }
        crate::cloud_clipboard::validate_relay_request(&request.action).map_err(|_| INVALID)?;
        let config = crate::cloud_clipboard::configured_relay_at_revision(
            std::path::Path::new(&request.preferences_directory),
            request.expected_revision,
        )
        .map_err(str::to_owned)?
        .0
        .ok_or("relay_not_configured")?;
        let device_name = host_device_name(request.device_name)
            .unwrap_or_else(msime_client_services::account::cloud_clipboard_device_name);
        let directory = std::path::Path::new(&request.directory);
        if request.create_identity == Some(false) {
            crate::cloud_clipboard::relay_request_existing_identity(
                config,
                directory,
                &device_name,
                &request.action,
            )
        } else {
            crate::cloud_clipboard::relay_request(config, directory, &device_name, &request.action)
        }
        .map_err(str::to_owned)
    })
}

/// 向宿主给出的中继（HTTP 中继，或者 `backend` 为 `webdav` 时的 WebDAV 网盘）发端到端加密驱动给出的一个请求，答 `{status, body}`。不读偏好、不读写密钥串，会联网，格式见头文件。
/// # Safety
/// `request` 指向 `length` 个可读的 JSON 字节，空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_relay_cloud_clipboard_exchange(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        const INVALID: &str = "invalid_cloud_clipboard";
        if request.is_null() || length > MAX_DESKTOP_CLIPBOARD_REQUEST_BYTES {
            return Err(INVALID.into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ExchangeRequest {
            /// `relay`（缺省）或 `webdav`。
            #[serde(default)]
            backend: Option<String>,
            url: String,
            #[serde(default)]
            token: Option<String>,
            #[serde(default)]
            username: Option<String>,
            #[serde(default)]
            password: Option<String>,
            /// WebDAV 后端放日志缓存和锁文件的目录（绝对路径）。
            #[serde(default)]
            directory: Option<String>,
            method: String,
            path: String,
            #[serde(default)]
            body: Option<String>,
        }
        // SAFETY: 由文档写明的调用方约定保证。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: ExchangeRequest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        use msime_client_services::clipboard_relay::RelayConfig;
        // 地址和凭据按偏好里的同一套规则校验：不是 HTTPS（或本机回环的 HTTP）就不发。每种后端只收它自己的字段。
        let (config, directory) = match request.backend.as_deref().unwrap_or("relay") {
            "relay" => {
                let (Some(token), None, None, None) = (
                    &request.token,
                    &request.username,
                    &request.password,
                    &request.directory,
                ) else {
                    return Err(INVALID.into());
                };
                let config = RelayConfig::new(&request.url, token)
                    .map_err(|error| error.code().to_owned())?;
                (config, None)
            }
            "webdav" => {
                let (None, Some(username), Some(password), Some(directory)) = (
                    &request.token,
                    &request.username,
                    &request.password,
                    &request.directory,
                ) else {
                    return Err(INVALID.into());
                };
                if directory.len() > 16384 || !std::path::Path::new(directory).is_absolute() {
                    return Err(INVALID.into());
                }
                let config = RelayConfig::webdav(&request.url, username, password)
                    .map_err(|error| error.code().to_owned())?;
                (config, Some(std::path::PathBuf::from(directory)))
            }
            _ => return Err(INVALID.into()),
        };
        crate::cloud_clipboard::relay_exchange(
            config,
            directory.as_deref(),
            &request.method,
            &request.path,
            request.body.as_deref(),
        )
        .map_err(str::to_owned)
    })
}

/// WebDAV 设置的「测试连接」，格式见头文件。会联网。
/// # Safety
/// `request` 指向 `length` 个可读的 JSON 字节，空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_clipboard_webdav_probe(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        const INVALID: &str = "invalid_cloud_clipboard";
        if request.is_null() || length > MAX_DESKTOP_CLIPBOARD_REQUEST_BYTES {
            return Err(INVALID.into());
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct ProbeRequest {
            url: String,
            username: String,
            password: String,
            #[serde(default)]
            directory: Option<String>,
        }
        // SAFETY: 由文档写明的调用方约定保证。
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: ProbeRequest = serde_json::from_slice(bytes).map_err(|_| INVALID)?;
        if request.directory.as_deref().is_some_and(|directory| {
            directory.len() > 16384 || !std::path::Path::new(directory).is_absolute()
        }) {
            return Err(INVALID.into());
        }
        crate::cloud_clipboard::webdav_probe(
            &request.url,
            &request.username,
            &request.password,
            request.directory.as_deref().map(std::path::Path::new),
        )
        .map_err(str::to_owned)
    })
}

/// 云剪贴板该走哪条路：偏好里配置了中继或 WebDAV 网盘时是 `relay`（`backend` 说明是哪一种），否则是 `account`。不联网。
/// # Safety
/// `directory` 必须指向 `length` 个可读字节，空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_cloud_clipboard_source(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        // SAFETY: 由文档写明的调用方约定保证。
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = super::parse_absolute_path(
            bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let (config, revision) = crate::cloud_clipboard::configured_relay_at_revision(
            std::path::Path::new(directory),
            None,
        )
        .map_err(str::to_owned)?;
        Ok(match config {
            Some(config) => {
                json!({ "source": "relay", "backend": config.backend(), "revision": revision })
            }
            None => json!({ "source": "account", "revision": revision }),
        })
    })
}

/// Try to read preferences without waiting for the writer lock.
/// # Safety
/// `directory` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_try_load_preferences(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = super::parse_absolute_path(
            bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let snapshot = PreferencesStore::new(directory)
            .try_load()
            .map_err(|e| e.to_string())?;
        serde_json::to_value(snapshot).map_err(|e| e.to_string())
    })
}

/// Repair a preferences document that is not well-formed JSON, backing it up first.
/// See `PreferencesStore::recover_malformed`; a valid or missing document is left untouched.
/// # Safety
/// `directory` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_recover_preferences(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    // SAFETY: forwarded under the same caller contract.
    unsafe { recover_preferences_in(directory, length, RecoveryScope::Malformed) }
}

/// The settings page's explicit 修复配置文件: repair a preferences document `load` rejects for any reason, a newer build's included, backing it up first.
/// See `PreferencesStore::recover`; a valid or missing document is left untouched. Only call it on the user's request: a document an older build cannot read is usually a newer build's, and the repair keeps only what this schema accepts.
/// # Safety
/// `directory` must point to `length` readable bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_repair_preferences(
    directory: *const u8,
    length: usize,
) -> *mut c_char {
    // SAFETY: forwarded under the same caller contract.
    unsafe { recover_preferences_in(directory, length, RecoveryScope::Unreadable) }
}

#[derive(Clone, Copy)]
enum RecoveryScope {
    /// Only a document that is not well-formed JSON; what input method hosts may do on their own.
    Malformed,
    /// Anything `load` rejects; only on the user's request.
    Unreadable,
}

/// # Safety
/// `directory` must point to `length` readable bytes. Null is rejected.
unsafe fn recover_preferences_in(
    directory: *const u8,
    length: usize,
    scope: RecoveryScope,
) -> *mut c_char {
    response(|| {
        if directory.is_null() || length > 16384 {
            return Err("invalid preferences directory buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(directory, length) };
        let directory = super::parse_absolute_path(
            bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let store = PreferencesStore::new(directory);
        let outcome = match scope {
            RecoveryScope::Malformed => store.recover_malformed(),
            RecoveryScope::Unreadable => store.recover(),
        }
        .map_err(|e| e.to_string())?;
        Ok(match outcome {
            msime_client_core::preferences::RecoveryOutcome::NotNeeded(snapshot) => json!({
                "recovered": false,
                "snapshot": snapshot,
            }),
            msime_client_core::preferences::RecoveryOutcome::Recovered {
                snapshot,
                backup_path,
                salvaged,
            } => json!({
                "recovered": true,
                "snapshot": snapshot,
                "backup_path": backup_path.to_string_lossy(),
                "backup_name": backup_path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                "salvaged": salvaged,
            }),
        })
    })
}

/// Compare-and-swap save for a validated PreferencesSnapshot.
/// A successful save with clipboard history disabled clears the default history
/// under the shared preference/history locks, matching the desktop settings path.
///
/// # Safety
/// The caller must provide non-null pointers to readable UTF-8 buffers whose
/// lengths match the supplied lengths and remain valid for the duration of the call.
#[no_mangle]
pub unsafe extern "C" fn msime_client_save_preferences(
    directory: *const u8,
    directory_length: usize,
    expected_revision: u64,
    snapshot: *const u8,
    snapshot_length: usize,
) -> *mut c_char {
    response(|| {
        if directory.is_null()
            || snapshot.is_null()
            || directory_length > 16384
            || snapshot_length > PREFERENCES_DOCUMENT_LIMIT
        {
            return Err("invalid preferences save buffer".into());
        }
        let directory_bytes = unsafe { std::slice::from_raw_parts(directory, directory_length) };
        let directory = super::parse_absolute_path(
            directory_bytes,
            "invalid preferences directory encoding",
            "preferences directory must be absolute",
        )?;
        let snapshot_bytes = unsafe { std::slice::from_raw_parts(snapshot, snapshot_length) };
        // 提交来的偏好严格解析（Preferences::from_submission），不走读盘时的宽松读法：宿主送来本版本不认识的名字是宿主的毛病，要报出来，而不是悄悄整理掉再存下。
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Submitted {
            format_version: u32,
            #[allow(dead_code)]
            revision: u64,
            preferences: Value,
        }
        let snapshot: Submitted =
            serde_json::from_slice(snapshot_bytes).map_err(|_| "invalid preferences snapshot")?;
        if snapshot.format_version != 1 {
            return Err("unsupported preferences format".into());
        }
        let preferences =
            Preferences::from_submission(snapshot.preferences).map_err(|error| match error {
                msime_client_core::preferences::PreferencesError::Json(_) => {
                    "invalid preferences snapshot".to_owned()
                }
                other => other.to_string(),
            })?;
        let store = PreferencesStore::new(directory);
        let saved = store
            .save(expected_revision, preferences)
            .map_err(|e| e.to_string())?;
        if !saved.preferences.clipboard_history {
            store
                .clear_disabled_clipboard_history()
                .map_err(|e| e.to_string())?;
        }
        serde_json::to_value(saved).map_err(|e| e.to_string())
    })
}

/// Read or update 背单词 wordbooks and review progress.
///
/// 只做转接：请求解析成共享的动作类型，应答是序列化后的共享状态。每条规则（动作做什么、复习队列怎么排、哪天算今天）都在 `msime_client_services::vocabulary::session`，所以这里和 Tauri 命令层不会各说各的。
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_vocabulary_review(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::vocabulary::session;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        /// The verified resource directory, whose `wordbooks/` sibling holds the bundled books.
        /// A host that stages none simply offers the imported ones.
        resources: String,
        /// The caller's local day. Required by every action, because the counts and the queue are
        /// both per-day and this layer cannot resolve the host's timezone.
        day: String,
        action: session::ReviewAction,
        /// 插件目录的绝对路径：其中的单词本插件作为 `pack-<插件 id>` 词书列出。不传的宿主（Android、iOS）只有内置和导入的书。
        #[serde(default)]
        plugins: Option<String>,
    }

    response(|| {
        // 文件本身仍由共享解析器限制为 8 MiB；请求另给常规 JSON 转义的两倍空间及目录等元数据余量。
        const MAX_REQUEST_BYTES: usize = 17 * 1024 * 1024;
        if request.is_null() || length > MAX_REQUEST_BYTES {
            return Err("invalid vocabulary review buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid vocabulary review request")?;
        if request.directory.len() > 16_384
            || !std::path::Path::new(&request.directory).is_absolute()
            || request.resources.len() > 16_384
            || !std::path::Path::new(&request.resources).is_absolute()
        {
            return Err("invalid vocabulary review directory".into());
        }
        if request.plugins.as_deref().is_some_and(|plugins| {
            plugins.len() > 16_384 || !std::path::Path::new(plugins).is_absolute()
        }) {
            return Err("invalid vocabulary review directory".into());
        }
        let status = session::apply(
            std::path::Path::new(&request.directory),
            std::path::Path::new(&request.resources),
            request.plugins.as_deref().map(std::path::Path::new),
            &request.day,
            request.action,
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(status).map_err(|_| "vocabulary review response failed".to_owned())
    })
}

/// 候选窗的「加入生词本」：把一个候选的释义加进本机生词本。
///
/// 只做转接：释义怎么变成卡片、怎么合并、给用户看哪句提示都在 `msime_client_services::vocabulary::personal`。
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_personal_wordbook_add(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::vocabulary::personal;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        candidate: String,
        gloss: String,
        #[serde(default)]
        pronunciation: String,
    }

    response(|| {
        // 候选和释义都是一行显示文字，宿主自己也把它们限制在几 KiB 内。
        const MAX_REQUEST_BYTES: usize = 64 * 1024;
        if request.is_null() || length > MAX_REQUEST_BYTES {
            return Err("invalid personal wordbook buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid personal wordbook request")?;
        if request.directory.len() > 16_384
            || !std::path::Path::new(&request.directory).is_absolute()
        {
            return Err("invalid personal wordbook directory".into());
        }
        let reply = personal::add_gloss(
            std::path::Path::new(&request.directory),
            &request.candidate,
            &request.gloss,
            &request.pronunciation,
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(reply).map_err(|_| "personal wordbook response failed".to_owned())
    })
}

/// 候选菜单的「朗读」和背单词卡片的喇叭要读的英文：`{candidate, gloss}`，成功时 `value` 是要读的文字，没有能读的时是 `null`。
///
/// 只做转接，规则在 `msime_client_services::vocabulary::read_aloud`。不读写任何文件，宿主在打开菜单时调用。
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_read_aloud_text(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::vocabulary::read_aloud;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        candidate: String,
        #[serde(default)]
        gloss: String,
    }

    response(|| {
        // 和「加入生词本」一样，候选和释义都是一行显示文字。
        const MAX_REQUEST_BYTES: usize = 64 * 1024;
        if request.is_null() || length > MAX_REQUEST_BYTES {
            return Err("invalid read aloud buffer".into());
        }
        // SAFETY: guaranteed by the documented caller contract.
        let bytes = unsafe { std::slice::from_raw_parts(request, length) };
        let request: Request =
            serde_json::from_slice(bytes).map_err(|_| "invalid read aloud request")?;
        Ok(serde_json::Value::from(read_aloud::read_aloud_text(
            &request.candidate,
            &request.gloss,
        )))
    })
}

/// 今日待复习的张数，给菜单栏、托盘菜单这类被动显示的地方用。
///
/// 和 `msime_client_vocabulary_review` 应答里的 `due` 是同一个数，但只读进度文档和选中的那一本书（见 `msime_client_services::vocabulary::session::due_count`）。返回张数（不小于 0）；请求不合法、进度文档损坏或选中的书读不出来时返回 -1，宿主据此隐藏这一项。
///
/// 内置词书的资源根可以直接给 `resources`，也可以给整份 HostOptions 文档 `host_options`，由这里按 [`crate::bundled_resource_root`] 求出：Linux 的输入法宿主手里只有 HostOptions，词库在线更新后 `resources` 指向状态目录里的下载代次，那里没有词书，自己再写一遍这条规则就会和设置应用找错地方。两者恰好给一个。
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_vocabulary_due_count(
    request: *const u8,
    length: usize,
) -> i32 {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        #[serde(default)]
        resources: Option<String>,
        #[serde(default)]
        host_options: Option<Value>,
        day: String,
        #[serde(default)]
        plugins: Option<String>,
    }

    // 路径各不超过 16 KiB；HostOptions 文档带着偏好，留到 1 MiB。
    const MAX_REQUEST_BYTES: usize = 1024 * 1024;
    if request.is_null() || length > MAX_REQUEST_BYTES {
        return -1;
    }
    // SAFETY: guaranteed by the documented caller contract.
    let bytes = unsafe { std::slice::from_raw_parts(request, length) };
    let Ok(request) = serde_json::from_slice::<Request>(bytes) else {
        return -1;
    };
    let valid_path = |path: &str| path.len() <= 16_384 && std::path::Path::new(path).is_absolute();
    let resources = match (&request.resources, &request.host_options) {
        (Some(resources), None) => std::path::PathBuf::from(resources),
        (None, Some(document)) => match crate::bundled_resource_root(document) {
            Some(root) => root,
            None => return -1,
        },
        _ => return -1,
    };
    if !valid_path(&request.directory)
        || !resources.to_str().is_some_and(valid_path)
        || request
            .plugins
            .as_deref()
            .is_some_and(|plugins| !valid_path(plugins))
    {
        return -1;
    }
    match msime_client_services::vocabulary::session::due_count(
        std::path::Path::new(&request.directory),
        &resources,
        request.plugins.as_deref().map(std::path::Path::new),
        &request.day,
    ) {
        Ok(count) => i32::try_from(count).unwrap_or(i32::MAX),
        Err(_) => -1,
    }
}

/// 鸿蒙设置应用导入键盘发布的生词本：把键盘沙箱里那本并进这个目录的生词本。
///
/// 只做转接，规则在 `msime_client_services::vocabulary::personal::merge_published`。请求里只带文件路径，生词本文档由共享层按单本书的大小上限去读。
/// # Safety
/// `request` 指向 `length` 个可读的 JSON 字节，空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_personal_wordbook_merge(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::vocabulary::personal;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        published: String,
        #[serde(default)]
        previous: Option<String>,
    }

    response(|| {
        // SAFETY: 由上面写明的调用方约定保证。
        let request: Request = unsafe { personal_wordbook_request(request, length) }?;
        let previous = match request.previous.as_deref() {
            Some(path) => Some(personal_wordbook_path(path)?),
            None => None,
        };
        let report = personal::merge_published(
            personal_wordbook_path(&request.directory)?,
            personal_wordbook_path(&request.published)?,
            previous,
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(report).map_err(|_| "personal wordbook response failed".to_owned())
    })
}

/// 鸿蒙键盘在设置应用确认并过某一版以后，把那一版里的词从键盘自己的生词本里清走。
///
/// 只做转接，规则在 `msime_client_services::vocabulary::personal::drain_published`。
/// # Safety
/// `request` 指向 `length` 个可读的 JSON 字节，空指针会被拒绝。
#[no_mangle]
pub unsafe extern "C" fn msime_client_personal_wordbook_drain(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_services::vocabulary::personal;

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        directory: String,
        published: String,
    }

    response(|| {
        // SAFETY: 由上面写明的调用方约定保证。
        let request: Request = unsafe { personal_wordbook_request(request, length) }?;
        let removed = personal::drain_published(
            personal_wordbook_path(&request.directory)?,
            personal_wordbook_path(&request.published)?,
        )
        .map_err(|error| error.to_string())?;
        Ok(serde_json::json!({ "removed": removed }))
    })
}

/// 读生词本合并、清走两个入口的请求。请求里只有几条路径，64 KiB 足够。
///
/// # Safety
/// `request` 指向 `length` 个可读的字节，空指针会被拒绝。
unsafe fn personal_wordbook_request<T: serde::de::DeserializeOwned>(
    request: *const u8,
    length: usize,
) -> Result<T, String> {
    const MAX_REQUEST_BYTES: usize = 64 * 1024;
    if request.is_null() || length > MAX_REQUEST_BYTES {
        return Err("invalid personal wordbook buffer".into());
    }
    // SAFETY: 由调用方保证 `request` 指向 `length` 个可读的字节。
    let bytes = unsafe { std::slice::from_raw_parts(request, length) };
    serde_json::from_slice(bytes).map_err(|_| "invalid personal wordbook request".into())
}

/// 请求里的路径必须是绝对路径。
fn personal_wordbook_path(path: &str) -> Result<&std::path::Path, String> {
    if path.len() > 16_384 || !std::path::Path::new(path).is_absolute() {
        return Err("invalid personal wordbook path".into());
    }
    Ok(std::path::Path::new(path))
}

#[cfg(test)]
mod cloud_clipboard_tests;

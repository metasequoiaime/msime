//! Host configuration, preferences, the skin catalog and clipboard history.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;
use msime_client_core::is_bounded_text;

/// Candidate and commit text normally stays far below this bound. Keep the direct conversion ABI
/// bounded as well so a malformed native length cannot make it scan an unbounded buffer or allocate
/// an arbitrarily large converted string.
const MAX_TRADITIONAL_CONVERSION_BYTES: usize = 1 << 20;

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
        if !resources.is_absolute() || !state.is_absolute() {
            return Err("bootstrap paths must be absolute".into());
        }
        let document = prepare_host_configuration(resources, state).map_err(|e| e.to_string())?;
        serde_json::from_str(&document).map_err(|e| e.to_string())
    })
}

/// Re-prepare a published runtime options file whose working dictionaries belong to an older resource generation, as after a package upgrade. Returns whether the file was rewritten. Call before creating any session from that file.
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
        refresh_host_options(path)
            .map(Value::Bool)
            .map_err(|e| e.to_string())
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
        let provider = msime_client_core::voice::provider::mobile_voice_provider_configuration(
            &snapshot.preferences,
        )
        .map(|value| {
            json!({
                "provider": value.provider,
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
        let polish = msime_client_core::voice::provider::mobile_voice_polish_configuration(
            &snapshot.preferences,
        )
        .map(|value| {
            json!({
                "endpoint": value.endpoint,
                "model": value.model,
                "token": value.token,
                "promptId": value.prompt_id,
                "promptLegacy": value.prompt_legacy,
                "promptCustom1": value.prompt_custom_1,
                "promptCustom2": value.prompt_custom_2,
                "promptCustom3": value.prompt_custom_3,
            })
        });
        Ok(json!({ "provider": provider, "polish": polish }))
    })
}

/// The global theme picker: every theme id in picker order with its title and palettes, and the default id.
///
/// `system` and `custom` carry no palettes here: `system` is the host's native tokens, and `custom` is only known once resolved against the user's `custom_theme`. Hosts draw the picker from this and keep no copy of the ids, titles or colours.
#[no_mangle]
pub extern "C" fn msime_client_theme_catalog() -> *mut c_char {
    response(|| {
        Ok(serde_json::json!({
            "themes": msime_client_core::skin::theme::catalog(),
            "default": msime_client_core::skin::theme::GlobalTheme::default(),
        }))
    })
}

/// Largest resolve request. A custom keyboard design may carry a photo of up to 682,668 base64 bytes, and the rest of the request is small.
const MAX_THEME_REQUEST_BYTES: usize = 1 << 20;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveThemeRequest {
    global_theme: msime_client_core::skin::theme::GlobalTheme,
    #[serde(default)]
    custom_theme: msime_client_core::preferences::CustomTheme,
    dark: bool,
    layout: msime_client_core::preferences::CandidateLayout,
    skins_directory: Option<String>,
    package: Option<serde_json::Value>,
}

/// Resolve the colours a host draws for a global theme.
///
/// The request carries the two preference fields as the host read them, so this takes no preferences directory and does no preference I/O: `global_theme` (one of the seven ids; any other id, a retired skin id included, fails the request as `invalid theme request`) and `custom_theme` (optional, validated as strictly as the preference itself). `dark` is the host's effective mode for the surface being drawn, and `layout` (`horizontal` or `vertical`) its candidate layout: a package is drawn only in a layout and a mode its manifest declares, so no host keeps a gate of its own. For `custom` with a `custom_theme.candidate_skin`, the host names where that package comes from with at most one of `skins_directory` (an absolute skin root; the package is loaded and validated as `msime_client_skin_package` does; every host that scans the root, which is every host but Linux) or `package` (one entry of the published `candidate_skin_catalog`, which only the Linux hosts read). A `package` that is not such an entry, a `SkinSummary` from `msime_client_skin_catalog` included, fails the call: that is a host bug, and reading it anyway would drop its declared modes and selection bar without a word. A package that is missing from the root, invalid on disk or not the one `candidate_skin` names is left out rather than failing the call: the theme still resolves, over its base.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
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
        let wanted = request
            .custom_theme
            .candidate_skin
            .as_deref()
            .filter(|_| theme == msime_client_core::skin::theme::GlobalTheme::Custom);
        let entry = request
            .package
            .map(msime_client_core::skin::theme::ThemePackage::from_host_catalog_entry)
            .transpose()?;
        let package = match (wanted, request.skins_directory, entry) {
            (Some(id), Some(directory), _) => {
                msime_client_core::skin::catalog::load_package(&directory, id)
                    .ok()
                    .map(|summary| msime_client_core::skin::theme::ThemePackage::from(&summary))
            }
            (Some(_), None, entry) => entry,
            _ => None,
        };
        let resolved = msime_client_core::skin::theme::resolve(
            theme,
            &request.custom_theme,
            request.dark,
            request.layout,
            package.as_ref(),
        );
        serde_json::to_value(resolved).map_err(|error| error.to_string())
    })
}

/// Per-key double-pinyin hint text for one profile, read out of the Engine's own
/// profile tables.
///
/// A touch keyboard labels its letter keys with the units they carry, and a face
/// that keeps its own copy of that keymap drifts from the scheme the session
/// actually runs. The hints depend only on the profile, not on session state, so
/// this takes no handle. An unknown name yields an empty object rather than the
/// default profile's hints: labelling the keys with a scheme the session is not
/// running is worse than labelling nothing.
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
        let hints: serde_json::Map<String, serde_json::Value> =
            msime_engine_bridge::shuangpin_key_hints(name)
                .into_iter()
                .map(|entry| (entry.key, serde_json::Value::String(entry.hint)))
                .collect();
        Ok(serde_json::Value::Object(hints))
    })
}

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
        let snapshot = PreferencesStore::new(directory)
            .load()
            .map_err(|e| e.to_string())?;
        serde_json::to_value(snapshot).map_err(|e| e.to_string())
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
        },
        SetEnabled {
            enabled: bool,
        },
        SetRetention {
            retention: String,
            /// The caller's local day, for the same reason `record` takes one: only the host
            /// knows which day the window is counted back from.
            day: String,
        },
        Reset,
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
        match request.action {
            StatisticsAction::Load => {
                serde_json::to_value(store.load().map_err(|error| error.to_string())?)
                    .map_err(|_| "typing statistics response failed".to_owned())
            }
            StatisticsAction::Record {
                text,
                source,
                day,
                hour,
            } => {
                let recorded = store
                    .record(&text, source, &day, hour)
                    .map_err(|error| error.to_string())?;
                Ok(json!({"recorded": recorded}))
            }
            StatisticsAction::SetEnabled { enabled } => serde_json::to_value(
                store
                    .set_enabled(enabled)
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|_| "typing statistics response failed".to_owned()),
            StatisticsAction::SetRetention { retention, day } => serde_json::to_value(
                store
                    .set_retention(
                        msime_client_core::typing_statistics::StatisticsRetention::parse(
                            &retention,
                        ),
                        &day,
                    )
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|_| "typing statistics response failed".to_owned()),
            StatisticsAction::Reset => {
                serde_json::to_value(store.reset().map_err(|error| error.to_string())?)
                    .map_err(|_| "typing statistics response failed".to_owned())
            }
        }
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
        serde_json::to_value(msime_client_core::skin::catalog::scan(directory))
            .map_err(|e| e.to_string())
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
            msime_client_core::skin::catalog::load_package(&request.directory, &request.id)?;
        serde_json::to_value(package).map_err(|e| e.to_string())
    })
}

#[derive(Debug, Deserialize)]
struct SkinResourceRequest {
    directory: String,
    id: String,
    relative: String,
    kind: String,
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
        if !Path::new(&request.directory).is_absolute() {
            return Err("skin directory must be absolute".into());
        }
        let resource = msime_client_core::skin::catalog::read_resource(
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
        let stylesheet = msime_client_core::skin::catalog::read_toolbar_stylesheet(
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
        let id = msime_client_core::skin::folder_import::import(
            Path::new(&request.source),
            Path::new(&request.directory),
        )
        .map_err(str::to_owned)?;
        Ok(json!({ "id": id }))
    })
}

#[derive(Debug, Deserialize)]
struct CustomSkinLibraryRequest {
    directory: String,
    action: Option<msime_client_core::skin::custom_library::CustomSkinLibraryAction>,
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
        let store = msime_client_core::skin::custom_library::CustomSkinLibraryStore::new(
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

/// The codes the shared community pages already have a sentence for. A host that
/// forwarded the `Display` text instead would put an English sentence written
/// for a log into a Chinese dialog.
fn custom_skin_library_code(
    error: msime_client_core::skin::custom_library::CustomSkinLibraryError,
) -> String {
    use msime_client_core::skin::custom_library::CustomSkinLibraryError as Failure;
    match error {
        Failure::Full => "community_skin_library_full",
        Failure::InvalidName => "community_skin_invalid_name",
        Failure::DuplicateName => "community_skin_duplicate_name",
        Failure::NotFound => "community_not_found",
        Failure::Json(_) | Failure::Invalid => "community_skin_library_format",
        Failure::Io(_) => "community_storage",
    }
    .to_owned()
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
    skin: msime_client_core::skin::custom_library::SavedTouchKeyboardSkin,
    trial: msime_client_core::skin::keyboard_trial::KeyboardSkinTrial,
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
        let library = msime_client_core::skin::custom_library::CustomSkinLibraryStore::new(
            &request.directory,
        );
        let trials = msime_client_core::skin::keyboard_trial::KeyboardSkinTrialStore::new(
            &request.directory,
            std::sync::Arc::new(msime_client_core::preferences::PreferencesStore::new(
                &request.directory,
            )),
        );
        let (trial, _) = trials
            .begin(&request.name, request.design.clone())
            .map_err(keyboard_skin_trial_code)?;
        let skin = match library.import_download(id, &request.name, request.design) {
            Ok(skin) => skin,
            Err(error) => {
                let _ = trials.finish(trial.id, false);
                return Err(custom_skin_library_code(error));
            }
        };
        serde_json::to_value(CommunitySkinInstallResponse { skin, trial })
            .map_err(|_| "community_storage".into())
    })
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
        let trials = msime_client_core::skin::keyboard_trial::KeyboardSkinTrialStore::new(
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
    error: msime_client_core::skin::keyboard_trial::KeyboardSkinTrialError,
) -> String {
    use msime_client_core::skin::keyboard_trial::KeyboardSkinTrialError as Failure;
    match error {
        Failure::Io(_) | Failure::Preferences(_) => "community_storage",
        Failure::Json(_) | Failure::Invalid => "community_trial_format",
    }
    .to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operation")]
enum CommunityResourceLibraryAction {
    Load,
    SaveReply {
        item: Box<msime_client_core::community::resource::CommunityResource>,
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
            msime_client_core::community::resource_library::CommunityResourceLibraryStore::new(
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
    error: msime_client_core::community::resource_library::CommunityResourceLibraryError,
) -> String {
    use msime_client_core::community::resource_library::CommunityResourceLibraryError as Failure;
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
        artwork: msime_client_core::skin::ai::AiSkinArtwork,
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
                                "content": msime_client_core::skin::ai::AI_SKIN_SYSTEM_PROMPT,
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
                let plans = msime_client_core::skin::ai::plan_ai_skins(&text)
                    .map_err(|_| "ai_skin_response")?;
                serde_json::to_value(plans).map_err(|_| "ai_skin_response".into())
            }
            AiSkinPlanRequest::Artwork { artwork } => {
                msime_client_core::skin::ai::validate_ai_skin_artwork(&artwork)
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
        let file = path.join("dictionary-manifest.json");
        let bytes = crate::bounded_file::read(
            std::fs::File::open(&file).map_err(|_| "dictionary_manifest_unavailable")?,
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
        let enabled = PreferencesStore::new(path)
            .load()
            .map_err(|_| "history preferences unavailable")?
            .preferences
            .clipboard_history;
        if !enabled {
            return Ok(serde_json::json!({"enabled": false, "entries": []}));
        }
        let mut history = msime_client_core::clipboard::ClipboardHistoryStore::open(
            path.join("clipboard_history.json"),
        );
        history
            .load()
            .map_err(|_| "clipboard history unavailable")?;
        let entries: Vec<_> = history
            .entries()
            .iter()
            .map(|entry| entry.text.as_str())
            .collect();
        // Keep the existing ABI shape until native hosts opt into the
        // structured history bridge in their platform-specific migrations.
        Ok(serde_json::json!({"enabled": true, "entries": entries}))
    })
}

const MAX_MOBILE_CLIPBOARD_REQUEST_BYTES: usize = 524_288;
const MAX_APPLE_LEGACY_CLIPBOARD_BYTES: u64 = 4_000_000;
const APPLE_REFERENCE_DATE_UNIX_SECONDS: f64 = 978_307_200.0;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MobileClipboardRequest {
    directory: String,
    #[serde(default)]
    legacy: Option<MobileClipboardLegacy>,
    action: MobileClipboardAction,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MobileClipboardLegacy {
    HarmonyState,
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum MobileClipboardAction {
    Load,
    Capture { text: String },
    SetPinned { text: String, pinned: bool },
    Remove { text: String },
    Clear,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppleLegacyClipboardEntry {
    id: String,
    text: String,
    date: f64,
    pinned: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HarmonyLegacyClipboardEntry {
    text: String,
    at: f64,
    pinned: bool,
}

fn apple_date_to_unix_ms(value: f64) -> Option<u64> {
    let milliseconds = (value + APPLE_REFERENCE_DATE_UNIX_SECONDS) * 1000.0;
    (milliseconds.is_finite() && milliseconds >= 0.0 && milliseconds <= u64::MAX as f64)
        .then(|| milliseconds.round() as u64)
}

fn apple_clipboard_migration_lock(root: &std::path::Path) -> Result<std::fs::File, String> {
    std::fs::create_dir_all(root).map_err(|_| "clipboard migration unavailable")?;
    let lock_path = root.join(".msime-clipboard-history-migration.lock");
    let lock = msime_client_core::file_lock::open_private_lock_file(lock_path)
        .map_err(|_| "clipboard migration unavailable")?;
    // Not `File::lock`: std has no implementation of it on Android, so it fails outright there and
    // takes every shared clipboard operation with it. `client-core` already owns the per-target
    // answer, and this is the only place in the workspace that had its own.
    msime_client_core::file_lock::exclusive(&lock)
        .map_err(|_| "clipboard migration unavailable")?;
    Ok(lock)
}

/// Migrate the fixed legacy Apple history into the shared mobile state once.
/// The source is removed only after the destination has been persisted.
pub fn migrate_apple_clipboard_history(root: &std::path::Path) -> Result<bool, String> {
    let _lock = apple_clipboard_migration_lock(root)?;

    let shared_path = root.join("MSIME").join("clipboard_history.json");
    let mut shared = msime_client_core::clipboard::ClipboardHistoryStore::open(&shared_path);
    shared
        .load()
        .map_err(|_| "shared clipboard history unavailable")?;
    if !shared.entries().is_empty() {
        return Ok(false);
    }

    let legacy_path = root.join("Clipboard").join("history.json");
    let metadata = match std::fs::symlink_metadata(&legacy_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("legacy clipboard history unavailable".into()),
    };
    if !metadata.file_type().is_file() {
        return Err("invalid legacy clipboard history".into());
    }
    let bytes = crate::bounded_file::read(
        std::fs::File::open(&legacy_path).map_err(|_| "legacy clipboard history unavailable")?,
        MAX_APPLE_LEGACY_CLIPBOARD_BYTES,
    )
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            "invalid legacy clipboard history"
        } else {
            "legacy clipboard history unavailable"
        }
    })?;
    let legacy: Vec<AppleLegacyClipboardEntry> =
        serde_json::from_slice(&bytes).map_err(|_| "invalid legacy clipboard history")?;
    if legacy.len() > 50 {
        return Err("invalid legacy clipboard history".into());
    }
    let mut ids = std::collections::HashSet::new();
    let mut texts = std::collections::HashSet::new();
    let mut entries = Vec::with_capacity(legacy.len());
    for entry in legacy {
        let Some(timestamp_ms) = apple_date_to_unix_ms(entry.date) else {
            return Err("invalid legacy clipboard history".into());
        };
        if !crate::valid_uuid_string(&entry.id)
            || !ids.insert(entry.id)
            || !texts.insert(entry.text.clone())
            || !msime_client_core::clipboard::mobile_text_is_valid(&entry.text)
        {
            return Err("invalid legacy clipboard history".into());
        }
        entries.push(msime_client_core::clipboard::ClipboardHistoryEntry {
            text: entry.text,
            timestamp_ms,
            pinned: entry.pinned,
        });
    }
    let imported = shared
        .import_if_empty(entries)
        .map_err(|_| "clipboard migration failed")?;
    if imported {
        std::fs::remove_file(&legacy_path).map_err(|_| "clipboard migration cleanup failed")?;
    }
    Ok(imported)
}

/// Migrate the first Harmony host's local structured history into the shared mobile store.
/// The caller opts into this path explicitly, so an unrelated `state` directory in an Apple App
/// Group can never be mistaken for Harmony data.
fn migrate_harmony_clipboard_history(root: &std::path::Path) -> Result<bool, String> {
    let _lock = apple_clipboard_migration_lock(root)?;
    let shared_path = root.join("MSIME").join("clipboard_history.json");
    let mut shared = msime_client_core::clipboard::ClipboardHistoryStore::open(&shared_path);
    shared
        .load()
        .map_err(|_| "shared clipboard history unavailable")?;
    if !shared.entries().is_empty() {
        return Ok(false);
    }

    let legacy_path = root.join("state").join("clipboard-history.json");
    let metadata = match std::fs::symlink_metadata(&legacy_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err("legacy clipboard history unavailable".into()),
    };
    if !metadata.file_type().is_file() {
        return Err("invalid legacy clipboard history".into());
    }
    let bytes = crate::bounded_file::read(
        std::fs::File::open(&legacy_path).map_err(|_| "legacy clipboard history unavailable")?,
        MAX_APPLE_LEGACY_CLIPBOARD_BYTES,
    )
    .map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            "invalid legacy clipboard history"
        } else {
            "legacy clipboard history unavailable"
        }
    })?;
    let legacy: Vec<HarmonyLegacyClipboardEntry> =
        serde_json::from_slice(&bytes).map_err(|_| "invalid legacy clipboard history")?;
    if legacy.len() > 50 {
        return Err("invalid legacy clipboard history".into());
    }
    let mut texts = std::collections::HashSet::new();
    let mut entries = Vec::with_capacity(legacy.len());
    for entry in legacy {
        if !entry.at.is_finite()
            || entry.at < 0.0
            || entry.at > u64::MAX as f64
            || !texts.insert(entry.text.clone())
            || !msime_client_core::clipboard::mobile_text_is_valid(&entry.text)
        {
            return Err("invalid legacy clipboard history".into());
        }
        entries.push(msime_client_core::clipboard::ClipboardHistoryEntry {
            text: entry.text,
            timestamp_ms: entry.at.round() as u64,
            pinned: entry.pinned,
        });
    }
    let imported = shared
        .import_if_empty(entries)
        .map_err(|_| "clipboard migration failed")?;
    if imported {
        std::fs::remove_file(&legacy_path).map_err(|_| "clipboard migration cleanup failed")?;
    }
    Ok(imported)
}

/// Clear shared mobile history and its fixed Apple legacy source under one lock.
fn clear_mobile_clipboard_history_with_legacy(
    root: &std::path::Path,
    legacy: Option<MobileClipboardLegacy>,
) -> Result<(), String> {
    let _lock = apple_clipboard_migration_lock(root)?;
    let legacy_path = root.join("Clipboard").join("history.json");
    match std::fs::symlink_metadata(&legacy_path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            std::fs::remove_file(&legacy_path).map_err(|_| "mobile clipboard clear failed")?;
        }
        Ok(_) => return Err("mobile clipboard clear failed".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("mobile clipboard clear failed".into()),
    }
    if matches!(legacy, Some(MobileClipboardLegacy::HarmonyState)) {
        let harmony_path = root.join("state").join("clipboard-history.json");
        match std::fs::symlink_metadata(&harmony_path) {
            Ok(metadata) if metadata.file_type().is_file() => {
                std::fs::remove_file(harmony_path).map_err(|_| "mobile clipboard clear failed")?;
            }
            Ok(_) => return Err("mobile clipboard clear failed".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("mobile clipboard clear failed".into()),
        }
    }
    msime_client_core::clipboard::ClipboardHistoryStore::open(
        root.join("MSIME").join("clipboard_history.json"),
    )
    .clear()
    .map_err(|_| "mobile clipboard clear failed".to_owned())
}

/// Clear the shared mobile and Apple legacy history for native callers that do not request a
/// platform-specific migration path.
pub fn clear_mobile_clipboard_history(root: &std::path::Path) -> Result<(), String> {
    clear_mobile_clipboard_history_with_legacy(root, None)
}

/// Structured mobile clipboard history operations. The directory is the trusted
/// App Group root; shared data lives below MSIME and the fixed Apple legacy path
/// is migrated under a stable lock. This intentionally does not read or change
/// the desktop automatic-capture preference: mobile access is host-permission gated.
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
        if matches!(&request.action, MobileClipboardAction::Clear) {
            clear_mobile_clipboard_history_with_legacy(root, request.legacy)?;
            return Ok(json!({"cleared": true, "migrated": false, "entries": []}));
        }
        let mut migrated = migrate_apple_clipboard_history(root)?;
        if matches!(request.legacy, Some(MobileClipboardLegacy::HarmonyState)) {
            migrated = migrate_harmony_clipboard_history(root)? || migrated;
        }
        let path = root.join("MSIME").join("clipboard_history.json");
        let mut history = msime_client_core::clipboard::ClipboardHistoryStore::open(path);
        match request.action {
            MobileClipboardAction::Load => {
                history
                    .load()
                    .map_err(|_| "mobile clipboard history unavailable")?;
                Ok(json!({"entries": history.entries(), "migrated": migrated}))
            }
            MobileClipboardAction::Capture { text } => {
                if !msime_client_core::clipboard::mobile_text_is_valid(&text) {
                    return Ok(
                        json!({"captured": false, "reason": "invalid", "migrated": migrated}),
                    );
                }
                let captured = history
                    .push_mobile(text)
                    .map_err(|_| "mobile clipboard capture failed")?;
                Ok(json!({
                    "captured": captured,
                    "reason": (!captured).then_some("full"),
                    "migrated": migrated,
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
                    "migrated": migrated,
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
                    "migrated": migrated,
                    "entries": history.entries()
                }))
            }
            MobileClipboardAction::Clear => unreachable!("clear handled before migration"),
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
            .load()
            .map_err(|_| "history preferences unavailable")?
            .preferences
            .clipboard_history
        {
            return Err("clipboard history disabled".into());
        }
        let mut history = msime_client_core::clipboard::ClipboardHistoryStore::open(
            path.join("clipboard_history.json"),
        );
        let removed = history
            .remove(&removal.text)
            .map_err(|_| "clipboard history removal failed")?;
        Ok(json!({"removed": removed}))
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
        let outcome = PreferencesStore::new(directory)
            .recover_malformed()
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
        let snapshot: PreferencesSnapshot =
            serde_json::from_slice(snapshot_bytes).map_err(|_| "invalid preferences snapshot")?;
        if snapshot.format_version != 1 {
            return Err("unsupported preferences format".into());
        }
        let store = PreferencesStore::new(directory);
        let saved = store
            .save(expected_revision, snapshot.preferences)
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
/// A shim: the request is parsed into the shared action type and the answer is the shared status,
/// serialised. Every rule — what an action does, how the queue is built, what counts as today —
/// belongs to `msime_client_core::vocabulary::session`, so this host and the Tauri command layer
/// cannot drift from each other.
/// # Safety
/// `request` points to `length` readable JSON bytes. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_vocabulary_review(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    use msime_client_core::vocabulary::session;

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
    }

    response(|| {
        // Deliberately not the 65_536 the other entry points use. Those carry a setting or one
        // clipboard row; this one carries an imported word list, and a five-thousand-word CET book
        // is a few hundred kilobytes of text. Copying the smaller cap here would have made the
        // import path reject every real file while looking like it was merely being careful.
        if request.is_null() || length > session::MAX_IMPORT_BYTES {
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
        let status = session::apply(
            std::path::Path::new(&request.directory),
            std::path::Path::new(&request.resources),
            &request.day,
            request.action,
        )
        .map_err(|error| error.to_string())?;
        serde_json::to_value(status).map_err(|_| "vocabulary review response failed".to_owned())
    })
}

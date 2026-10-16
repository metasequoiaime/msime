//! Key sounds, commit sounds, background music, sound and music pack files, and the 插件 page's pack store for native hosts.
//!
//! Part of the C ABI; see the parent module for what these shims guarantee. The three sound calls sit on the key path, so they answer a plain bool instead of a JSON document the host would have to free: whether a request was queued. They never block, decode or touch the disk; `key_sound` says why.

use crate::*;
use key_sound::{KeyClass, PluginRoots, SessionSound};
use msime_client_services::plugins::mentions::{MentionEntry, MentionStore};
use msime_client_services::plugins::{self, PluginFailure};
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
use msime_client_services::plugins::{script_commands, script_grants};

/// 授权页说明插件指令的状态时用的用户指令表：状态目录 `state_root`（插件根目录的上一级，偏好文件也在这里）里偏好启用的指令表包，和输入法交给引擎的是同一份。偏好读不出时按没有启用任何指令表说明。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn user_command_triggers(state_root: &str, root: &Path) -> Vec<String> {
    let enabled = msime_client_core::preferences::PreferencesStore::new(state_root)
        .load()
        .map(|snapshot| snapshot.preferences.plugins.command_tables)
        .unwrap_or_default();
    script_commands::user_triggers(root, &enabled)
}

/// 拿会话的声音设置执行 `action`。未知会话、错线程、重入调用和 panic（不能越过 C 边界）都返回 false。会话开了生命周期的 FEEDBACK 时同样返回 false、什么都不做：声音由核心驱动，宿主再调一次会响两遍。
fn with_sound(handle: u64, action: impl FnOnce(&SessionSound) -> bool) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        SESSIONS.with(|sessions| {
            let Ok(sessions) = sessions.try_borrow() else {
                return false;
            };
            sessions
                .get(&handle)
                .filter(|session| !session.feedback_driven())
                .is_some_and(|session| action(&session.sound))
        })
    }))
    .unwrap_or(false)
}

/// Queue the sound of one key press. `key_class` is 0 for any other key, 1 space, 2 enter, 3 backspace; anything else queues nothing.
#[no_mangle]
pub extern "C" fn msime_client_key_sound(handle: u64, key_class: u32) -> bool {
    let Some(class) = KeyClass::from_code(key_class) else {
        return false;
    };
    with_sound(handle, |sound| key_sound::key(sound, class))
}

/// 一个键或一次上屏的打字特效，打包成一个整数；事件编号和各位的含义见 `include/msime_client.h`。未知会话、错线程、重入调用和 panic（不能越过 C 边界）都返回 0。会话开了生命周期的 FEEDBACK 时，事件 0-4 由核心产生，这里对它们返回 0；事件 5（绕过按键音的删除）核心看不到，仍由宿主调用。
#[no_mangle]
pub extern "C" fn msime_client_typing_effect(handle: u64, event: u32) -> u32 {
    catch_unwind(AssertUnwindSafe(|| {
        SESSIONS.with(|sessions| {
            let Ok(sessions) = sessions.try_borrow() else {
                return 0;
            };
            sessions.get(&handle).map_or(0, |session| {
                if event & 0xFF <= key_sound::EVENT_COMMIT && session.feedback_driven() {
                    return 0;
                }
                key_sound::typing_effect(&session.sound, event, std::time::Instant::now())
            })
        })
    }))
    .unwrap_or(0)
}

/// The session's resolved typing effect: the selected effect pack, or the preferences' style and intensity without one, as `{pack, issue, style, intensity, colors, duration_ms, particles, combo_counter}`. Read it when the preferences change or a field gains focus, not per key: `msime_client_typing_effect` stays the key-path call.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub extern "C" fn msime_client_typing_effect_settings(handle: u64) -> *mut c_char {
    response(|| {
        with_session(handle, |session| {
            Ok(key_sound::effect_settings(&session.sound))
        })
    })
}

/// Queue the sound of a commit: the key pack's commit sample, the melody's next note when it advances on commits, or both.
#[no_mangle]
pub extern "C" fn msime_client_commit_sound(handle: u64) -> bool {
    with_sound(handle, key_sound::commit)
}

/// Whether background music may play now: true while the input method is active in a field that is not a secure one, false otherwise.
#[no_mangle]
pub extern "C" fn msime_client_music_set_active(handle: u64, active: bool) -> bool {
    with_sound(handle, |sound| key_sound::music_active(sound, active))
}

/// The request both pack calls take: the state root holding `plugins/`, the bundle's built-in sound packs, and the pack's id.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PackRequest {
    state_root: Option<String>,
    sound_packs: Option<String>,
    pack: String,
}

/// Read a `PackRequest` and resolve its roots. `invalid` and `relative` are the call's own errors for a request it cannot read and for a path that is not absolute.
/// # Safety
/// As the calling export: `request` points to `length` readable bytes, or is null.
unsafe fn pack_request(
    request: *const u8,
    length: usize,
    invalid: &'static str,
    relative: &'static str,
) -> Result<(PluginRoots, String), String> {
    // SAFETY: guaranteed by the documented caller contract; the length is bounded first.
    let request: PackRequest = unsafe {
        with_bounded_bytes(request, length, 65_536, invalid, |bytes| {
            serde_json::from_slice(bytes).map_err(|_| invalid.to_owned())
        })
    }?;
    let absolute = |path: &Option<String>| path.as_deref().is_none_or(absolute_path);
    if !absolute(&request.state_root) || !absolute(&request.sound_packs) {
        return Err(relative.into());
    }
    // 这两个调用只认音效包和背景音乐包，官方插件包与它们无关。
    let roots = PluginRoots::new(
        request.state_root.as_deref(),
        request.sound_packs.as_deref(),
        None,
        "",
    );
    Ok((roots, request.pack))
}

/// The validated files of one sound pack, for a host that plays packs itself.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_key_sound_pack(
    request: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        // SAFETY: forwarded from this function's own contract.
        let (roots, pack) = unsafe {
            pack_request(
                request,
                length,
                "invalid sound pack request",
                "sound pack paths must be absolute",
            )
        }?;
        key_sound::pack_files(&roots, &pack)
    })
}

/// The validated tracks of one music pack, for a host that streams music itself.
/// # Safety
/// `request` points to `length` readable UTF-8 JSON bytes. Null is rejected.
/// The returned response must be released with `msime_client_string_free`.
#[no_mangle]
pub unsafe extern "C" fn msime_client_music_pack(request: *const u8, length: usize) -> *mut c_char {
    response(|| {
        // SAFETY: forwarded from this function's own contract.
        let (roots, pack) = unsafe {
            pack_request(
                request,
                length,
                "invalid music pack request",
                "music pack paths must be absolute",
            )
        }?;
        key_sound::music_pack_files(&roots, &pack)
    })
}

/// Largest `msime_client_plugins` request: a full name list at its document bound, with room for JSON escaping and the two paths.
const MAX_PLUGINS_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// 插件页的包仓库和 @ 名单，给桌面外壳以外的设置宿主：Windows 的原生设置应用、鸿蒙的插件页，以及 Android、iOS 设置里导入辅助码表。所有规则和失败码都来自 client-services，与桌面外壳回答设置页的相同。
///
/// 脚本插件只给有授权页的宿主：请求带 `script_page: true` 时目录列出脚本包、导入接受脚本包（`import_from_settings`），桌面上还能读写授权（`script_grants`、`set_script_grant`、`reorder_scripts`，回答和桌面外壳的同名命令一样是 `ScriptGrantsView`，其中插件 `/` 指令的状态按 `state_root` 里偏好启用的指令表算）。不带它时（Android、HarmonyOS 和各处的辅助码表导入）照旧：目录经 `without_scripts`，导入走 `import_without_script_page`。
/// # Safety
/// `request` 指向 `length` 个可读的 UTF-8 JSON 字节，空指针会被拒绝。返回的响应必须用 `msime_client_string_free` 释放。
#[no_mangle]
pub unsafe extern "C" fn msime_client_plugins(request: *const u8, length: usize) -> *mut c_char {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        state_root: String,
        sound_packs: Option<String>,
        /// 桌面安装包随带的官方插件包目录，同宿主选项的 `plugin_packs`：没有时取 `sound_packs` 旁边的 `plugin-packs`。目录、授权和授权页的指令状态都按它找官方包。
        plugin_packs: Option<String>,
        /// 宿主有脚本插件的授权页（列出、授权、排序、写明残余风险）。没有时为假。
        #[serde(default)]
        script_page: bool,
        /// 输入法的 `user_data` 目录（宿主选项里的同名项）：授权的回答带上它下面 `script-health.json` 里的停用记录。没有时不带停用记录。
        user_data: Option<String>,
        action: PluginAction,
    }
    #[derive(Deserialize)]
    #[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
    enum PluginAction {
        Catalog,
        Import {
            source: String,
        },
        Remove {
            kind: String,
            id: String,
        },
        ImportHelpcodeTable {
            file_name: String,
            text: String,
        },
        LoadMentions,
        SaveMentions {
            entries: Vec<MentionEntry>,
        },
        /// 脚本插件的授权、执行顺序和输入法记下的停用。
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        ScriptGrants,
        /// 授予、调整或撤销一个脚本插件的授权；`hash` 是页面展示给用户的内容哈希。
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        SetScriptGrant {
            id: String,
            hash: String,
            enabled: bool,
            granted: Vec<String>,
        },
        /// 调整执行顺序：`ids` 按给出的顺序排在最前。
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        ReorderScripts {
            ids: Vec<String>,
        },
    }
    plugin_response(|| {
        let invalid = || PluginFailure::code("invalid");
        // SAFETY: guaranteed by the documented caller contract; the length is bounded first.
        let request: Request = unsafe {
            with_bounded_bytes(request, length, MAX_PLUGINS_REQUEST_BYTES, "", |bytes| {
                serde_json::from_slice(bytes).map_err(|_| String::new())
            })
        }
        .map_err(|_| invalid())?;
        if !absolute_path(&request.state_root)
            || !request.sound_packs.as_deref().is_none_or(absolute_path)
            || !request.plugin_packs.as_deref().is_none_or(absolute_path)
            || !request.user_data.as_deref().is_none_or(absolute_path)
        {
            return Err(invalid());
        }
        let roots = PluginRoots::new(
            Some(&request.state_root),
            request.sound_packs.as_deref(),
            request.plugin_packs.as_deref(),
            "",
        );
        let root = roots.installed.clone().ok_or_else(invalid)?;
        #[cfg_attr(
            not(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            )),
            allow(unused_variables)
        )]
        let builtin_packs = roots.builtin_packs.as_deref();
        #[cfg_attr(
            not(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            )),
            allow(unused_variables)
        )]
        let user_data = request.user_data.as_deref().map(Path::new);
        let value = match request.action {
            // 没有授权页的宿主看不到脚本插件。
            PluginAction::Catalog => {
                let catalog = plugins::scan(&root, roots.builtin());
                serde_json::to_value(if request.script_page {
                    catalog
                } else {
                    catalog.without_scripts()
                })
            }
            PluginAction::Import { source } => {
                if !absolute_path(&source) {
                    return Err(invalid());
                }
                let source = Path::new(&source);
                serde_json::to_value(if request.script_page {
                    plugins::import_from_settings(source, &root)?
                } else {
                    plugins::import_without_script_page(source, &root)?
                })
            }
            // 移动端选到的是 content URI 或 security-scoped URL，没有能交给 `import` 的路径：宿主严格按 UTF-8 解码后把码表正文放进请求。
            PluginAction::ImportHelpcodeTable { file_name, text } => serde_json::to_value(
                plugins::helpcode_pack::import_table(&root, &file_name, text.as_bytes())?,
            ),
            PluginAction::Remove { kind, id } => {
                plugins::remove_named(&root, &kind, &id)?;
                Ok(Value::Null)
            }
            PluginAction::LoadMentions => serde_json::to_value(MentionStore::new(root).load()?),
            PluginAction::SaveMentions { entries } => {
                MentionStore::new(root).save(&entries)?;
                Ok(Value::Null)
            }
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            PluginAction::ScriptGrants => serde_json::to_value(script_grants::view(
                &root,
                builtin_packs,
                script_grants::read(&root),
                user_data,
                &user_command_triggers(&request.state_root, &root),
            )),
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            PluginAction::SetScriptGrant {
                id,
                hash,
                enabled,
                granted,
            } => {
                let granted = script_grants::capabilities(&granted)?;
                let grants =
                    script_grants::set(&root, builtin_packs, &id, &hash, enabled, granted)?;
                serde_json::to_value(script_grants::view(
                    &root,
                    builtin_packs,
                    grants,
                    user_data,
                    &user_command_triggers(&request.state_root, &root),
                ))
            }
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            PluginAction::ReorderScripts { ids } => serde_json::to_value(script_grants::view(
                &root,
                builtin_packs,
                script_grants::reorder(&root, &ids)?,
                user_data,
                &user_command_triggers(&request.state_root, &root),
            )),
        };
        value.map_err(|_| PluginFailure::code("storage"))
    })
}

/// 带上页面失败记录的 `response`：`error` 是失败码，`detail` 在 client-services 给出时是被违反的那条规则。
pub(super) fn plugin_response(
    operation: impl FnOnce() -> Result<Value, PluginFailure>,
) -> *mut c_char {
    let value = match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(value)) => json!({ "ok": true, "value": value }),
        Ok(Err(PluginFailure {
            code,
            detail: Some(detail),
        })) => json!({ "ok": false, "error": code, "detail": detail }),
        Ok(Err(PluginFailure { code, detail: None })) => json!({ "ok": false, "error": code }),
        Err(_) => json!({ "ok": false, "error": "internal runtime failure" }),
    };
    // JSON escapes embedded NUL bytes, so this cannot contain an interior NUL.
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}

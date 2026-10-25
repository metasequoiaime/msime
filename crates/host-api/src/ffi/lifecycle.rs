//! Updating a live session, tearing it down, and freeing what the library returned.
//!
//! 也在这里：生命周期核心的选择加入和它的几个按键路径调用（`set_lifecycle`、`key`、`set_field_traits`、`set_active`），见 `crate::lifecycle`。
//!
//! Part of the C ABI; see the parent module for what these shims guarantee.

use crate::*;

/// Queue a validated preference snapshot without interrupting composition.
/// # Safety
/// `snapshot` must point to `length` readable bytes for this call. Null is rejected.
#[no_mangle]
pub unsafe extern "C" fn msime_client_update_preferences(
    handle: u64,
    snapshot: *const u8,
    length: usize,
) -> *mut c_char {
    response(|| {
        if snapshot.is_null() || length > PREFERENCES_DOCUMENT_LIMIT {
            return Err("invalid preferences buffer".into());
        }
        // SAFETY: guaranteed by the caller's buffer contract.
        let bytes = unsafe { std::slice::from_raw_parts(snapshot, length) };
        let snapshot = serde_json::from_slice(bytes).map_err(|_| "invalid preferences document")?;
        with_session(handle, |session| {
            // 关掉 AI 或换了 AI 配置会在组字中途去掉候选里的 AI 行：按旁路调用推导。
            let pending = session.lifecycle_begin(lifecycle::Call::SideChannel);
            let updated = session.update(snapshot);
            let updated = session.lifecycle_on_error(pending, updated)?;
            session.lifecycle_settle(pending);
            Ok(updated)
        })
    })
}

#[no_mangle]
pub extern "C" fn msime_client_destroy(handle: u64) -> *mut c_char {
    response(|| destroy_session(handle))
}

/// `msime_client_destroy` 的本体。
pub(crate) fn destroy_session(handle: u64) -> Result<Value, String> {
    SESSIONS.with(|sessions| {
        let mut session = sessions
            .try_borrow_mut()
            .map_err(|_| "reentrant host call")?
            .remove(&handle)
            .ok_or("unknown session or wrong thread")?;
        // 还在组字时先发 composition_end{Destroyed}；是音乐所有者时关掉音乐。
        session.lifecycle_destroy();
        // A host may tear a session down without a focus-out first; what it counted is still written.
        session.flush_selections();
        // Dropping the engine session writes its own journal's delayed context learning; this also covers every other journal the process learned into, while this session still holds its dictionary access.
        msime_engine::flush_personal_learning();
        // 停顿模型可能仍在后台评分；销毁会话时放弃任务，避免模型残留在作业表中。
        crate::ffi::settled::abandon_settled_job(handle);
        drop(session);
        Ok(Value::Null)
    })
}

/// 在会话所在线程上对选择加入的会话做一件事，返回值不经 JSON。未知会话、错线程、重入调用和 panic 都返回 `fallback`：panic 不能越过 C 边界。
fn with_host_session<T: Copy>(
    handle: u64,
    fallback: T,
    action: impl FnOnce(&mut HostSession) -> T,
) -> T {
    catch_unwind(AssertUnwindSafe(|| {
        SESSIONS.with(|sessions| {
            let Ok(mut sessions) = sessions.try_borrow_mut() else {
                return fallback;
            };
            sessions.get_mut(&handle).map_or(fallback, action)
        })
    }))
    .unwrap_or(fallback)
}

/// 让会话选择加入生命周期核心，或换一组位。不落盘。返回实际启用的位；未知会话返回 0。
#[no_mangle]
pub extern "C" fn msime_client_set_lifecycle(handle: u64, flags: u32) -> u32 {
    with_host_session(handle, 0, |session| {
        let accepted = match session.lifecycle.as_deref_mut() {
            Some(state) => state.set_flags(&session.sound, flags),
            None => {
                let mut state = lifecycle::LifecycleState::new(handle, flags);
                state.bind_scripts(
                    session.plugin_roots.installed.clone(),
                    session.plugin_roots.builtin_packs.clone(),
                    &session.options.user_data,
                );
                // 脚本插件的候选变换和插件指令的提供者挂在 runtime 上，只在派发类调用里运行；移动端什么都不挂。
                state.attach_scripts(&mut session.runtime);
                let accepted = state.flags();
                session.lifecycle = Some(Box::new(state));
                // 选择加入之前聚焦过的会话，插件指令不等下一次聚焦。
                session.refresh_plugin_commands();
                accepted
            }
        };
        // 上屏只能观察时候选变换停下，不运行也不重放。
        crate::scripts::observe_only(
            &mut session.runtime,
            accepted & lifecycle::COMMIT_OBSERVE_ONLY != 0,
        );
        accepted
    })
}

/// 进程级：打开或关闭脚本插件，返回开关的实际状态。只由 4 个桌面入口（macOS 输入法进程、Windows Server、IBus 引擎进程、Fcitx5 addon）在启动时调用；Windows TIP、设置应用和 MCP 从不调用。移动端恒返回 false。
#[no_mangle]
pub extern "C" fn msime_client_scripts_enable(on: bool) -> bool {
    crate::scripts::enable(on)
}

/// 一次物理按下，开启新的一轮（`PASSTHROUGH` 除外）。FEEDBACK 下出按键音并返回打字特效，位布局同 `msime_client_typing_effect`；没选择加入、类别不认识、SECURE 字段或指针调用时返回 0。
#[no_mangle]
pub extern "C" fn msime_client_key(handle: u64, key: u32) -> u32 {
    with_host_session(handle, 0, |session| {
        session
            .lifecycle
            .as_deref_mut()
            .map_or(0, |state| state.key(&session.sound, key))
    })
}

/// 报告字段属性。变了才发 `traits_changed`。返回现在生效的属性；没选择加入时返回 0。
#[no_mangle]
pub extern "C" fn msime_client_set_field_traits(handle: u64, traits: u32) -> u32 {
    with_host_session(handle, 0, |session| {
        session
            .lifecycle
            .as_deref_mut()
            .map_or(0, |state| state.set_traits(&session.sound, traits))
    })
}

/// 输入法在这个会话里激活或停用。FEEDBACK 下由它仲裁背景音乐的所有者。会话已选择加入时返回 true。
#[no_mangle]
pub extern "C" fn msime_client_set_active(handle: u64, active: bool) -> bool {
    with_host_session(handle, false, |session| {
        session.lifecycle.as_deref_mut().is_some_and(|state| {
            state.set_active(&session.sound, active);
            true
        })
    })
}

/// Write what would otherwise wait: the selection counts of every session on the calling thread and every queued personal-context transition of the process. The C++ Engine wrote its queue from `atexit` (personal_ngram_store.cpp:254-255); a host that can exit without a focus-out or a destroy, such as macOS `[NSApp terminate:]`, calls this from its will-terminate hook on the thread that owns its sessions. Sessions stay usable.
#[no_mangle]
pub extern "C" fn msime_client_flush_all() -> *mut c_char {
    response(|| {
        SESSIONS.with(|sessions| {
            // A reentrant call (from inside another host call on this thread) cannot reach the sessions; the Engine's queue is still written.
            if let Ok(mut sessions) = sessions.try_borrow_mut() {
                for session in sessions.values_mut() {
                    session.flush_selections();
                }
            }
        });
        msime_engine::flush_personal_learning();
        Ok(Value::Null)
    })
}

/// # Safety
/// `value` must be null or an allocation returned by this library, not yet freed.
#[no_mangle]
pub unsafe extern "C" fn msime_client_string_free(value: *mut c_char) {
    if !value.is_null() {
        // SAFETY: ownership is transferred back exactly once by the C caller.
        drop(unsafe { CString::from_raw(value) });
    }
}

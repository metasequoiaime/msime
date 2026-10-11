//! 脚本插件在输入法进程里的一侧：进程开关、每个会话的实例集合，以及把生命周期核心推导出的观察类事件按授权投递给脚本。解释器、ABI v1 和配额在 `msime-script-runtime`，授权文件和包的读取在 client-core 的 `plugins::script_grants`、`plugins::script_pack`。
//!
//! - **进程开关** `msime_client_scripts_enable`：只有 4 个桌面入口在启动时打开它，即 macOS 的 IMK 进程、Windows Server、IBus 引擎进程和 Fcitx5 addon。Windows TIP（与 Server 共用同一份 DLL，被注入每个应用进程）、设置应用和 MCP 从不调用。移动端和 release 构建里它恒为关：授权文件在用户可写的目录下，同一用户下的任何进程都能伪造一条授权，在 macOS 钥匙串 MAC 等加固落地（第 5 阶段）之前，release 构建不运行脚本。
//! - **运行条件**，五项同时满足：进程开关开着；会话经 `msime_client_set_lifecycle` 选择加入（没选择加入的会话没有 `LifecycleState`，也就没有这里的任何东西）；字段属性带 KNOWN；不是 SECURE 或 PRIVATE；不是 ELEVATED。属性未知按不满足处理；属性变得不满足时正在跑的实例当场丢弃。每次聚焦之后属性还要重新确认（聚焦前报告过属性，或者聚焦后的第一个键），否则新字段在第一个键之前沿用的是上一个字段的属性。
//! - **投递哪些事件**：`focus_in`、`focus_out`、`input`、`selection_retracted`、`candidate_selected`、`commit`、`composition_start`、`composition_update`、`composition_end`、`candidates_shown`、`candidates_hidden`，只来自派发类调用、focus 和 destroy。按键（`key_down`）不投递，免得泄露击键节奏；字段属性、激活和旁路调用（`apply_online_*`、`rerank_settled`、设置类调用）产生的视图事件也不投递，免得泄露停顿和网络时序。`before_commit` 和 `candidates` 是修改类钩子，第 4 阶段才投递。
//! - **实例**挂在会话上，第一次需要投递时才实例化，按授权顺序，受每轮的初始化预算约束。每次 `msime_client_focus`（两个方向）和 destroy 都把整个集合丢弃：失焦在 `focus_out` 送达之后丢，聚焦在任何事件之前丢。一个插件 trap 之后，这次聚焦里不再重建它的实例；每轮的 fuel 配额按插件记，同一轮里重建的实例接着用剩下的。
//! - **模块缓存、熔断和健康文件是进程级的**，见 `process`。

use msime_input_runtime::{DispatchFacts, View};

#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
mod process;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
mod set;
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) use set::ScriptSet;
#[cfg(all(
    test,
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
pub(crate) use {
    process::{
        health_barrier, health_snapshot, insert_ready, TEST_COMPILE_PANIC_ID, TEST_PANICKED,
        TEST_SLOW, TEST_SWITCH,
    },
    set::{TEST_PANIC, TEST_SCRIPT_LOG},
};

/// 一次调用里脚本可能读到的文本，借自这次调用的 transition 和 runtime。每个插件按自己的授权和本地模式各复制一份，没有授权的文本根本不复制。
#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(
    not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )),
    allow(dead_code)
)]
pub(crate) struct CallTexts<'a> {
    /// 调用之后的视图：预编辑、读音、光标和候选页。
    pub view: Option<&'a View>,
    /// 这次上屏的文本，全角转换之后、宿主拿到的那份。
    pub commit: Option<&'a str>,
    /// 上屏时的本地模式（`commit_context.local_mode`，派发之前的模式）。
    pub commit_local_mode: Option<&'a str>,
    /// 这次调用的派发事实，切分上屏文本的 body 和 tail 用。
    pub facts: DispatchFacts,
    /// 选中的候选的文本，见 `Runtime::selected_text`。
    pub selected: Option<&'a str>,
}

/// 进程开关。只有开发构建的桌面目标有它。
#[cfg(all(
    debug_assertions,
    any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )
))]
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `msime_client_scripts_enable` 的实现：返回开关的实际状态。release 构建和移动端恒为 false，`on` 被忽略。
pub(crate) fn enable(on: bool) -> bool {
    #[cfg(all(
        debug_assertions,
        any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        )
    ))]
    {
        ENABLED.store(on, std::sync::atomic::Ordering::Release);
        on
    }
    #[cfg(not(all(
        debug_assertions,
        any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        )
    )))]
    {
        let _ = on;
        false
    }
}

/// 进程开关现在是否开着。release 构建恒为 false。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn switched_on() -> bool {
    #[cfg(debug_assertions)]
    {
        ENABLED.load(std::sync::atomic::Ordering::Acquire)
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

/// `msime_client_script_grants_key` 的实现：把 macOS 宿主从钥匙串取到的密钥交给 client-core 的授权读取方。返回本进程现在用的是不是这把密钥；长度不对或移动端返回 false。
pub(crate) fn set_grants_key(key: &[u8]) -> bool {
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        msime_client_core::plugins::script_grants::set_mac_key(key)
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        let _ = key;
        false
    }
}

/// 移动端没有脚本插件：同样的接口，什么都不做。解释器不进这些目标的产物。
#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
mod inert {
    use super::CallTexts;
    use crate::lifecycle::{Context, InputClass, LifecycleEvent, LocalModeClass};
    use std::path::PathBuf;

    #[derive(Default)]
    pub(crate) struct ScriptSet;

    impl ScriptSet {
        pub(crate) fn bind(&mut self, _root: Option<PathBuf>, _user_data: &str) {}
        pub(crate) fn next_turn(&mut self) {}
        pub(crate) fn traits_reported(&mut self, _context: Context) {}
        pub(crate) fn key_reported(&mut self) {}
        pub(crate) fn focus_in(&mut self) {}
        pub(crate) fn release(&mut self) {}
        pub(crate) fn input(
            &mut self,
            _context: Context,
            _class: InputClass,
            _local_mode: LocalModeClass,
            _input_char: Option<char>,
        ) {
        }
        pub(crate) fn deliver(
            &mut self,
            _event: &LifecycleEvent,
            _context: Context,
            _texts: &CallTexts<'_>,
        ) {
        }
    }
}
#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) use inert::ScriptSet;

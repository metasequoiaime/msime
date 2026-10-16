//! 脚本插件在输入法进程里的一侧：进程开关、每个会话的实例集合，以及把生命周期核心推导出的观察类事件按授权投递给脚本。解释器、ABI v1 和配额在 `msime-script-runtime`，授权文件和包的读取在 client-services 的 `plugins::script_grants`、`plugins::script_pack`。
//!
//! - **进程开关** `msime_client_scripts_enable`：只有 4 个桌面入口在启动时打开它，即 macOS 的 IMK 进程、Windows Server、IBus 引擎进程和 Fcitx5 addon。Windows TIP（与 Server 共用同一份 DLL，被注入每个应用进程）、设置应用和 MCP 从不调用。移动端恒为关。三个桌面平台的开发构建和 release 构建都能打开：授权文件（client-services 的 `script_grants`）不签名，同一用户下的进程能伪造授权，这一残余风险写在授权页上，macOS 和 Linux 是共享设置页，Windows 是原生设置应用（platforms/windows/settings，经 `msime_client_plugins` 的 `script_page` 列出脚本包并读写授权）。Windows 的提权窗口（`ELEVATED`）里不运行脚本。
//! - **运行条件**，五项同时满足：进程开关开着；会话经 `msime_client_set_lifecycle` 选择加入（没选择加入的会话没有 `LifecycleState`，也就没有这里的任何东西）；字段属性带 KNOWN；不是 SECURE 或 PRIVATE；不是 ELEVATED。属性未知按不满足处理；属性变得不满足时正在跑的实例当场丢弃。每次聚焦之后属性还要重新确认（聚焦前报告过属性，或者聚焦后第一个不带 PASSTHROUGH 的键），否则新字段在第一个键之前沿用的是上一个字段的属性。
//! - **投递哪些事件**：`focus_in`、`focus_out`、`input`、`selection_retracted`、`candidate_selected`、`commit`、`composition_start`、`composition_update`、`composition_end`、`candidates_shown`、`candidates_hidden`，只来自派发类调用、focus 和 destroy。按键（`key_down`）不投递，免得泄露击键节奏；字段属性、激活和旁路调用（`apply_online_*`、`rerank_settled`、设置类调用）产生的视图事件也不投递，免得泄露停顿和网络时序。
//! - **插件指令**（`command`）：输入法在聚焦时把计划里各插件声明的 `/` 指令交给引擎（`ScriptSet::plugin_commands`，引擎按用户指令表和先到先得过滤）；用户打完某个触发词时，runtime 在派发的 refresh 开头经 [`CommandProvider`](msime_input_runtime::CommandProvider) 问一次，只投递给计划里第一个声明这个触发词的插件，事件带触发词、参数和 64 字节的系统随机数，在快照上运行。插件给的行由引擎按预编辑串缓存，旁路调用、方向键和翻页都不再投递。
//! - **修改类钩子**：`candidates` 由 runtime 在派发类调用的 refresh 和展开之后经 [`CandidateTransform`](msime_input_runtime::CandidateTransform) 调用，插件交回的重排、隐藏和注释由 runtime 按候选身份记下、按授权顺序合成，旁路调用只重放；`before_commit` 在派发类调用的 `complete_transition`（全角转换）之后、`commit` 之前，每次在复位到快照的新实例上运行，只改 body，tail 原样接回。可改写的条件见 `lifecycle::rewritable`；改写不回灌引擎学习和 AI 上文（runtime 在这之前已经按原文记下）。trap、fuel 耗尽或结果不合规时用原文。
//! - **实例**挂在会话上，第一次需要投递时才实例化，按授权顺序，受每轮的初始化预算约束。每次 `msime_client_focus`（两个方向）和 destroy 都把整个集合丢弃：失焦在 `focus_out` 送达之后丢，聚焦在任何事件之前丢。一个插件 trap 之后，这次聚焦里不再重建它的实例；每轮的 fuel 配额按插件记，同一轮里重建的实例接着用剩下的。没有经 `msime_client_key` 开新一轮的派发（修饰键组合、快捷键、失焦命令）和上一次按键共用一轮；本轮配额用完时跳过这个插件，不算失败；运行到一半被截断的实例丢弃、这次聚焦里不再重建，这一次用掉的不到本钩子上限的一半时同样不算失败，达到一半由 runtime 报成 fuel 耗尽的失败，计入熔断。
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

/// 会话选择加入生命周期时把脚本集合挂到 runtime 上，作为它的候选变换和插件指令的提供者。移动端什么都不做。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) fn attach(
    scripts: &std::rc::Rc<std::cell::RefCell<ScriptSet>>,
    runtime: &mut msime_input_runtime::Runtime,
) {
    runtime.set_candidate_transform(Some(Box::new(set::ScriptTransform::new(scripts.clone()))));
    runtime.set_command_provider(Some(Box::new(set::ScriptTransform::new(scripts.clone()))));
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) fn attach(
    _scripts: &std::rc::Rc<std::cell::RefCell<ScriptSet>>,
    _runtime: &mut msime_input_runtime::Runtime,
) {
}

/// 上屏只能观察（`COMMIT_OBSERVE_ONLY`）时暂停候选变换：派发里不运行 `candidates`，旁路调用也不重放。Windows 上 TIP 在自己的会话里选词的列表（注音等 AlwaysInlinePreedit 方案）按那个会话的顺序选，这边重排或隐藏，显示的顺序就和数字选中的对不上。移动端什么都不做。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
pub(crate) fn observe_only(runtime: &mut msime_input_runtime::Runtime, observe_only: bool) {
    runtime.suspend_candidate_transform(observe_only);
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) fn observe_only(_runtime: &mut msime_input_runtime::Runtime, _observe_only: bool) {}

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

impl<'a> CallTexts<'a> {
    /// 派发和 focus 的 transition 已经完成（全角转换之后）时，这次调用的文本。
    pub(crate) fn of(
        transition: &'a msime_input_runtime::Transition,
        facts: DispatchFacts,
        selected: Option<&'a str>,
    ) -> Self {
        Self {
            view: Some(&transition.view),
            commit: transition.commit.as_deref(),
            commit_local_mode: transition
                .commit_context
                .as_ref()
                .map(|context| context.local_mode.as_str()),
            facts,
            selected,
        }
    }
}

/// 一次可能改写的上屏，交给 `before_commit` 的几项。
#[derive(Clone, Copy, Debug)]
#[cfg_attr(
    not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )),
    allow(dead_code)
)]
pub(crate) struct RewriteRequest<'a> {
    /// `CommitOrigin` 的判别值。
    pub origin: u8,
    /// 这次上屏能否改写；不能时照样投递，插件只能看。
    pub rewritable: bool,
    /// 上屏时的本地模式（`commit_context.local_mode`）。
    pub local_mode: Option<&'a str>,
    /// 可改写的那一段，全角转换之后的。
    pub body: &'a str,
    /// 标点、顶字的尾巴，只读，改写后原样接回。
    pub tail: &'a str,
    /// 选中的候选的文本。
    pub selected: Option<&'a str>,
}

/// 进程开关。三个桌面平台的开发构建和 release 构建一样有它，见模块说明。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// `msime_client_scripts_enable` 的实现：返回开关的实际状态。移动端恒为 false，`on` 被忽略。
pub(crate) fn enable(on: bool) -> bool {
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    {
        ENABLED.store(on, std::sync::atomic::Ordering::Release);
        on
    }
    #[cfg(not(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    )))]
    {
        let _ = on;
        false
    }
}

/// 进程开关现在是否开着。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
fn switched_on() -> bool {
    ENABLED.load(std::sync::atomic::Ordering::Acquire)
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
        pub(crate) fn bind(
            &mut self,
            _root: Option<PathBuf>,
            _builtin_packs: Option<PathBuf>,
            _user_data: &str,
        ) {
        }
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
        pub(crate) fn before_commit(
            &mut self,
            _context: Context,
            _request: &super::RewriteRequest<'_>,
        ) -> Option<String> {
            None
        }
        pub(crate) fn plugin_commands(&mut self) -> Vec<msime_input_runtime::PluginCommand> {
            Vec::new()
        }
    }
}
#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
)))]
pub(crate) use inert::ScriptSet;

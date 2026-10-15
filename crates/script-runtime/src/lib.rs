//! 桌面脚本插件的沙箱运行时。
//!
//! 只在桌面编译（macOS、Windows、非 ohos 的 Linux），移动端和 wasm32 上整个 crate 为空。
//!
//! - [`abi`]：ABI v1 的契约：能力、钩子、字段编号、import 白名单和返回码。
//! - [`compile`]/[`validate`]：写死的 wasmi `Config`（Eager 编译、关闭不允许的提案、fuel 计量、栈限制）和模块校验（导出、import 白名单、双向的静态最小权限）。导入时和加载时用同一份。
//! - [`ScriptInstance`]：每会话每插件一个实例，按轮和按钩子的固定 fuel 配额、`StoreLimits` 卡死的线性内存、`before_commit` 和 `command` 的快照复位、观察类 import，以及修改类 import 暂存、校验过的动作（[`Actions`]，含 `command` 给出的行）。
//!
//! 接口不带泛型：宿主（host-api）、client-services 和 pack-tool 都只看到这里的具体类型，wasmi 不出现在它们的签名里。

#![cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]

pub mod abi;
mod instance;
pub mod limits;
mod module;

#[cfg(test)]
mod tests;

pub use abi::{Capability, CapabilityRuleError, CapabilitySet, Hook, HookSet};
pub use instance::{
    ActionSink, Actions, CandidateRow, Candidates, CommandEvent, CommitTexts, Composition,
    DebugSinks, Event, Failure, InitFailure, LogSink, Outcome, Scalars, ScriptInstance,
};
#[cfg(feature = "test-support")]
#[doc(hidden)]
pub use module::compile_text;
pub use module::{compile, validate, Declaration, ModuleError, ScriptModule};

// 模块缓存是进程级的，由后台编译线程填充、各会话线程读取，所以 `Module` 必须能跨线程共享。wasmi 2.0.0 的 `Module` 是 `Arc<ModuleInner>`，这条断言在编译期确认它确实是 Send + Sync；不成立时退回在会话线程的 focus 路径上编译。
const _: fn() = || {
    fn shared_across_threads<T: Send + Sync>() {}
    shared_across_threads::<wasmi::Module>();
    shared_across_threads::<ScriptModule>();
};

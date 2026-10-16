# Agent Note: 重复设置词段暂存保留撤回历史

Status: implemented

## Problem

公开 Rust 接口 `Runtime::set_phrase_preedit(true)` 重复调用时保留 `phrase_prefix`，却清空 `phrase_selections`。部分选词后普通退格无法恢复读音，分段退格删空剩余拼写也会提前提交暂存文字。两个运行时回归均复现了提前提交“海滩”。当前 C ABI 只在创建会话时调用该接口，没有证据表明现有原生宿主正在重复调用它。

## Decision

重复设置同一暂存能力时直接返回 `None`，完整保留当前组词。只有能力真正改变时才执行原有切换逻辑；关闭时仍返回暂存文字并清理历史。

## Historical note audit

按 `set_phrase_preedit`、重复暂存和 `phrase_selections` 检索活跃笔记。[退格偏好](../../implemented/feature/2026-10-16-backspace-selected-phrase.md)持有用户开关与撤回条件，[专用英文切换](../../implemented/bug-fix/2026-10-16-dedicated-english-held-phrase.md)持有模式变化时取消暂存、重复模式保留组词，均为部分重叠的独立决定。本篇记录宿主暂存能力 setter 的幂等性。插件、Windows 生产入口和惰性缓冲验证的命中不持有这个 setter 的状态规则，无需修改。

## Alternatives considered

- **要求调用方去重。** 不改变运行时实现，当前 C ABI 也只调用一次；但公开 Rust 接口没有“同值会丢历史”的契约，且调用者不知道内部撤回栈是否非空。在状态拥有者处保留无变化的组词，才能让所有调用方遵守同一规则。
- **每次启用都清空整段组字。** 前缀与历史可以保持一致，但重复声明宿主具备展示能力不应取消输入，也会改变 setter 现有保留前缀的行为。

## Verification

两个回归在旧实现中都因收到 `Some("海滩")` 而非 `None` 失败。修复后验证普通退格恢复最近词段读音、分段退格删空后保留暂存段且仍可撤回，另覆盖关闭两次只送出一次、重新开启后旧选词不复活。

- `cargo test -p msime-input-runtime -p msime-host-api --locked`：968 项通过、2 项按既有配置忽略，其中 runtime 306 项、host-api 单测 647 项与集成测试 15 项。
- 相关两个 crate 的全目标 `clippy -D warnings`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 与 `git diff --check` 通过。
- 独立审查核对状态转换、调用点与三项回归，未发现可证实的问题。

- `bash scripts/verify-local.sh --quick` 通过：Rust 工作区、Android 目标及 Java 宿主、两种 WASM 配置、macOS 原生宿主、Apple 桥接和仓库契约门禁；Windows 本机可移植测试 155 项通过。Windows 交叉编译及管道、Linux Docker、HarmonyOS 真打包和 Tauri 资源构建因前置环境缺失跳过。

## Consequences

同值设置不再改变输入状态，能力真正切换时沿用已有规则；代价是调用方不能再把重复启用当作清理历史的隐式入口，应使用显式的取消或提交动作。修复仅涉及运行时状态，不改变 C ABI。回归使用已有合成 `PhraseEngine` 验证编排，不代表系统输入框验收。

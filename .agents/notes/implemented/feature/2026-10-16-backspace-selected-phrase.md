# Agent Note: 退格是否回退已选候选词由共享偏好控制

Status: implemented

## Problem

#7572 希望输入 `woshi`、选定“我”以后，退格修改剩余拼音而不要求重新选“我”，并提供“退格键回退已选候选词”开关。当前 `Runtime::retreat_phrase_selection` 已在剩余拼写多于一个字符时逐字删除，但在末尾只剩零或一个字符时撤回最近一次选词；用户无法关闭最后一步的回退。

## Decision

- 共享偏好 `backspace_reverts_selection` 默认开启，保持现有的末尾回退条件；关闭时普通退格直接交给引擎删除拼写，已选词段仍由 Runtime 暂存。拼写删空时沿用既有收尾逻辑，把已选文字一次上屏；下一次退格交回编辑器，不重建旧拼写。
- 开关控制 Runtime 已有的暂存词段编排，不在引擎、宿主各复制一套撤回状态。偏好更新立即传给活着的 Runtime，不等待组字结束，也不清空选词历史，因此中途重新开启仍能按原条件撤回。
- `Ctrl+Backspace`、组字光标中间的编辑、九键内部的拼写锁定、整句改字、取消与提交沿用原规则。
- 共享 React 设置（macOS/Linux/HarmonyOS）和 Android/iOS 原生设置提供同名开关；iOS 把键加入活会话的设置合并名单。

Windows 当前 `EngineSessionAdapter` 和 `ServerSession` 使用准备好的 options（`phrase_preedit: None`），没有启用 Runtime 的暂存词段；TIP 仍保留自己的 `_creatingWordRestoreHistory` 和旧造词分支。这份改动不在 Windows 原生页提供无效开关，也不把这次需求扩成 TSF 组字所有权迁移；PR 使用 `Refs #7572`，Windows 的一致行为与真实编辑器验收保留为后续缺口。

## Historical note audit

检索活跃笔记的 `退格`、`phrase_preedit`、`phrase_prefix`、`retreat_phrase_selection`、`暂存词段`：没有记录同一开关的既有归属。[组字光标点按](../../implemented/feature/2026-10-08-composition-caret-tap.md) 和 [整句改字](../../implemented/feature/2026-10-10-sentence-conversion-edit.md) 记录相关但不重叠的独立状态；本决定不修改它们。其他命中为 Android 编辑器文档删除、语言自动机或候选排序，均无关。

## Alternatives considered

- **在每个平台截获退格，重打剩余拼音**：能在一个宿主中迅速做出界面效果，但会丢掉共享的选词历史、大小写、网格身份，还让六个平台各维护一份状态；复用 Runtime 的现有分支更可靠。
- **把已选词段永远留在预编辑区，即使拼写已经删空**：可以继续在预编辑区逐字删已选文字，但必须新定义已选文字与读音、学习记录的反向映射和删词规则。需求针对未选拼写，沿用现有删空时上屏收尾能保留选择且不引入悬空组字。

## Verification

- 偏好与真实 C ABI 合成词库回归已通过：默认开启和旧文档保持原来的末尾回退；关闭后 `我shi` → `我sh` → `我s` → 上屏“我”，无拼写回退、重复提交或丢字。
- Runtime 全部 298 项单测通过，覆盖多次部分选词、组字中切换开关、取消与分段退格；宿主 JSON 更新走到真实 Runtime。
- 共享 React 页面三个开关用例通过，验证旧文档默认开启、关闭值回显与字段更新。
- 相关三个 crate 的全目标 `clippy -D warnings`、前端类型检查与构建通过。
- `scripts/verify-local.sh --quick` 通过：包括 Rust 工作区、Android/wasm 目标、Android Java 宿主、Windows x64 链接及 GNU 桌面检查、macOS 原生、Apple 桥接和 x64/i686 管道编译。Windows 本机宿主测试 155 项通过；初次运行缺少 x64 缓存，起始 i686 语法阶段跳过。Linux Docker 不可用、HarmonyOS 缺暂存原生库/资源/模块、macOS Tauri 资源包未暂存，相关阶段跳过。
- iOS 原生模拟器构建通过，`MSIMEKeyboardTests/CandidateOptionsSettingsTests/testBackspaceSelectionSwitchReachesAnExistingComposition` 实际执行 1 项、0 失败：真实原生桥接会话在组字中重载 App 写入的关闭值，连续退格保留已选词段，删空后只提交一次。专用模拟器在测试结束后删除；这不代替真机键盘按钮与真实编辑器验收。Android 本轮只有宿主/JVM 检查，未运行模拟器；没有安装用户真机。

## Consequences

收益是所有启用共享暂存词段的宿主遵循同一删除规则，不需要重复学习已选文字。代价是关闭开关后，剩余拼写删空就结束组字并上屏已选词段，不能再用退格恢复它的原拼写。Windows 仍有独立状态路径，尚未接入本开关；界面入口本身不证明实机可用，必须分开记录共享逻辑、编译和设备验证。

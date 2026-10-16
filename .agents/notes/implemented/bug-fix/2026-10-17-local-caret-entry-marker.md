# Agent Note: 直接设置快捷模式光标时保护入口标记

Status: implemented

## Problem

`edit_at_caret` 的方向键把带前缀的本地模式起点限制在入口标记之后，`set_caret` 却只约束上界。公开 Rust 光标接口传入零后，前向删除会删除模式标记；临时日文重建还会把原文第一个字母当标记跳过。插入也可能出现在标记之前，或使本应合法的 Unicode `+` 被拒绝并提交原文。

## Decision

直接光标入口复用方向键的可编辑起点规则：带前缀的本地模式最小为 1，普通方案、专用英文和没有标记的网址模式仍从 0 开始。保留 `None` 返回末尾、越界位置截到末尾、裸标记退格退出模式和九键直接光标无操作的既有语义。规则留在共享引擎，runtime 与宿主不复制。

标准库 [`Ord::clamp`](https://doc.rust-lang.org/std/cmp/trait.Ord.html#method.clamp)要求下界不大于上界，起点先以实际编辑长度限制，空组字也可安全处理。复用仓库已有方向键的起点谓词，不增加依赖或宿主规则。

## Historical note audit

- [直接光标候选代次](../../implemented/bug-fix/2026-10-16-runtime-caret-candidate-generation.md)部分重叠，持有 runtime 的身份失效协议，本篇修正引擎可编辑范围。
- [光标拒绝键](../../implemented/bug-fix/2026-10-16-local-caret-rejected-keys.md)部分重叠，继续持有拒绝字符提交与透传规则；本篇只保证公开位置不会落在入口之前。
- [拆字模式](../../implemented/feature/2026-10-13-builtin-chaizi-mode.md)、[Shift 入口](../../implemented/feature/2026-10-11-shift-letter-entry-switch.md)与[网址补全](../../implemented/feature/2026-10-15-engine-email-url-completions.md)持有各自入口与编辑规则，本篇不改变其决定。其余 UTF-8、候选和光标性能笔记无关；没有完全吸收或过时提案。

## Alternatives considered

- **只在插入与删除处排除零。** 能挡住本次破坏，但快照仍会发布不能编辑的光标位置，每个消费者都必须再次修正。公开入口直接钳位与现有方向键规则一致。
- **让 runtime 拒绝零位置。** 能向调用方明确报错，但同一位置在普通方案和网址中合法，而且直接使用 `Session` 仍会破坏模式；由引擎按自身模式处理更集中。

## Verification

两项新增引擎回归在旧实现失败：`U0041` 前向删除得到 `0041`，插入 `+` 则以 `handled = false` 提交原文；真实 runtime 的同一插入回归也失败。修复后四项引擎回归和一项 runtime 回归通过，覆盖 12 种带标记模式的前向删除与起点退格，Unicode、临时英文和临时日文的插入，以及普通方案、专用英文、网址、裸标记、`None` 与上界钳位。默认并行完整测试通过：引擎 2230、golden 31、runtime 309，失败 0、忽略 37；两 crate 全目标 Clippy `-D warnings`、格式、笔记和差异检查通过。独立只读复审未发现阻断或重要问题，并核对其余光标赋值与模式入口；本地 quick 退出 0，覆盖仓库契约、Rust workspace、Android/WASM、Android Java、macOS 原生宿主及共享 Apple bridge；Windows 可移植测试 155 通过。Linux Docker、完整 Windows 交叉、HarmonyOS 真工程及 pipe-only 缺前置跳过，未声明设备验收。

## Consequences

直接 Rust 调用方请求标记之前的光标会得到第一个可编辑位置，符合方向键的边界。没有改变 C ABI 的按键路径或声明系统编辑器验收。

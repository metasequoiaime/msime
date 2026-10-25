# Agent Note: 切换热键的录入、冲突表与宿主共用布局键身份

Status: implemented

## Problem

#9298 / PR #9249 的录键页保存 DOM `KeyboardEvent.code`，macOS 却用布局字母匹配 `KeyA` 一族、用物理位置匹配标点。Dvorak 的物理句号键输出 `v`，于是两个经共享校验判为不同的 `Ctrl+Period` 和 `Ctrl+KeyV` 会同时命中。录入物理 E 键后保存 `KeyE`，真正响应的又是其他位置的布局 E 键。共享占用表还漏了 macOS 随 `switch_language_shift` 开关启用的 `Shift+Space`。

## Decision

三个可重绑热键的可打印主键都按当前布局的无修饰字符认，字母折成大写规范键名，数字与标点使用共享 `ChordKey` 名称；空格和功能键仍按具名键认。录入端把物理主键交给 macOS AppKit 的 `charactersByApplyingModifiers:0`，去掉 Shift、Option 和 Control 的字符变换后才交给共享 Rust 解析、规范化和冲突检查。手工配置与 MCP 已经提供规范主键，不经过录入转换。宿主对真实按键使用相同的无修饰字符与规范名表，两个不同的规范键不能再因混用布局字母和物理标点而同时命中。

保留非拉丁布局或缺少字符时的既有 ANSI 回退；不请求键盘监听、辅助功能或输入监控权限，不投递按键，也不改变系统输入源。AppKit 的无修饰重译不改变死键状态。主键表仅负责平台键名映射，字符串语法和占用判定仍由 `client-core` 持有。

`Shift+Space` 随 `switch_language_shift` 加入共享占用表，试写与落盘验证共用。macOS 已有 `Option+Shift+H` 的独立全半角入口也按对应开关占用，避免另一槽位抢走它；无开关的 `Ctrl+Shift+Meta+KeyK` 屏幕键盘入口同样保留。固定字母快捷键继续跟随布局，也使用同一个无修饰字符翻译，避免 Shift 或 Option 产生的字符变换让占用表与宿主分叉。

## Historical audit

- [可重绑切换热键](../../implemented/feature/2026-10-21-rebindable-toggle-hotkeys.md)：部分取代。本篇接手其 macOS 混合认键与直接保存 DOM code 的决定；字符串格式、三个槽位、默认值、MCP 和共享校验入口继续沿用。
- [macOS 字母快捷键跟随布局](../../implemented/bug-fix/2026-10-09-macos-letter-shortcuts-follow-keyboard-layout.md)：部分重叠。保留固定字母快捷键的布局语义及非拉丁回退，不把这些快捷键退回物理位置。
- [Linux IBus 保持键盘布局](../../implemented/bug-fix/2026-10-09-linux-ibus-keyboard-layout.md)：不改 Linux 宿主与输入源选择。
- 检索命中其余性能笔记中的 Dvorak 文本与本问题无关；没有其他热键录入身份提案。

## Alternatives considered

- 全部改为物理键：与 DOM code 一致，三个可重绑槽位的相等判定最简单。但 macOS 既有简繁、维护和全半角字母快捷键按布局认，同一物理键仍可能被固定动作占用；把固定快捷键一起改回物理位置又会撤销 #5552 的布局修复。
- 保留混合身份，在录入时附上当前布局冲突表：可以保留旧标点位置，但手工配置与 MCP 写入没有录入事件，布局切换后附带的冲突表也过时。统一可打印主键身份使共享保存校验仍能只靠文档判断三个槽位互斥。
- 在前端直接读取 `KeyboardEvent.key`：不增加原生转换，但 Option、Shift 与死键会改变字符，不能代表带这些修饰键的主键；AppKit 重译可显式去掉这些变换。

## Verification

- 先在原实现上复现 Dvorak `Ctrl+Period` 与 `Ctrl+KeyV` 同时命中，以及启用 Shift 切换时仍允许保存 `Shift+Space`。
- 录入、共享校验和宿主认键对 Dvorak 字母与标点位置、Shift 标点、Option 字母给出一致结果；三个槽位保持互斥。
- 开关开启时拒绝被固定入口占用的组合，关闭后允许重绑；MCP 与落盘验证不绕过该规则。
- 原有默认绑定、解绑、取消、重复按键与独立全半角入口回归通过；本地 quick、笔记门禁和 CI 通过后再合并。

已运行的证据：原实现上的 Dvorak 匹配测试与 `Shift+Space`、独立全半角保留键测试均按目标断言失败；修复后 `cargo test -p msime-client-core --lib toggle_hotkey --locked` 的 12 项通过。`ToggleHotkeysLayoutTest.cpp` 与 `ToggleHotkeyEventTest.mm` 直接编译执行通过，覆盖 Dvorak 字母与标点互斥、数字、功能键、非拉丁回退和去掉全部修饰键的事件边界。设置页新增的物理录键测试先因缺少布局转换标记失败（其余 6 项通过）。

修复后设置页 7 项测试通过，`pnpm --filter @msime/desktop build`（含类型检查）通过；`cargo check -p msime-host-macos --all-targets --locked` 与 `pnpm run verify-notes` 通过。

`cargo clippy -p msime-client-core -p msime-host-macos --all-targets --locked -- -D warnings` 通过；合入上游的两个 Vim 测试初始化按 clippy 要求改写为结构体初始化，不改断言或行为。`TAURI_CONFIG='{"bundle":{"resources":[]}}' cargo check -p msime-desktop --all-targets --locked` 通过：该检查只编译共享设置组件的 macOS 命令，关闭打包资源复制，不把 Tauri 当作输入法产品验收；已有 `DesktopMacVersion::parse` 未使用警告仍存在。

本地完整 quick 通过；原生 CTest 的 `toggle-hotkeys`、`toggle-hotkeys-layout`、`toggle-hotkey-event` 和 `shortcut-toggle-hotkeys` 四项通过。尚未声称真机 Dvorak 输入源或已安装产品验收。

首轮集成 CI 的 iOS、macOS、共享设置、Linux 与 Windows 构建通过；仓库契约因 Windows 设置说明契约过时失败，脚本插件因作者 SDK 缺少 `GRID_LIMIT = 11` 失败。两项基线缺陷分别由 #9389 和 #9399 修复并合入 `develop`，本 PR 不重复携带它们的补丁；合并仍需等待包含当前基线的最新一轮 CI 全部通过或按范围跳过。

## Consequences

Dvorak 下默认 `Ctrl+Period` 会跟随句号字符，原先物理句号位置改为按该位置的布局字符解释。这是统一认键所需的行为变化，不能声称仅修了错误提示。非拉丁字符回退与无字符合成事件需要独立回归；不以英文 QWERTY 上的成功替代布局用例。

[可编辑焦点限制](../feature/2026-10-24-macos-editable-shift.md) 只约束可选的单按 Shift，不改变本文的快捷键或状态归属规则。

# Agent Note: macOS 可配置的 K 模式附加入口

Status: implemented

## Problem

#11052 的英文状态不能用 Shift+K 进入快捷短语；用户已决定新增可配置快捷键，默认不占用 Ctrl+K，保留原 Shift+K。

## Decision

共享偏好增加 `keybindings.enter_quick_phrase`，缺字段和空串都不绑定，沿用共享 KeyChord 校验与冲突表。按维护者对 #11052 的明确授权，macOS 本次在旧 AppKit 回退设置里保留录键控件；不增加共享 React 设置。入口只在没有组字时生效；模式自己的开关仍生效，但不依赖 Shift+字母入口总开关。引擎以显式命令进入 K 模式，不把快捷键伪装成用户键入的大写 K。英文状态在临时输入期间保留，选词、取消或失焦后后续键继续原英文行为。已有 Shift+K 路径不变。

## Related decisions

[Shift+字母总开关](../../implemented/feature/2026-10-11-shift-letter-entry-switch.md)部分重叠：保留它对 Shift 入口的决定，补上独立入口。[原生界面迁移](../../proposed/architecture/2026-11-03-all-native-ui.md)部分重叠：新增控件放在 macOS 原生设置。

## Alternatives considered

- 默认增加 Ctrl+K：无需设置即可使用，但会抢终端与编辑器的既有按键，用户明确选择默认不绑定。
- 自动切到中文再模拟 Shift+K：复用现有调用，但会改变后续英文输入，并受到 Shift 入口总开关影响。

## Verification

共享偏好的缺键兼容、规范化保存、解绑、普通字符和 Shift-only 拒绝及双向冲突检查通过。C ABI 合成词库回放验证显式入口在 Shift 总开关两种状态下均选中真实短语，保留 Shift+K，并拒绝已组字、禁用模式和专用英文。macOS 的原生路由回归验证英文进入、取消后英文透传、安全输入、重复按键消费、在线英文托管与禁用模式；原生设置录键、冲突、Esc、解绑和重新创建对象后的持久化回归通过。设置卡片前后图来自基线和当前代码的真实 NSView，不读取用户偏好。`cargo test -p msime-client-core --lib`、`cargo test -p msime-host-api --lib`、相关 Clippy、原生测试和 `bash scripts/verify-local.sh --quick` 均通过；`pnpm run verify-notes` 与 `git diff --check` 通过。

原有未知命令测试用的 21 现已分配给 K 模式，因此将仍未分配的 99 用作未知编号，保留拒绝断言；不修改既有输入或默认开关来绕过测试。

## Consequences

用户自行选择附加组合键后，可在普通中英文状态进入 K 模式，退出后保持原来的中英文状态；默认不抢占 Ctrl+K。新增入口只支持 macOS，专用英文候选模式的显式入口仍不接管；语法和冲突规则、模式入口归共享 Rust。数字命令 21 是新的跨宿主契约，其余平台并不因此获得界面或快捷键。未在报告者终端、编辑器里安装验收，不能把模拟路由与原生控件测试写成真实编辑器验收。现有 Shift 总开关对原有 Shift+字母的行为保持不变，独立入口不受它控制。

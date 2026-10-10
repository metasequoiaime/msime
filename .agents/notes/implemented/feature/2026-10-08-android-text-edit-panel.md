# Agent Note: Android 键盘的文本编辑面板

Status: implemented

## Problem

#5625：编辑英文文档、在 MacroDroid 或网页输入框里精细调整文字时，要逐字移动光标、全选、复制剪切粘贴。Android 键盘里只有空格键拖动左右移光标，没有上下移动、选择、全选和剪贴板操作的入口，手指点文字又不准。用户给的参照是 Gboard 的编辑面板。

## Decision

- 功能面板第 3 页加一格「文本编辑」（`FunctionPanelModel.Id.TEXT_EDIT`，Lucide text-cursor-input 图标，经 `generate_keyboard_icons.py` 生成），点开 `ImeTextEditPanel`。面板是又一个工具栏面板：加入 `closeToolbarPanels`、`anyToolbarPanelOpen`、`alignOverlaysBelowTopRow` 和皮肤底色，打开功能面板时先关掉它（它后加入外框，否则会盖住功能面板）。只要有输入连接就可用，不要求引擎会话。
- 键和动作在无 Android 依赖的 `TextEditPanelModel`：4 × 4，左边 ← → 各占三行、中间 ↑ 选择 ↓，下排 移到开头 / 移到结尾 / 退格，右列 全选 / 复制 / 剪切 / 粘贴。视图用嵌套的 LinearLayout 按权重搭出同样的形状。
- 打开前由 Engine 完成组字，之后所有动作直接作用于编辑器，不经过 Engine：
  - 方向键发 DPAD 按键事件；「选择」开着时先发左 Shift 按下、再发带 Shift meta 的方向键、最后松开 Shift。EditText 的 `ArrowKeyMovementMethod.isSelecting` 只看文字缓冲里 MetaKeyKeyListener 记下的 Shift，不看事件的 meta；WebView 看事件的 meta，所以两样都给。事件的设备是 `VIRTUAL_KEYBOARD`，它的修饰键是 chorded，Shift 松开即清除，不会留下粘滞的 Shift。
  - 移到开头 / 结尾是 Ctrl+Home / Ctrl+End，到整篇文档两端（issue 说的是文本的头部和尾部，不是当前行）。
  - 全选、复制、剪切、粘贴走 `performContextMenuAction`。编辑器不支持时只有粘贴退回到直接上屏剪贴板文字。
  - 退格发 `KEYCODE_DEL`（有选区时删选区），复用删除键的连发与上滑快速删除。方向键按住连发，节奏同删除键。剪切、粘贴、退格之后关闭「选择」。

## Alternatives considered

- **用 `setSelection` 按位置移动光标和选区** — 不依赖编辑器对按键事件的处理，左右移动可以精确计算；但上下移动需要知道文字的排版（哪一行、哪一列），只有编辑器知道，宿主算不出来；按键事件让编辑器自己按它的排版移动。
- **复制、剪切在编辑器不支持菜单动作时由宿主读选中文字写剪贴板** — 能覆盖更多编辑器；但密码框禁止复制正是靠编辑器拒绝这两个动作，宿主自己读选区再写剪贴板会绕过它，而且写进去的文字还会被剪贴板历史记下。
- **全选后自动复制** — issue 原文提到「点击全选后会自动复制」，但同一句接着说「然后点击复制/剪切即可」，Gboard 也不这样做；自动复制会悄悄覆盖剪贴板，还会把整篇文字记进剪贴板历史，所以全选只选中。
- **在工具栏上放一个常驻按钮** — 少点一次；但工具栏按钮的开关要新增一项 Android 本地设置，并在 `crates/client-core` 的本机设置同步清单、设置页里各加一处，这次只放进功能面板（#6351 后来加上了，见 [2026-10-09-android-toolbar-text-edit](2026-10-09-android-toolbar-text-edit.md)）。
- **GridLayout 按模型的行列跨度直接排** — 视图与模型一一对应；但 GridLayout 的权重与跨行单元格的组合在不同版本上表现不一，无法在没有真机的情况下核对，嵌套 LinearLayout 的权重行为是确定的。

## Consequences

- **收益**：方向移动、选区、剪贴板操作和跳到首尾在一个面板里完成；键位、按键码、修饰键、菜单动作由 `TextEditPanelModelSmoke` 验证，`check-host.sh` 守住面板与其他工具栏面板一起关闭和计数。
- **代价与已知上限**：完全依赖编辑器对 DPAD、Shift、Ctrl+Home/End 和上下文菜单动作的处理，自定义输入框（游戏引擎、部分跨平台框架）可能不响应；不支持的编辑器里复制、剪切静默无效。入口原先只在功能面板里；#6351 之后工具栏上可以打开一个「文本编辑」按钮，见 [2026-10-09-android-toolbar-text-edit](2026-10-09-android-toolbar-text-edit.md)。
- **未验证**：没有在真机或模拟器上验收，选区延伸在 EditText 与 WebView 上的实际表现、面板在九键/单手/分离式键盘下的布局都只按源码推断。

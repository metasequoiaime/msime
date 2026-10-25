# Agent Note: Android 键盘的文本编辑面板

Status: implemented

## Problem

#5625：编辑英文文档、在 MacroDroid 或网页输入框里精细调整文字时，要逐字移动光标、全选、复制剪切粘贴。Android 键盘里只有空格键拖动左右移光标，没有上下移动、选择、全选和剪贴板操作的入口，手指点文字又不准。用户给的参照是 Gboard 的编辑面板。

## Decision

固定网格与七个侧边键的排序替代本篇的旧滚动布局，见[固定网格与七键排序](2026-10-24-android-text-edit-layout.md)；本篇保留旧布局的选择理由及编辑器动作约束。

面板打开时的复制预览及返回键由 [文本编辑中实时显示最近复制](2026-10-15-android-recent-copy-in-text-edit.md) 持有；本篇继续描述编辑器动作和键盘布局。

- 功能面板第 3 页加一格「文本编辑」（`FunctionPanelModel.Id.TEXT_EDIT`，Lucide text-cursor-input 图标，经 `generate_keyboard_icons.py` 生成），点开 `ImeTextEditPanel`。面板是又一个工具栏面板：加入 `closeToolbarPanels`、`anyToolbarPanelOpen`、`alignOverlaysBelowTopRow` 和皮肤底色，打开功能面板时先关掉它（它后加入外框，否则会盖住功能面板）。只要有输入连接就可用，不要求引擎会话。
- 键和动作在无 Android 依赖的 `TextEditPanelModel`：4 × 4，左边 ← → 各占三行、中间 ↑ 选择 ↓，下排 移到开头 / 移到结尾 / 退格，右列 全选 / 复制 / 剪切 / 粘贴。视图用嵌套的 LinearLayout 按权重搭出同样的形状。
- #8487 在右列增加可上下滚动的快捷键：数字键盘、空格和换行。原有四个剪贴板动作保持在首屏；数字键盘复用普通 `123` 层但总是进入数字层，空格与换行直接向当前 `InputConnection` 提交字面字符，不经过 Engine。
- 打开前由 Engine 完成组字，之后所有动作直接作用于编辑器，不经过 Engine：
  - 方向键发 DPAD 按键事件；「选择」开着时先发左 Shift 按下、再发带 Shift meta 的方向键、最后松开 Shift。EditText 的 `ArrowKeyMovementMethod.isSelecting` 只看文字缓冲里 MetaKeyKeyListener 记下的 Shift，不看事件的 meta；WebView 看事件的 meta，所以两样都给。事件的设备是 `VIRTUAL_KEYBOARD`，它的修饰键是 chorded，Shift 松开即清除，不会留下粘滞的 Shift。
  - #8175 的选择状态直接复用面板的 `selecting`：开启时键面和无障碍动作读作「取消选择」，关闭时恢复「选择」及原有「选择模式」描述。剪切、粘贴、退格和重新打开面板仍按已有规则关闭选择模式，标签随同刷新，不另存一份状态。
  - 移到开头 / 结尾是 Ctrl+Home / Ctrl+End，到整篇文档两端（issue 说的是文本的头部和尾部，不是当前行）。
  - 全选、复制、剪切、粘贴走 `performContextMenuAction`。编辑器不支持时只有粘贴退回到直接上屏剪贴板文字。
  - 退格发 `KEYCODE_DEL`（有选区时删选区），复用删除键的连发与上滑快速删除。方向键按住连发，节奏同删除键。剪切、粘贴、退格之后关闭「选择」。

## Alternatives considered

- **用 `setSelection` 按位置移动光标和选区** — 不依赖编辑器对按键事件的处理，左右移动可以精确计算；但上下移动需要知道文字的排版（哪一行、哪一列），只有编辑器知道，宿主算不出来；按键事件让编辑器自己按它的排版移动。
- **复制、剪切在编辑器不支持菜单动作时由宿主读选中文字写剪贴板** — 能覆盖更多编辑器；但密码框禁止复制正是靠编辑器拒绝这两个动作，宿主自己读选区再写剪贴板会绕过它，而且写进去的文字还会被剪贴板历史记下。
- **全选后自动复制** — issue 原文提到「点击全选后会自动复制」，但同一句接着说「然后点击复制/剪切即可」，Gboard 也不这样做；自动复制会悄悄覆盖剪贴板，还会把整篇文字记进剪贴板历史，所以全选只选中。
- **在工具栏上放一个常驻按钮** — 少点一次；但工具栏按钮的开关要新增一项 Android 本地设置，并在 `crates/client-core` 的本机设置同步清单、设置页里各加一处，这次只放进功能面板（#6351 后来加上了，见 [2026-10-09-android-toolbar-text-edit](2026-10-09-android-toolbar-text-edit.md)）。
- **GridLayout 按模型的行列跨度直接排** — 视图与模型一一对应；但 GridLayout 的权重与跨行单元格的组合在不同版本上表现不一，无法在没有真机的情况下核对，嵌套 LinearLayout 的权重行为是确定的。
- **把三个快捷键挤进 4 × 4 首屏** — 会缩小已有剪贴板按钮，且用户仍看不到更多操作；右列改为四行视口加滚动内容，保持原有编辑键的尺寸和位置。
- **让空格和换行复用普通键盘的 `space()` / `enter()`** — 这会重新进入 Engine 或触发编辑器 action；文本编辑面板的需求是直接编辑文档，所以改用 `commitText` 提交字面空格和换行。

## Consequences

- **收益**：方向移动、选区、剪贴板操作和跳到首尾在一个面板里完成；键位、按键码、修饰键、菜单动作由 `TextEditPanelModelSmoke` 验证，`check-host.sh` 守住面板与其他工具栏面板一起关闭和计数。
- **收益**：右列向上或向下滑动即可访问数字键盘、空格和换行，不必先退出文本编辑面板切换键区。
- **代价与已知上限**：完全依赖编辑器对 DPAD、Shift、Ctrl+Home/End 和上下文菜单动作的处理，自定义输入框（游戏引擎、部分跨平台框架）可能不响应；不支持的编辑器里复制、剪切静默无效。入口原先只在功能面板里；#6351 之后工具栏上可以打开一个「文本编辑」按钮，见 [2026-10-09-android-toolbar-text-edit](2026-10-09-android-toolbar-text-edit.md)。
- **验证边界**：EditText 的选择状态和选区延伸已在下述模拟器验收；WebView、自定义编辑器和九键/单手/分离式键盘下的布局仍未在这一轮验收。

## Verification

- #8175：`check-host.sh` 通过；Gradle 原生宿主 `assembleFullRelease` 通过。Java 改动重新编译，测试包复用 #8094 已验收的原生库和 staged 词库，不把这轮表述为 Rust 原生库重建。
- #8487：`check-host.sh` 通过（242 个 Android JVM smoke、150 个设备测试源编译），`TextEditPanelModelSmoke` 覆盖三项滚动快捷键、直接按键码和原有 4 × 4 网格。
- `TextEditRecentClipboardDeviceSmoke` 在 API 28、API 35 专用模拟器通过：开启后键面及无障碍描述为「取消选择」，方向键延伸 EditText 选区；关闭后恢复「选择」，下一次方向键只移动光标。原有复制、剪切、粘贴、顶部预览和返回键验收同时通过。
- `bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 与差异检查通过。

2026-10-19 整套设备回归复查：换行专项原来向单行 `EditText` 断言换行，数字页返回也硬编码旧英文键面 `ABC`。合成编辑器现在通过专用 Intent 参数为该套件启用多行，其余套件仍使用原单行输入框；返回数字页按稳定的「切换到字母键盘」无障碍动作查找，兼容当前中文九宫格。API 35 独立专项通过：滚动、空格、真实换行、数字往返、选择、复制剪切、预览和返回均执行。没有修改产品提交换行或数字页的行为。

#9332 的[上滑空格复制全文](2026-10-23-android-space-swipe-copy.md)沿用编辑器菜单权限与已有预览开关，新增手势入口和复制后原选区恢复。

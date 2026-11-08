# Agent Note: Android 文本编辑 ⇤ ⇥ 单击行首行尾、双击文首文尾

Status: implemented

## Problem

[无缝面板](../../implemented/feature/2026-11-06-android-text-edit-seamless-panel.md)去掉了行首、行尾键，[行级动作](../../implemented/feature/2026-11-07-android-text-edit-line-actions.md)把它们放到 ⇤ ⇥ 上滑，全选上滑选择当前行。两者都随 Android 0.7.1 发布后，仍有反馈说这三个功能没了（#11111）：上滑只在长按时弹出提示气泡，键面上看不出来。维护者在 #11111 建议单击行首行尾、双击文首文尾。

## Decision

⇤ ⇥ 经 `TextEditEdgeTapPolicy`：单击立即移到行首、行尾，不等第二下；`DOUBLE_TAP_MS`（暂取 `ShiftTapPolicy` 的 350 ms）内对同一个键再按一下，接着发原来的 Ctrl+Home / End，移到文首、文尾。超时的第二下、换了另一个键、按了面板上别的键、换输入框或同一输入框重启连接、面板关闭，都让下一下重新从单击算。第二下用掉这次双击，紧接着的第三下又是单击。

行按换行符分，和选择行相同。`ImeTextEditPanel.moveToLineEdge` 用和 `selectLine` 共用的 `fullText` 读完整的 `ExtractedText`，由 `TextEditLineSelection.lineStart` / `lineEnd` 找光标活动端（`selectionEnd`）两侧的换行，再 `setSelection`；选择模式下选区起点（`selectionStart`）不动，只移动活动端。光标在空行上时行首、行尾都是它自己；选择行在空行上选前一行，那是选择行原有的规则，这里不跟。读不到完整文本的输入框（终端、只给局部文本的 WebView）、密码框，以及 `setSelection` 被拒时，退回不带 Ctrl 的 Home / End，由编辑器自己决定移到哪里。

⇤ ⇥ 不再有上滑。全选上滑选择当前行不变，`TextEditSwipePolicy` 的判定也不变；提示从长按气泡改成键面右上角的淡色「选行」角标（`TextEditPanelModel.cornerHint`）。`TextEditHintButton` 继承 `KeyHintButton` 的角标画法（10 sp、上 3 dp、右 5 dp、kbSub 色），只画不进节点文字；每次排版后按字形墨迹判断（`TextEditCornerHintPolicy`），角标要整个落在键里，并且和居中的「全选」横向或纵向至少错开 2 dp 才画，否则不画，上滑照旧。三个键都不再 `setTooltipText`。

读屏名称改为「移到行首，双击移到开头」「移到行尾，双击移到结尾」。

## Related notes audit

- [行边界与换行长按符号](../../implemented/feature/2026-10-25-android-text-edit-line-keys.md)：本篇翻转了它「不读取文本找换行符，交给宿主的 Home / End」的取舍（理由见下面的备选方案），那篇已加指向本篇的说明；它的其余决定早已被无缝面板取代。
- [行级动作](../../implemented/feature/2026-11-07-android-text-edit-line-actions.md)：部分取代。⇤ ⇥ 上滑和三个键的提示气泡由本篇取代；全选上滑选择当前行和 `TextEditSwipePolicy` 保留。那篇已加指向本篇的说明。
- [选择当前行](../../implemented/feature/2026-10-26-android-text-edit-select-line.md)：部分重叠。共用完整文本的读取和行的定义，选择行本身的行为不变。
- [无缝面板](../../implemented/feature/2026-11-06-android-text-edit-seamless-panel.md)：部分重叠。布局、图标和键位不变，只改 ⇤ ⇥ 的动作和读屏名称。
- [双击 Shift 锁定大写](../../implemented/bug-fix/2026-10-08-shift-double-tap-caps-lock.md)：部分重叠。沿用它的窗口值；本篇的判定只看两次点按的间隔和键，不读选区回报，那篇记下的「WebView 在两次点按之间回报光标」不影响这里。
- 没有完全吸收的旧笔记或过时提案。

## Alternatives considered

- **不限时的「再按一下」**：只要光标没动，再按同一个键就到文首文尾。读屏用户也按得出来。但「光标没动」要靠编辑器的选区回报判断：WebView 会在两次点按之间回报，终端根本不回报，隔了很久的一下还会突然跳到文首尾。改用和 Shift 一样的时间窗口。
- **按显示行（Home / End 键码）**：行边界键当初的选择，能跟着自动折行，读不到文本的终端也收得到。但选择行一直按换行符分，同一面板上的「行」会有两种意思；#11111 的报告者和维护者说的行首行尾也是按换行分的行。读不到完整文本时仍退回 Home / End。
- **保留 ⇤ ⇥ 上滑，作为单击的等价操作**：照顾已经习惯 0.7.1 上滑的用户。但一个动作两种手势，上滑又看不出来；去掉。
- **长按触控板选择当前行**（#11111 原来的提议）：维护者没有采纳，选择行只保留全选上滑这一个入口。
- **角标放不下时也画**：键窄又矮时会压在「全选」上；放不下就不画，上滑照旧可用。

## Consequences

- 行首、行尾、文首、文尾都在一个键上，键面和布局不变；单击不等第二下，没有延迟。上滑选择行有了看得见的角标。
- 读屏下双击窗口几乎按不出来：0.7.1 的读屏用户单击就到文首文尾，现在单击只到行首行尾，文首文尾要等给 ⇤ ⇥ 加命名的无障碍动作。
- 每次单击读一次完整文本，和选择行一样；文本很长的输入框多一次读取。
- 350 ms 是暂定值，要在真机和模拟器上按快慢两种按法实测后再定。
- 已经习惯在 0.7.1 上滑 ⇤ ⇥ 的用户，上滑不再有作用。

## Verification

- JVM 冒烟：`TextEditEdgeTapPolicySmoke` 覆盖单击、窗口内双击、超时、换键、别的键、复位和时钟回退；`TextEditPanelModelSmoke` 覆盖行首行尾的边界（段中、换行符前后、空行、首末行、越界钳制）、只有全选有上滑和角标、⇤ ⇥ 的读屏名称；`TextEditCornerHintPolicySmoke` 覆盖角标横向、纵向错开和放不下。
- 设备：`TextEditRecentClipboardDeviceSmoke -e layout-only true` 里的 `TextEditLineDeviceChecks` 覆盖单击、双击、超时的第二下、选择模式两步、触控板模式、折行段落到整段首尾、终端退回 Home / End、⇤ ⇥ 上滑无效、三个键没有提示气泡和全选上滑选择当前行。

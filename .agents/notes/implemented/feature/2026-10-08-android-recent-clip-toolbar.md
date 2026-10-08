# Agent Note: Android 工具栏显示最近复制的文字

Status: implemented

## Problem

#5692：在别处复制了一段文字或表情，回到输入框要粘贴，现在得先点工具栏的剪贴板、再点那一条。用户希望刚复制的内容直接出现在键盘顶部那一行，点一下就粘贴。

## Decision

- 「最近复制」占工具栏那一行的位置（`ImeToolbar.addRecentClipRow`）：左边是文字预览（空白和换行并成一个空格，最多 40 个字），点按经 `insertClipboardText` 粘贴；右边 × 关闭。同高替换工具栏，出现和消失时键盘不跳。
- 状态在 `RecentClipboardSuggestion`（不依赖 Android）：复制后显示 60 秒；用过、关掉或开始打字（`render` 里 `!idle`）之后同一条不再出现，复制新的一条才会再出现。条目身份沿用 `ClipboardCapturePolicy.identity`（复制时刻加文字散列）。
- 来源有两个：复制监听（`clipboardWatcher`，读不到复制时刻时按现在算），和弹出键盘时的补看（`onStartInputView` 的刷新任务，先只读 `getPrimaryClipDescription()` 的复制时刻，过了 60 秒或读不到时刻就不读内容）。过期由 `main.postDelayed` 安排一次重画。
- 不显示的情况：本地设置 `platform.android.clipboard_suggestion`（默认开，只在本机）关着；隐私模式、密码类和不许个性化学习的输入框（`ImePrivacyGate`）；系统标为敏感的内容（`EXTRA_IS_SENSITIVE`）；工具栏被设为隐藏、有工具栏面板开着、正在组词。它与剪贴板历史开关无关，也不往历史里写任何东西。
- 设置入口在「键盘」页的「剪贴板」一节：「工具栏显示最近复制」。

## Alternatives considered

- **只在剪贴板历史开着时显示** — 和历史共用一个开关，设置少一项；但它不保存任何内容，只是读系统剪贴板当前那一条（任何输入法都能读），把它绑在一个默认关、而且要「保存」的开关上，大多数用户永远看不到它。
- **一直显示，直到下一次复制** — 不用计时；但几小时前复制的内容每次弹出键盘都占着工具栏，用户得每次手动关。60 秒与 Gboard 的做法相近。
- **放进候选条，和候选一起排** — 打字时也能看到；但打字时候选条属于 Engine 的候选，把剪贴板内容插进去要改候选的分页和选择编号。

## Consequences

- **收益**：复制后切回输入框，一次点按就粘贴；工具栏原有按钮不变，过期、用过或关掉后自动回来。
- **代价**：显示期间工具栏的按钮被替换，要用它们得先关掉或等它过期；工具栏设为隐藏时也不显示。关掉过的那一条只记在内存里，键盘进程被回收后、60 秒内再弹出键盘会再出现一次。

## Verification

`platforms/android/tests/clipboard/RecentClipboardSuggestionSmoke.java` 验证显示窗口、过期、关掉后不再出现、新复制重新出现、预览折叠与截断、本地设置默认值和只在本机；`bash platforms/android/check-host.sh` 编译服务与工具栏代码并运行它。设置页只在 Gradle 构建里编译。真机上的显示与粘贴没有在设备上验证。

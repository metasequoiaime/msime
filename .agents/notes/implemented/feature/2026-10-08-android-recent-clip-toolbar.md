# Agent Note: Android 工具栏显示最近复制的文字

Status: implemented

## Problem

#5692：在别处复制了一段文字或表情，回到输入框要粘贴，现在得先点工具栏的剪贴板、再点那一条。用户希望刚复制的内容直接出现在键盘顶部那一行，点一下就粘贴。

## Decision

- 「最近复制」占工具栏那一行的位置（`ImeToolbar.addRecentClipRow`）：中间是文字预览（空白和换行并成一个空格，最多 40 个字），点按经 `insertClipboardText` 粘贴；左边是和工具栏同款的剪贴板历史入口，右边是 × 关闭和收起键盘。同高替换工具栏，出现和消失时键盘不跳。第一版这一行只有文字和 ×：复制后的一分钟里要打开剪贴板历史或收起键盘，都得先点 × 把这一条永久关掉，所以把这两个与复制最相关的按钮留在这一行上。
- 状态在 `RecentClipboardSuggestion`（不依赖 Android）：复制后显示 60 秒；用过、关掉、开始打字（`render` 里 `!idle`）或从剪贴板插入过任何内容（`insertClipboardText`，含面板里的历史和分词）之后同一条不再出现，复制新的一条才会再出现。从面板插入也算，是因为否则在面板里点了刚复制的那一条，面板一关工具栏又换成刚插入的文字。条目身份沿用 `ClipboardCapturePolicy.identity`（复制时刻加文字散列）。
- 来源有两个：复制监听（`clipboardWatcher`，读不到复制时刻时按现在算），和弹出键盘时的补看（`onStartInputView` 的刷新任务，先只读 `getPrimaryClipDescription()` 的复制时刻，过了 60 秒或读不到时刻就不读内容）。过期由 `main.postDelayed` 安排一次重画。
- 跟着剪贴板历史开关走：历史关着时不显示，也不为它读系统剪贴板（连复制时刻都不读）；历史开着时听本地设置 `platform.android.clipboard_suggestion`（默认开，只在本机）。判断只有一处，`RecentClipboardSuggestion.enabled(clipboardHistoryEnabled, clipboardSuggestionEnabled)`，服务经 `recentClipEnabled()` 在补看读剪贴板之前和 `render` 里都问它。它本身不往历史里写任何东西。
- 键盘进程冷启动时，`clipboardHistoryEnabled` 在实时偏好到来前是字段初始值（关），弹出键盘时的补看因此会跳过；实时偏好把开关由关变开时（`reloadPreferences`、`loadAppearanceWithoutSession`）再补看一次（`offerRecentClipOnHistoryEnabled`），键盘进程不在时复制的那一条仍能出现。
- 其余不显示的情况：隐私模式、密码类和不许个性化学习的输入框（`ImePrivacyGate`）；系统标为敏感的内容（`EXTRA_IS_SENSITIVE`，补看读到后直接丢弃）；工具栏被设为隐藏、有工具栏面板开着、正在组词。
- 设置入口在「键盘」页的「剪贴板」一节：「工具栏显示最近复制」，说明里写明需先在「隐私」里打开剪贴板历史；历史关着时这一行置灰（`GroupCard.Row.setEnabled`，与本页「滑动方向」跟随「滑动输入符号」、工具栏按钮跟随「显示方式」同一做法），开关的值保留。

## Alternatives considered

- **与剪贴板历史开关无关，单独默认开（本 PR 第一版）** — 它不保存任何内容，只是读系统剪贴板当前那一条（任何默认输入法都能读，Gboard 也这样做），绑在默认关的历史开关上，大多数用户永远看不到它。不用的原因是用户的预期：关掉剪贴板历史的人想要的是键盘不碰复制的内容，而第一版在历史关着时仍然读出最近一分钟复制的文字、摆在键盘顶部，在共享屏幕或旁人看着时同样会露出来。产品决定是历史关着就不显示、不读。
- **一直显示，直到下一次复制** — 不用计时；但几小时前复制的内容每次弹出键盘都占着工具栏，用户得每次手动关。60 秒与 Gboard 的做法相近。
- **放进候选条，和候选一起排** — 打字时也能看到；但打字时候选条属于 Engine 的候选，把剪贴板内容插进去要改候选的分页和选择编号。

## Consequences

- **收益**：复制后切回输入框，一次点按就粘贴；工具栏原有按钮不变，过期、用过或关掉后自动回来。
- **代价**：剪贴板历史默认关，没打开历史的用户看不到这个功能；在设置里把历史从关打开时，一分钟内复制的那一条会马上出现。显示期间工具栏上除剪贴板和收起以外的按钮（品牌菜单、表情、常用语、皮肤、输入方式）被替换，要用它们得先关掉或等它过期；从面板插入一条旧的历史也会让刚复制的那一条不再出现；工具栏设为隐藏时也不显示。关掉过的那一条只记在内存里，键盘进程被回收后、60 秒内再弹出键盘会再出现一次。

## Verification

`platforms/android/tests/clipboard/RecentClipboardSuggestionSmoke.java` 验证显示窗口、过期、关掉后不再出现、新复制重新出现、预览折叠与截断、`enabled` 在历史关着时为假、本地设置默认值和只在本机；`bash platforms/android/check-host.sh` 编译服务与工具栏代码并运行它，另有源码守卫要求这一行保留剪贴板入口、`insertClipboardText` 收起这条建议，以及服务里本地开关只经 `RecentClipboardSuggestion.enabled` 使用、`offerRecentClip` 在读剪贴板之前先问 `recentClipEnabled()` 并丢弃敏感内容、`render` 也问它（在把这几处分别改回只看本地开关、去掉敏感判断的副本上确认守卫会失败）。设置页只在 Gradle 构建里编译。真机上的显示与粘贴没有在设备上验证。

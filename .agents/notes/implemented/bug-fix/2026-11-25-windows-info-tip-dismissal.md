# Agent Note: Windows 设置里点开的「?」说明气泡能被普通操作收起

Status: implemented

## Problem

Windows 11 的设置窗口里点开几颗「?」，气泡就都挂在窗口上不走了：点页面别处、按 Esc、滚到别处都关不掉，点开几颗就叠几片，跟普通应用里「点一下固定、点别处消失」的交互对不上。用户把叠了四片的截图发过来时，描述是「点开的提示框不会消失，这个和普通应用的交互不一致」。

原因在 `platforms/windows/settings/main.cpp` 的 `info_tip`：气泡的开关只有按钮 `Click` 里那一处 `shown.IsOpen(!shown.IsOpen())`，全文件没有第二个地方碰 `ToolTip.IsOpen`。`IsOpen(true)` 之后这颗气泡由我们保持打开，WinUI 的 `ToolTipService` 不再管它，于是只有再点同一颗问号才能关。

同一套设计语言在别的端都已经收起来了：共享设置页 `packages/ui/src/core/info-tip.tsx` 听 outside pointerdown（capture）、Escape、document scroll（capture）和 window resize；Win7 设置页的 `platforms/windows/settings/win7/InfoTip.h` 收 `WM_MOUSELEAVE`、`WM_KILLFOCUS`、`VK_ESCAPE`；Android 的 `InfoTipView` 用 `PopupWindow(true)` 让点外部自动 dismiss。[设置项说明收进标题旁的「?」](../feature/2026-11-01-settings-descriptions-behind-info-tip.md) 订的交互是「点一下固定，点别处或按 Esc 收起」，Windows 只做完了「固定」那一半。

## Decision

新增 `platforms/windows/settings/InfoTipHost.h`，里面的 `InfoTipPins` 只记「当前点开固定的那一颗」：问号和气泡各存一个弱引用，另存两者的 ABI 指针（`winrt::get_abi`）。弱引用任一侧失效、或 ABI 指针不像当初那两个对象（整页重画换掉了问号和气泡），就当作没有固定的气泡，此后什么也不做。

`main.cpp` 只放接入的那几行：

- `info_tip` 的 `Click` 调 `info_tips_.toggle(anchor)`：正固定着同一颗就收起，否则先收起上一颗再固定这一颗，所以气泡永远只有一片。
- `track_pointer(root)` 的 `PointerPressed`（`handledEventsToo`）调 `close_unless_inside(info_tips_.contains(args.OriginalSource()))`：点页面别处收起；落点在问号或气泡里不收起，因为那里的点击先跑完按钮自己的 `Click`、再冒到窗口，若再关一次就把刚固定下来的关掉了。
- 新增 `track_keys(root)`，在 Content 上以 `handledEventsToo` 听**隧道**的 `PreviewKeyDown`：Esc 且正有固定气泡时收起并 `Handled(true)`。走隧道而不是冒泡的 `KeyDown`，说明开着时一次 Esc 只关气泡，不会顺手清空搜索框或者关掉对话框。
- `scroll_.ViewChanged`、`render_page_at`（整页重画）、`on_activated` 的 `Deactivated` 各调一次 `close()`：气泡位置只在打开时量一次，滚动和重画会让它脱离所在的那一行；窗口失活后它还会盖在别的窗口上。

收起时只 `IsOpen(false)` 自己记下的那一颗，不去遍历整棵树关掉悬停弹出的那些：那几百颗归 `ToolTipService` 按指针离开自己关，我们只对「点开固定」的那一颗负责。

## Alternatives considered

- **在每个收起点上自己找「哪一颗是开着的」再关。** 不要额外状态，但每次点击都要遍历页面上的 ToolTip 才能知道该关哪一颗；`info_tip` 里问号和气泡的对应关系也只在 `ToolTipService` 里，遍历到气泡还得反查问号才能判断点在不在里面。记一颗弱引用更直接，也让「点同一颗问号是关、点另一颗是先关再开」这条规则有地方落脚。
- **点别处时遍历整棵可视树，把开着的 ToolTip 全关掉。** 一处代码关掉所有情况，但会把用户正在读的悬停气泡也关掉，而且「高级选项」页有几百颗问号，每次点击都遍历一遍。
- **不自己 `IsOpen(true)`，完全交给 `ToolTipService`。** 那就没有「点一下固定」：说明长一点的设置，指针一离开问号气泡就没了，而这一套设计语言里点开是要能慢慢读的。
- **听冒泡的 `KeyDown` 而不是隧道的 `PreviewKeyDown`。** 少一个「先关哪一层」的问题，但搜索框、对话框这类控件会先把 Esc 用掉；`handledEventsToo` 只保证我们也收到，改不了它们已经处理过的事实。

## Consequences

- 点页面别处、Esc、滚轮、切页、切到别的窗口、改一项设置引起的整页重画，都会收起点开的说明；连点几颗问号只会剩最后点的那一颗。
- 代价是说明开着时那一次 Esc 被吞掉：本来按 Esc 是清空搜索框查询或关对话框，现在先关气泡，再按一次才轮到它们。这与「最上面那层先关」的常规一致，但确实和改之前不同。
- 滚动收起意味着读长说明时滚一下就得重新点开。气泡位置只在打开时量一次，不收起的话它会飘到别的行上，共享设置页也选了同一条。
- 依赖 ABI 指针比较判断「还是不是当初那两个对象」：整页重画后旧状态自动作废，不需要在重画路径上做额外的清理登记。

## Testing

新增 `platforms/windows/tests/tools/settings_info_tip.ps1` 的 `Assert-InfoTipDismissal`，由 `platforms/windows/tests/tools/settings_ui.ps1` 在第一轮主题里跑一次。它开真实 WinUI 窗口，用真实鼠标、键盘和滚轮输入，按桌面上 `ControlType=ToolTip` 的元素计数（气泡是独立弹出窗口，不在宿主窗口的 UIA 子树里），依次断言：初始没有气泡 → 点开一颗有 1 颗且正文是该行的 → 点页面普通文字收起 → 再点开、Esc 收起 → 再点开、滚轮收起 → 再点开、再点同一颗收起 → 点开第二颗只剩第二颗 → 换页收起。

本机（Windows 11，真实窗口）实测：

- 未改动的 HEAD 构建：点别处、按 Esc 之后仍剩 1 颗气泡，点开四颗叠成三片（对照截图），与报告一致。
- 修复后的构建：上面六个断言连续三轮通过（Windows PowerShell 5.1，与 CI 同一个 shell）。连点四颗问号只剩最后一颗（对照截图）。
- 悬停弹出的那条路径没有验证：合成鼠标（`SetCursorPos` + `mouse_event`/`SendInput`）在本机的新旧两个构建上都触发不出悬停气泡，`ToolTipService` 要真实的指针停留序列。本次改动不碰这条路径——没有固定的气泡时 `dismiss()` 什么也不做。
- 点「?」这一下在窗口刚激活时会被吞掉（Windows 第一次点击只用来激活窗口），所以 `Open-SettingsInfoTip` 点完先确认气泡真的开了，没开再点一次；没开的气泡再点不会把它关掉。
- 本机跑整页冒烟（`settings_ui.ps1 -InputOnly`）还有两条与本次改动无关的失败：`settings_quick_phrase.ps1` 等窗口退出的 10 秒超时，在未改动的 HEAD 构建上同样复现（保存偏好后关窗卡住）；MCP「复制命令」按钮在 1.5 秒内变回「复制命令」的那一步在这台机器上偶发超时，单独拿同一个按钮做对照，HEAD 和修复版都能正常变成「已复制」再变回来。

## Related notes audit

[设置项说明收进标题旁的「?」，全平台设计语言](../feature/2026-11-01-settings-descriptions-behind-info-tip.md) 是这套交互的设计出处，本篇补上 Windows 原生设置窗口的收起行为，不改它的规则。检索 proposed、implemented、rejected，没有其他关于 Windows 说明气泡开关的笔记；`win7/InfoTip.h` 的收起条件只写在代码里，与本篇分属两套宿主。

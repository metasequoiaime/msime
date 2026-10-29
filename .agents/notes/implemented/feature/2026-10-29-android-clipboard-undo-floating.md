# Agent Note: Android 剪贴板删除后的浮动「撤销」与撤销时长

Status: implemented

## Problem

#10073：Android 键盘剪贴板面板里多选、勾选几条、删除后，「撤销」排在可以横滑的顶行末尾（[多选删除](2026-10-17-clipboard-multi-select.md)），常见手机宽度下要先往左滑才看得到，再去点一个小按钮，而撤销期限写死为 5 秒，报告者觉得来不及。诉求两点：删除后在面板右下角弹出一个大号「撤销」，倒数归零后消失；设置里能调撤销时长，5 到 9 秒，默认 5 秒。

[「更多」菜单](2026-10-29-android-clipboard-more-menu.md)（#10037）把分类、导出、清空收进了「更多」，顶行常见宽度下一行放得下，但删除后多出的「撤销」仍会把顶行撑宽，横滑的问题照旧。

多选删除和撤销只有 Android 有（iOS、鸿蒙的剪贴板面板没有多选，也没有 `removeMany` / 撤销），这次只改 Android。

## Decision

- **撤销不在顶行，浮在面板右下角。** `MSIMEInputService.clipboardUndoButton` 挂在 `clipboardSurface`（`FrameLayout`）上，`Gravity.BOTTOM | Gravity.END`，右、下边距 8dp，高 48dp（比 44dp 的「返回顶部」大一号），宽度随字，左右各 20dp 内边距，16sp。它由 `ImePanels.buildClipboardUndo` 用 `KeyboardKeyRole.RETURN` 的强调键帽造出来，和卡片、顶行的普通按钮区分开，换肤遍历按当前皮肤上色（收着时被跳过的子树由 `ImeStyler` 的 `restyleShown` 在显示前补一遍）。点按先收起「更多」菜单，再走原来的 `undoClipboardItems`。顶行里原来的「撤销」删掉，入口只有一个。
- **倒数。** 按钮字样是「撤销（N 秒）」，读屏说明是「撤销刚才删除的剪贴板记录，还剩 N 秒」。删除时记下截止时刻 `clipboardUndoDeadline`（`SystemClock.uptimeMillis` 加设置的秒数），投递 `clipboardUndoTick`；每次走到它时，剩余秒数大于 0 就按 `ClipboardUndoPolicy.millisUntilNextTick` 投递下一次（正好落在显示的数字减一的那一刻，最后一次落在截止时刻），否则丢掉可撤销的记录。每次只调 `ImePanels.updateClipboardUndo` 刷新这个按钮，不重画整个面板。撤销、再次删除和服务 `onDestroy` 都先撤掉还没执行的那次回调；倒数每秒重排自己，销毁时不撤的话会一直跑到期限，去刷新已经脱离窗口的按钮。
- **何时显示。** `updateClipboardUndo` 在剩余秒数大于 0、且面板画的是本机列表常态（`clipboardUndoAllowed`：不在云端分段、确认清空、多选、详情或分词里，剪贴板历史开着）时显示，条件和原来顶行里「撤销」出现的条件相同。`renderClipboardHistory` 一开头把它关掉并收起按钮，画到本机常态的顶行时再打开，所以所有提前返回的分支（详情、分词）都不会留着按钮。面板关着时倒数照走，期限内重新打开面板仍能撤销，和原来一样。
- **和「返回顶部」错开。** 「撤销」占住右下角；两者同时出现时，`ImePanels.layoutClipboardFloating` 把「返回顶部」的下边距改成 8 + 48 + 8dp，叠在「撤销」正上方；「撤销」收起后再落回角上。「返回顶部」和「撤销」各自的显示更新都会调它。
- **列表底部让出位置。** 同一个 `layoutClipboardFloating` 在「撤销」显示期间把 `clipboardPanel` 的下内边距从 8dp 加到 8 + 48 + 8dp，收起后改回 8dp。不让的话，列表刚好不够滚动或双列排布时，右下角那张卡片被按钮盖住，点不到，横滑露出的删除、固定也按不到，最长要等 9 秒；让出之后列表能滚到最后一行完整落在按钮上方。
- **撤销时长设置。** Android 本地设置 `platform.android.clipboard_undo_seconds`（`AndroidLocalSettings.CLIPBOARD_UNDO_SECONDS`），整数 5 到 9、步长 1、默认 5，只在本机、不随账号同步：同步字段表（`crates/client-services` 的 `ANDROID_LOCAL_SETTINGS`）没有这个键，和同组的「排列」`clipboard_columns` 一样。设置页「键盘 → 工具栏 → 剪贴板」在「排列」下面加一行「撤销时长」，点开 `OptionSheet` 列出 5 到 9 秒，搜索关键词同时加进 `PageId`。键盘在 `applyLocalSettings` 读进 `clipboardUndoSeconds`，删除那一刻取用，改了设置只影响之后的删除。删除后的提示改为「已删除 N 项，可在 N 秒内撤销」。
- 计时和文案放在纯 Java 的 `ClipboardUndoPolicy`：`seconds` 把 5..9 以外的值按 5 秒（设置文件读出的值已由 `AndroidLocalSettings` 校验，这里再兜一次），`deadline`、`remainingSeconds`（向上取整）、`millisUntilNextTick`，以及按钮、读屏、提示和设置页的文字。

## Alternatives considered

- **「撤销」放在「返回顶部」左边，两者并排**：位置固定，不用在运行时改「返回顶部」的边距。但「返回顶部」只在列表滚动后出现，大多数时候「撤销」会离右下角空出 52dp，和报告草图里贴着角的位置不一致；拇指在右下角最好按，「撤销」是限时动作，更该占角。
- **「撤销」显示期间直接藏起「返回顶部」**：最简单，不会重叠。但用户删除后往下翻找别的记录时，倒数那几秒里回不到顶部，两个按钮各管一件事，没理由互相让位。
- **保留顶行里的「撤销」，再加浮动按钮**：顶行那个仍要横滑才看得见，留两个入口只会让顶行继续超宽，「更多」菜单那次刚把顶行收到一行放得下。
- **撤销时长做成滑块或自由输入**：只有 5 个取值，`OptionSheet` 和同组的「排列」「历史记录上限」一致，报告者草图也是下拉选项。
- **把撤销时长放进共享偏好（`crates/client-core` 的 Preferences）**：架构上新设置的规则应下沉到共享 Rust 层，但多选删除、撤销只有 Android 宿主有，其它平台没有读它的地方；Android 专有、不进共享偏好的设置本来就归 `AndroidLocalSettings`，`clipboard_columns`、`clipboard_suggestion` 同样如此。哪天其它平台也有了多选撤销，再一起迁到共享偏好并进同步字段表。

## Consequences

- **收益**：删除后不用横滑，右下角一个大按钮直接点；倒数让人知道还剩多久；时长能放宽到 9 秒。顶行删除后不再被「撤销」撑宽，「更多」菜单那次的「常见宽度一行放得下」在删除后也成立。
- **代价**：浮动按钮在倒数期间盖住列表右下角的一块，不滚动时那块下面的卡片要往上滚一点才露出来；列表底部内边距随按钮出现、消失变一次，滚在最底下时列表会跟着挪 56dp；「返回顶部」在「撤销」出现、消失时上下跳一次。倒数每秒重设一次按钮的字和读屏说明，读屏焦点停在按钮上时会读到变化后的说明。
- 撤销仍只在当前输入法进程内有效，进程退出后不保留待恢复的数据；单条删除（#10080）还没有撤销，以后可以复用这个按钮和时长。
- `ToolbarEdgesDeviceSmoke` 里「更多」钉在滚动外的检查原来靠顶行里的「撤销」把 240dp 的顶行撑宽；现在从 240dp 往下每次收窄 20dp，直到顶行确实超宽再量，到 120dp 还不超宽就失败，免得这条检查静悄悄地测不到要测的情形。

## Verification

`tests/clipboard/ClipboardUndoPolicySmoke.java`（JVM，`check-host.sh` 自动发现并运行）覆盖：设置默认 5 秒、只在本机、5 到 9 接受、越界和非整数拒绝；`seconds` 越界回到 5；7 秒期限的倒数从 7 开始、向上取整、到截止归零；从任意时刻起逐次按 `millisUntilNextTick` 推进，每次正好减一，七次后正好落在截止时刻；按钮、读屏、提示和设置页的文字。`SettingsSearchIndexSmoke` 核对「撤销时长」进了设置搜索。

`ToolbarEdgesDeviceSmoke.checkClipboardUndoFloating` 用 30 条合成记录驱动真实的 `removeClipboardItems`：删除前按钮收着；把时长设为 7 秒后删一条，按钮出现、字样是「撤销（7 秒）」、挂在面板上而不在顶行或列表里，顶行没有旧的「撤销」；按钮是 `RETURN` 角色的键帽，把字色改乱后走一遍 `applySkin`，字色回到当前皮肤的 `returnForeground`；320dp、600dp 下滚到底，按钮高至少 48dp、贴在右下角 16dp 以内，「返回顶部」在它正上方、不重叠，列表最后一行整个落在按钮上方；点按恢复记录并收起；再删一次、把截止时刻改到过去后走一次倒数，按钮收起、记录没有被恢复，列表下内边距回到和上内边距相同，「返回顶部」落回角上。这条设备检查没有在模拟器或真机上跑过；深色模式和各皮肤下强调键帽的实际观感、倒数的实际节奏、读屏的朗读效果，都没有截图或真机确认。

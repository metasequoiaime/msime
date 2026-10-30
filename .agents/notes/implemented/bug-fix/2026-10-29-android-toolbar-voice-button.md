# Agent Note: Android 工具栏语音按钮回到工具栏

Status: implemented

## Problem

#10142：Android 用户在「语音输入 → 启动方式」选了「工具栏按钮」，键盘工具栏上没有语音按钮，「键盘 → 工具栏 → 按钮顺序」里也没有语音这一项，「更多工具」面板里的「语音输入」磁贴也不见了，键盘上一个语音入口都没有。

根因在键盘宿主。#3782 改版工具栏时，`ImeToolbar.installShortcutBar` 把 `voiceShortcutButton` 和回复、简繁、AI 润色一起当作退役按钮从工具栏上拆下，注释写着「语音由长按空格进入」；之后设置页又提供了「工具栏按钮」这个启动方式（写共享偏好 `touch_voice_shortcut`），却没人把按钮接回来。服务的 `render` 仍按 `touch_voice_shortcut` 把这个已经脱离视图树的按钮设成 `VISIBLE`，而 `ImeFunctionPanel` 用 `ViewPolicy.isVisible(s.voiceShortcutButton)` 判断「工具栏上已有语音」并藏掉磁贴，`isVisible` 只看 `getVisibility()`，于是选了「工具栏按钮」反而把最后一个入口也藏掉了。`ToolbarOrderPolicy` 只有九项，「按钮顺序」和设置页的工具栏开关列表里自然没有语音。[语音入口直接开始听写](2026-10-24-mobile-voice-entry.md) 写的「Android 顶部语音按钮按现有设置显示，显示时菜单去重」在本次修复之前并不成立。

空格键上的麦克风跟随长按空格开关，由[空格键麦克风跟随长按空格语音开关](2026-10-29-space-mic-follows-voice-trigger.md)（#10143）处理，本篇不重复。

## Decision

工具栏语音按钮是普通的中间区工具，开关是共享偏好顶层的 `touch_voice_shortcut`，与语音页「启动方式」选「工具栏按钮」写的是同一项。

- `ImeToolbar`：`voiceShortcutButton` 不再退役，排在 `shortcutButtons` 末尾，和其他工具一样参与长按进编辑、尺寸、配色和排序；`applyShortcutOrder` 增加 `VOICE` 分支。显隐仍由服务按 `touchVoiceShortcutEnabled` 设置，可点状态仍由 `voiceInsertionReady()` 决定，点按直接开始语音输入（`startVoiceRecognition`）。
- `ToolbarOrderPolicy` 增加 `VOICE("voice", "语音", "touch_voice_shortcut")`，排在默认顺序最后。旧版本存下的顺序里没有 `voice`，`parse` 把它补在末尾，原有顺序不变；排序仍只在本机，沿用[工具栏排序](../feature/2026-10-15-android-toolbar-order.md)的规则。
- `MSIMEInputService.toolbarRowHidden` 把 `touchVoiceShortcutEnabled` 算进「有没有工具按钮」：其他工具全关、只开语音时，工具栏这一行照样显示。
- `ImeFunctionPanel` 和表情、剪贴板等磁贴一样看开关不看视图：工具栏未隐藏且 `touchVoiceShortcutEnabled` 为真时才藏「语音输入」磁贴。工具栏「显示方式：隐藏」时磁贴一直在。
- 语音页「启动方式」选着「工具栏按钮」而工具栏「显示方式」是「隐藏」时，按钮不会出现（隐藏时中间区按钮全藏，长空格留着的那一行也一样）。行下写明「工具栏已隐藏，按钮暂不显示」，指向「键盘 → 工具栏」和「更多工具」里的「语音输入」磁贴（`VoicePage.TRIGGER_TOOLBAR_HIDDEN_HINT`），不再出现选了却看不到按钮又没有任何提示的情况。不在选的时候顺带改显示方式：那是工具栏整体的设置，用户隐藏它是为了只留候选条。
- 设置首页搜索：`PageId.KEYBOARD_OPTIONS` 的关键词补上工具栏开关「语音」，以及之前漏掉的「撤销」「重做」，`SettingsSearchIndexSmoke` 的计算行一起列出。
- 设置页「键盘 → 工具栏」：开关列表和预览按排序列出「语音」。`KeyboardOptionsPage.toolbarButton` 对语音读顶层 `touch_voice_shortcut`（缺键为关），`saveToolbar` 对语音写顶层这一项，不写进 `touch_toolbar`。打开「语音」和语音页选「工具栏按钮」走同一条规则：先把本地的长按空格语音写成关，再写 `touch_voice_shortcut`，空格键上的麦克风随之消失（#10143），这正是 #10142 要的「同时关闭空格键上的麦克风」；关掉「语音」只改这一项，不替用户打开长按空格。开关说明写明「打开时关闭长按空格语音」。
- 语音页「启动方式」由两个开关派生，长按空格优先。手势分页的「长按空格语音输入」仍可单独打开，两个都开时这里显示「长按空格」；`OptionSheet` 点已选中的项也会跑动作，所以 `pickTrigger` 对已选中的项直接返回，不再写回 `touch_voice_shortcut=false`、悄悄关掉工具栏按钮。
- 不改任何默认值：`touch_voice_shortcut` 仍默认关，没有选过「工具栏按钮」的用户工具栏不变。

iOS 的工具栏语音按钮一直按 `KeyboardLayoutPreference.voiceShortcutEnabled` 显示，鸿蒙按[全平台设计](../feature/2026-10-07-ios-harmony-all-platform-design.md)把语音放在功能面板、也不显示「启动方式」，两端都不改。

## Alternatives considered

- **删掉「工具栏按钮」这个启动方式**：Android 维持「语音只从长按空格和更多工具进入」，不用再接按钮。不用它，是因为这个选项和 `touch_voice_shortcut` 是跨端共享的设置，iOS 一直在用，Linux 和 Windows 7 的屏幕键盘也读它，账号设置同步（`settings_sync`）也带着它；Android 设置页和 README 都按有按钮来写，删掉选项要同时改几端的设置和同步语义，而用户要的就是这个按钮。
- **在工具栏设置里只放一行跳到语音页的入口，不做开关**：语音开关只在一处写，不会出现两处设置。不用它，是因为工具栏页其他每个按钮都是一个开关，语音单独成为跳转行很突兀；两处写的是同一个偏好键，状态不会分叉，开关说明里写明了打开它等同语音页选「工具栏按钮」。
- **工具栏页的「语音」只改 `touch_voice_shortcut`，不动长按空格语音**：两个入口各管各的，手势分页本来就有独立的「长按空格语音输入」。最初的实现是这样，评审时放弃了：两个都开时语音页「启动方式」只能显示「长按空格」，用户在那里再点一下已打钩的「长按空格」，`saveTrigger` 就写 `touch_voice_shortcut=false`，刚在工具栏页打开的按钮悄悄没了；开关说明「与启动方式的工具栏按钮是同一个开关」在这个状态下也不成立；空格键麦克风和工具栏按钮同时出现，正是 #10142 说的显示冲突、容易误触。两个开关在同一页的相邻分页，重画后用户看得见长按空格被关掉，说明里也写了。
- **手势分页打开「长按空格语音输入」时也关掉工具栏按钮**，让两个开关处处互斥：没有做。那是 #10142 之前就有的开关，改它的语义超出本次修复；两个都开的状态由 `pickTrigger` 的已选项不写回兜住，不会再丢设置。

## Consequences

选「工具栏按钮」以后工具栏上有语音按钮，「按钮顺序」里有「语音」，「更多工具」里的「语音输入」磁贴在按钮露出时去重、在「无」和工具栏隐藏时保留。原来选过「工具栏按钮」、或从别的设备同步来 `touch_voice_shortcut` 为真的用户，升级后工具栏上会多出语音按钮，这是该设置本来的含义。

代价：中间区多了一个可能出现的工具，窄屏上更容易需要横划；语音按钮默认排在最后，想放前面要去「按钮顺序」里调。

上面「iOS 一直在用」这条理由已不成立：iOS 工具栏不放语音按钮，也不读写 `touch_voice_shortcut`（[恢复 iOS 语音入口](2026-10-30-ios-voice-entry-restore.md)）。本篇的 Android 决定另有 #10142 的用户要求和其他平台的读取支撑，不变。

## Testing

- `ToolbarOrderPolicySmoke`：默认顺序以语音结尾、身份和偏好键、旧九项顺序原样保留并在末尾补上语音、重复项去重。
- `ToolbarVoiceButtonSmoke`（新增）：按源码钉住按钮回到中间区且不在退役列表里、参与排序、`toolbarRowHidden` 算上语音、设置页对语音读写顶层偏好且打开时关掉长按空格语音、语音页重新点已选的启动方式不写回、语音页与工具栏写同一个键、工具栏隐藏时语音页的说明、磁贴去重只在工具栏未隐藏时生效。按钮表只查语音按钮在不在中间区和退役表里，不钉整张数组。
- `ToolbarEdgesDeviceSmoke`（设备）：在真实服务的工具栏上确认语音按钮挂在中间区，按存下的顺序排到第一位。
- `ToolbarEditorDeviceSmoke`（设备）：十个按钮的「按钮顺序」面板折叠时保存、取消在可见区以外，进入面板的断言改为只要求保存节点存在。
- `SpaceKeyMicSmoke`：磁贴去重规则改为看 `touchVoiceShortcutEnabled`。
- 以上由 `platforms/android/check-host.sh` 跑（JVM 冒烟和宿主 Java/API 检查）；设置页 `home/` 由 Gradle `compileFullReleaseJavaWithJavac` 编译。
- 未在真机或模拟器上看过工具栏和设置页；设备套件只在 CI 的 API 35 模拟器上跑。

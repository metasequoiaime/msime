# Agent Note: 恢复 iOS 语音只从长按空格和功能菜单进入

Status: implemented

## Problem

2026-10-21 维护者要求 iOS 的语音入口只有长按空格：键盘工具栏不放语音按钮，「语音输入」页的「启动方式」三选一（长按空格 / 工具栏按钮 / 无）换成一个「长按空格说话」开关，iOS 不读也不写共享偏好 `touch_voice_shortcut`。改动写在 #9106 的分支上（`92a9713182`，同一分支上还有把语音与 AI 服务页套上卡片样式的 `c30be602bf`），却是在 #9106 合并之后才推上去的。GitHub 只合并了合并那一刻的分支头，之后推到已合并分支上的提交不会进 develop，也没有任何提示，这两个提交就此丢失。

丢失之后的工作都建立在「iOS 工具栏有语音按钮」上：[语音入口直接开始听写](2026-10-24-mobile-voice-entry.md)让 iOS 功能菜单按工具栏语音按钮是否露出去重，[空格键麦克风跟随长按空格语音](2026-10-29-space-mic-follows-voice-trigger.md)给 iOS「启动方式」的「无」补了说明，[Android 工具栏语音按钮](2026-10-29-android-toolbar-voice-button.md)否掉「删掉工具栏按钮这个启动方式」时把「iOS 一直在用 `touch_voice_shortcut`」当作理由之一。iOS 键盘还在每次出现时把文档里的 `touch_voice_shortcut` 镜像进 App Group，键盘里调一次高度就把镜像值写回文档，恢复几何默认值时把这一项从文档里删掉，等于 iOS 一直在改写 Android 的设置。

## Decision

- 两个提交原样摘回（保留作者），冲突按现在的代码改写：`c30be602bf` 无冲突；`92a9713182` 的工具栏按钮标签沿用现在的「语音输入」，「语音输入」页的开关换成 `DesignToggleRow`「长按空格说话」（`voiceSpaceToggle`），开着时说明「按住空格说话，松开上屏，上滑取消」，关着时沿用 #10143 的说法「长按空格改为移动光标；要说话可在「更多工具」里点「语音输入」」。`VoiceTrigger` 整个删掉。
- iOS 的语音入口是长按空格和功能菜单（「更多工具」）里的「语音输入」图块。工具栏的 `scriptShortcut` 保持隐藏，和已经移进功能菜单的高情商回复按钮一样留在栏里，按标识符 `layoutVoiceShortcut` 找它的地方不会找到别的控件。功能菜单不再按它去重，「语音输入」一直在。
- iOS 不读也不写 `touch_voice_shortcut`：`KeyboardLayoutPreference.voiceShortcutEnabled` 和它的 App Group 键 `keyboard.shortcut.voice` 去掉；`mirrorGeometry` 不镜像它；`geometryMapping`、`setTouchKeyboardGeometry`、`persistTouchKeyboardGeometry`、`saveGeometry` 不再带语音参数；`resetGeometry` 和 `resetTouchKeyboardGeometry` 不再从文档里删它。工具栏设置页的预览和皮肤预览也不再按它画语音入口。
- 本篇只收窄 iOS。Android 的「启动方式」与工具栏语音按钮照 [Android 工具栏语音按钮](2026-10-29-android-toolbar-voice-button.md) 不变，那篇的决定另有用户要求（#10142）支撑；它理由里「iOS 一直在用」这一条从此不成立。

## Alternatives considered

- **只摘回原提交，不动几何代码**：改动最小，原提交也只是把按钮藏起来、换掉设置页。但键盘调高度时仍按镜像值写 `touch_voice_shortcut`、恢复默认值时仍删它，和「iOS 不读也不写」对不上，同步到 Android 的设备上工具栏按钮会被 iOS 关掉或删掉。
- **连按钮带动作一起从工具栏删掉**：藏起来的按钮和它的点按动作从此走不到，删掉更干净。没这样做，是因为工具栏已有同样处理的高情商回复按钮，测试按标识符查找的写法依赖按钮还在；删按钮要连带改工具栏排布和这一类查找，收益只是少几行。
- **保留 #10152 起的三选一，只把「工具栏按钮」去掉**：剩下「长按空格 / 无」两项，本质就是一个开关，用选择器表达开关多点一次，维护者 10-21 要的就是开关。

## Consequences

收益：iOS 设置和键盘对语音入口的说法一致，没有关不掉的工具栏按钮；从 Android 同步来的 `touch_voice_shortcut` 不会再被 iOS 键盘调高度、恢复默认值盖掉或删掉。

代价：iOS 用户在别的设备上选了「工具栏按钮」，iOS 上不会出现这个按钮。原来在 iOS 上选过「工具栏按钮」的用户升级后工具栏上的语音按钮消失，长按空格语音保持他们当时的本机开关（选「工具栏按钮」时写成了关），要说话需从功能菜单进入或在「语音输入」页打开开关。App Group 里残留的 `keyboard.shortcut.voice` 不再有人读。

[按住空格说话](../feature/2026-10-21-ios-hold-to-talk-voice-session.md)、[语音入口直接开始听写](2026-10-24-mobile-voice-entry.md)和[空格键麦克风跟随长按空格语音](2026-10-29-space-mic-follows-voice-trigger.md)里关于 iOS 工具栏语音按钮和「启动方式」的部分由本篇取代，那几篇的其他决定不变，已互链。开着的 #10316 给工具栏设置页预览加了 `voiceShortcut` 参数，与本篇冲突，后合并的一方需要去掉它。

## Verification

Mac Studio 上经 `rbuild`，Xcode 27.1，`xcodegen generate`、`build-native.sh simulator`、词库暂存，一次性 iPhone 17 Pro / iOS 27.0 模拟器（脚本 trap 里删除）：`MSIMEApp` Debug 模拟器编译通过，`MSIMEClientTests` 全套结果见 PR。新增 `KeyboardGeometrySettingsTests.testGeometryNeverTouchesTheAndroidToolbarVoiceButton`：文档里 `touch_voice_shortcut` 为真时，App 保存几何、键盘提交几何、两种恢复默认值之后它都还是真。`NineKeyKeyboardTests.testTheToolbarNeverCarriesAVoiceButton` 对文档里这一项为真、为假两种情况核对工具栏按钮隐藏、功能菜单有「语音输入」。`SpaceVoicePanelTests` 的点按听写集成用例改从功能菜单图块进入。

没验证：没在真机或模拟器界面里看「语音输入」页的开关样子；没测从 Android 同步 `touch_voice_shortcut` 到 iOS 的真实账号同步。

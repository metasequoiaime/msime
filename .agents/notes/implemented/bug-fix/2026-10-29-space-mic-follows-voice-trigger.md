# Agent Note: 空格键麦克风跟随长按空格语音开关

Status: implemented

## Problem

#10143：Android 用户在「语音输入 → 启动方式」选了「无」，看不懂它是什么意思，又发现九键空格键上还画着麦克风。

「无」不会关掉语音识别，只是把长按空格语音（本地设置 `platform.android.space_voice`）和工具栏语音按钮（共享偏好 `touch_voice_shortcut`）两个开关都写成关，空格键和工具栏上从此没有语音入口；「更多工具」面板里的「语音输入」磁贴还在（`ImeFunctionPanel` 只在工具栏语音按钮露出时才藏它，iOS 的 `makeTools` 同理），点它照样能说话。长按手势本身一直按 `space_voice` 武装（`ImeBottomRow.bindSpaceCursor`），可空格键面上的麦克风只是装饰：`SpaceKeyFace.setShowsMic` 写好了却没有任何调用方，`showsMic` 永远是 true。于是选「无」或「工具栏按钮」以后，长按空格已经不能说话，键面却还在暗示按住能说话。iOS 同样如此：`KeyboardViewController.spaceKeyFace` 除日语外一律返回 `mic: true`，不看 `spaceVoiceEnabled`。HarmonyOS 的空格键按 `voiceEnabled` 决定画不画麦克风，也没有「启动方式」这个设置项，本来就是对的。

## Decision

空格键上的麦克风表示「长按空格可以说话」，所以它跟着长按空格语音开关走，不跟语音服务可不可用走。

- Android：`ImeBottomRow.spaceShowsMic()` 读 `AndroidLocalSettings.SPACE_VOICE`，`updateActionRow` 给主空格、`syncSplitSpace` 给分离键盘右半边的空格各设一次 `setShowsMic`。两处都在每次重画底行时跑，偏好重读后的下一次 `render` 就会换键面。
- Android 数字页的空格键面由[数字页空格键主标画 ␣](2026-10-29-android-number-pad-space-glyph.md)（#10059）取代：主标是 ␣，麦克风缩到右上角，仍只在开着长按空格语音时画。
- iOS：`spaceKeyFace` 多一个 `voice` 参数，`updateSpaceKeyTitle` 传 `spaceVoiceEnabled`。`readGesturePreferences` 发现开关变了、键盘也已经建好时立刻调 `updateSpaceKeyTitle`，从设置 App 改完切回键盘时键面马上变。
- 语音设置页「启动方式」：选择面板加一行说明，选中「无」时设置行下面也显示同样的说明。两端写的是各自的真实行为：Android 关掉长按语音后长按空格松手仍是空格，写「空格键和工具栏上不放语音入口，长按空格只输入空格；要说话可在「更多工具」里点「语音输入」」；iOS 关掉后长按空格改为移动光标（见[iOS 空格长按光标](../feature/2026-10-18-ios-space-cursor-and-quick-symbols.md)），前半句改写「长按空格改为移动光标」，后半句相同。说明不能写成「键盘上没有语音入口」：更多工具里的磁贴一直在，那样写设置页就和键盘对不上。
- 不改任何默认值：`space_voice` 仍默认开，原来没动过这项的用户看到的键面不变。

## Alternatives considered

- **麦克风同时看 `voiceInsertionReady()`**：长按手势武装时确实还要求不在组字中、不在本地输入模式，把它也算进来，键面和手势就完全同步。不用它，是因为组字状态每按一个键都会变，麦克风会跟着一闪一闪；组字中长按空格不进语音是一个短暂的状态，不是用户的选择，键面只表达用户的选择。
- **把「无」改名为「不使用」或「关闭」**：更直白。不用它，是因为「关闭」会让人以为识别语言、服务和离线模型这些设置也一起清掉了，而「无」只改两个入口开关，识别设置原样保留，切回别的启动方式就接着用；保留「无」再配一句说明，既不误导，也不动 Android 与 iOS 已经一致的选项名。
- **选「无」时把更多工具里的「语音输入」磁贴也藏掉**：这样「键盘上没有语音入口」就成立了。不用它，是因为 #10143 要的是把「无」解释清楚，不是改它的行为；更多工具是用户主动打开的菜单，留一个入口不会误触，也让选了「无」的人临时想说话时不必回设置页。
- **iOS 也照搬「长按空格只输入空格」**：三端文案一字不差。不用它，是因为 iOS 关掉长按语音后长按空格确实会移动光标，照搬就是写错。

## Consequences

选「无」或「工具栏按钮」以后，Android 和 iOS 的空格键上都不再画麦克风，主空格、分离键盘的两半和数字页一致；切回「长按空格」麦克风就回来。用户在设置页能直接看懂「无」意味着什么。

代价：Android 的 `SpaceKeyFace` 是 View，JVM 冒烟画不出来，`SpaceKeyMicSmoke` 按源码（先压空白）钉住两处 `setShowsMic` 和设置页说明；说明里提到的磁贴由同一个冒烟经 `FunctionPanelModel` 和 `ImeFunctionPanel` 的接线核对，以后选「无」若也藏掉磁贴，冒烟会提醒改说明。真实键面要在设备上看。

iOS 的「启动方式」后来换回维护者 10-21 定下的「长按空格说话」开关，「无」的说明随之改为开关关着时的说明，空格键麦克风仍跟随这个开关，见[恢复 iOS 语音入口](2026-10-30-ios-voice-entry-restore.md)。

## Testing

- Android：`platforms/android/check-host.sh` 跑全部 JVM 冒烟，其中 `SpaceKeyMicSmoke` 覆盖本次改动；Gradle `compileFullReleaseJavaWithJavac` 编译 `home/VoicePage.java`。
- iOS：`NineKeyKeyboardTests.testSpaceFaceNamesTheScheme` 补了关掉开关时三种键面都不带麦克风；`SpaceVoicePanelTests.testTheSpaceVoiceSwitchReachesTheSpaceBar` 断言关掉时空格键没有图标、打开后 `viewWillAppear` 一次麦克风就回来。
- 未在真机或模拟器上看过键面；Android 工具栏语音按钮的恢复在 #10142 处理，不在这里。

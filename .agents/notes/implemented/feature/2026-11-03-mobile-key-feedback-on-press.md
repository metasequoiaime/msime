# Agent Note: 移动键盘按下即给按键反馈

Status: implemented

## Problem

#10757：Android 键盘开着按键振动时，普通按键按下没有任何振动，手指抬起才振一下，打字有「不跟手」的延迟；只有退格是按下就振。

原因是移动端都把按键反馈放在按键动作里，而动作要等手指抬起：

- Android 的 `bindCountedAction`、`bindAction`、`iconKey` 都在 `OnClickListener` 里调 `ImeKeyFeedback.playFeedback`，`Button` 的点击在 `ACTION_UP` 之后才发生。退格是例外：没有快速删除撤回备份时，`ImeLetterRows.bindBackspaceRepeat` 在 `ACTION_DOWN` 里就 `performClick`，反馈跟着提前；有撤回备份时同样要等松手。`ImeKeyFeedback.onKeyDown` 原本是按下入口，但没有任何调用方。
- iOS 也一样，另有报告 #10768，由那边的 PR（#10775）单独改，这份记录不管 iOS。
- 鸿蒙的振动在 `KeyboardSession.tapFeedback`，由 `pressKey`、`pressGridKey`、`pressSpace`、`glide` 这些上屏路径调用，字母键靠 ArkUI 的点击触发，也在抬手之后。

[iOS 九键立即反馈](../bug-fix/2026-10-19-ios-nine-key-immediate-press-feedback.md)去掉的是按压动画的过渡，没有改反馈时机；[振动移出 UI 线程](../bug-fix/2026-10-27-mobile-haptics-off-ui-thread.md)（#10021）解决的是振动阻塞 UI 线程，也没有改时机。

## Decision

Android 和鸿蒙在同一个改动里，把按键区里按键的反馈挪到手指按下时，抬手上屏不再给第二次；iOS 的同一件事由 #10768 的 PR（#10775）做，规则相同，那边有自己的决定记录。不改任何设置的出厂默认值（按键音、振动开关和强度都不变），不加依赖。候选栏、工具栏和面板不算按键区，仍在点击时反馈。

**规则（各端相同）。** 按键区在一根手指按下、命中一个键时给反馈，并把这次按下记下来；这次按键的上屏路径来问反馈时，被记下的那一次消耗掉，不再出声、不再振动。按住期间的确认性反馈（长按弹出选项、按住连删、上滑清空待命、空格长按语音）不消耗，照常给。读屏激活、`sendActions` 这类没有触摸的上屏不经过按下，照常反馈。

**Android。**

- `KeyboardKeyArea`（26 键各行、九键的标点列和网格、底部功能行、大屏侧边数字键盘）加 `PressListener`：`ACTION_DOWN` / `ACTION_POINTER_DOWN` 在 `route()` 算出命中的键（包括落在空隙里、被让给左邻的）之后、交给键之前回调 `pressed`；同一根手指抬起时在交给键之前回调 `releasing`。两次回调返回的令牌在分发之后经 `post` 交还给 `settled`，排在框架投递的 `PerformClick` 后面。滑行接管、`ACTION_CANCEL` 之后不再回调抬起。不可用（`isEnabled() == false`）的键不回调。
- `ImeKeyFeedback` 实现这个接口。`pressed` 按键面替身映射（`visible`）后出声（`playSound(classify(key))`）、振动，并在 `KeyPressFeedbackPolicy` 里武装这个键；`releasing` 再武装一次。`playFeedback` 先问 `consume(visible(source))`：武装着就只做调试计数和按键动画，不出声不振动，所以计数和动画仍是每次按键一次、留在点击路径上。两段武装覆盖了退格在按下分发里自己点击、松手时点击、以及抬手时才上屏的滑动符号、九键滑出数字、日文九键滑动。
- `KeyPressFeedbackPolicy` 按令牌记武装，不按次数：快速连点同一个键时，上一次抬手排着的撤回不会撤掉这一次的武装。
- 抬手那一刻的确认性反馈改走不消耗的 `ImeKeyFeedback.confirm`：上滑清空松手执行（`ImeLetterRows`），以及松手才补判出的长按语音（`ImeBottomRow.bindSpaceCursor` 的 `startVoice`）。
- 「中/英」在没有会话、点按会被忽略时，点击监听本来就不给反馈、不计数（[中英键长按选输入法](2026-10-08-android-language-key-ime-picker.md)）。按下时同样不给：`MSIMEInputService.pressGivesFeedback` 用和点击监听同一个 `languageTap()` 判断，`pressed` 返回 null，这根手指抬起时也不武装。

**鸿蒙。** 触屏键盘装着 `keyFaces` 的那一列加 `onTouch`（`KeyboardView.keyPressTouch`）：按键的触摸处理完冒泡到这里，`keyFaces` 画着按键（`keyFacesShowKeys`，与 `keyFaces` 的分支一一对应）时每根手指按下调 `KeyboardSession.keyDown` 振动并留令牌，抬起或取消调 `keyUp`，会话在 `KeyPressFeedback.RELEASE_GRACE_MS` 之后撤回没用掉的令牌。`tapFeedback` 先问 `consume()`。`holdFeedback` 是确认，直接振动、不消耗；九键滑出的数字原来借 `holdFeedback` 补点按的振动，改用会消耗令牌的 `keyFeedback`。键盘收起（`keyboardHidden`）时丢掉所有令牌。令牌是每次按下新取的序号，ArkUI 的手指 id 会复用。

## 两端不一致的地方

- 鸿蒙只挪了振动。触屏按键音由各键的点击经 `softKeyPress(id)` 按键位 id 选样本，按下时在容器上拿不到是哪个键，所以按键音仍在抬手时出声。Android 的按键音跟着振动一起挪到按下。
- 鸿蒙的容器接住整块按键区，落在分离式键盘中缝、单手侧栏这类非按键位置的按下也会振一下；Android 的 `KeyboardKeyArea.route()` 只对命中键或键距的按下回调，这两类位置不振。九键左列的拼写列表是例外：它是可以滚动的列表，每次滚动的按下都振会很吵，而 Android 的拼写列表不在按键区里、也不振。列表自己的 `onTouch` 在按下时置上 `keyPressOutsideKeys`，冒泡到容器时 `keyPressTouch` 读掉并跳过这次按下；不调 `stopPropagation`，键盘根上清理按键气泡的 `pruneKeyPreview` 仍看得到这次按下。左列在不拼写时放的标点和数字层的符号是键，按下时照常振，与 Android 九键标点列在按键区里一致。
- 鸿蒙的令牌不认键：抬手后的宽限期里，别的上屏路径会消耗它。只会让读屏点击这类没有按下的上屏少振一次，不会让任何一次按下少振或多振。

## Alternatives considered

- **只在 `bindCountedAction` 等点击入口去重，按调用点区分「上屏」和「确认」**：语义最精确，不依赖分发时机。但 Android 有五十多处 `playFeedback`，散在 `ImeLetterRows`、`ImeBottomRow`、`ImeLayoutRows` 和服务里，漏改一处就是按下振一次、抬手再振一次；按分发窗口去重不必逐处改，只有抬手时的确认需要显式绕开，这类调用点少且容易认出。
- **按下时只振动，按键音留在点击**：改动小，不用在按下时判断按键音类别。Gboard 和 iOS 系统键盘都是按下时同时出声和振动，分开会变成「先振后响」。Android 能在按下时拿到键，所以一起挪；鸿蒙拿不到，见上一节。
- **鸿蒙给每个按键的 builder 加 `onTouch`**：能和 Android 一样只对按键振动、也能挪按键音。但按键散在几十个 `@Builder` 里（`flexibleKey`、`fixedKey`、`designRowKey`、`deleteKey`、`spaceKey`、九键、假名……），一处漏掉就回到抬手振动，而且要先给每个键接上按键音类别；这次先在容器上统一处理。
- **鸿蒙按手指 id 记武装**：实现更短。但 ArkUI 会复用手指 id，快速连点时上一次抬手排着的撤回会撤掉这一次的武装，于是这一次抬手又振一次，所以改成每次按下取新序号。

## Consequences

- 打字时每次按键在手指按下时出声振动，抬手只上屏。按下后滑走取消、滑行输入、空格拖光标、Shift、切层这些没有上屏的按压，以前不振，现在按下时振一下。
- 鸿蒙的退格、Shift、123 这类不经会话上屏路径的键，以前没有振动，现在按下时振动，与 Android 一致。
- 调试面板的按键计数和按键动画仍在点击路径上，每次按键一次。
- 去重依赖分发时机：Android 依赖 `PerformClick` 和 `KeyboardKeyArea.post` 在同一个主线程消息队列里先后排队；鸿蒙依赖点击在抬手后 `RELEASE_GRACE_MS`（100 ms）内到达。两端都要真机确认没有「按下振、抬手又振」。

## Verification

- `KeyPressFeedbackPolicySmoke`（Android JVM）和 `key-press-feedback.test.ts`（鸿蒙 node）按各端的调用先后模拟普通点击、退格按下即删、长按确认、多指、快速连点、没有上屏的按下和读屏点击。`scripts/test-android-key-sound-lifecycle.py` 编译真实的 `ImeKeyFeedback`，验证按下时响一次、这次点击不响、令牌交还后的点击照常响；键面替身（`stageFace`）登记后，按键区回调的键面和原按钮发出的点击算同一个键，按下分发和抬手分发里原按钮的点击都不响，抬手的确认（`confirm`）照常响。
- Android 的视图接线由设备套件的 `KeyPressAreaDeviceChecks` 覆盖（`smoke.sh` 默认的 API 35 套件经 `OptionSheetDeviceSmoke -e key-feedback press-area` 跑）：在合成的 `KeyboardKeyArea` 上分发 `MotionEvent`，核对直接命中、空隙、回车让位、多指各自回调，不可用的键、`ACTION_CANCEL` 和滑行接管之后不回调抬起，抬起的令牌在点击之后交还。宿主 JVM 上只有 `android.jar` 的空实现，`KeyboardKeyArea` 的分发跑不起来，所以放在设备套件里。键面替身映射本身（`ImeKeyFeedback.stageFace` / `visible`）由上面的按键音回归覆盖；`ImeBottomRow`、`ImeLetterRows` 是否给 ⇧ ⌫ 空格 ↵ 每一个都登记了替身，要真的服务建出键盘才看得到，留给真机验收。
- 鸿蒙 `tests/run.sh` 钉住 `keyPressTouch` 挂在装着 `keyFaces` 的那一列上、按下交给 `keyDown`、抬起和取消交给 `keyUp`、拼写列表置上 `keyPressOutsideKeys` 且不拦冒泡、`tapFeedback` 先消耗令牌，以及九键滑出数字经 `keyFeedback` 而不是 `holdFeedback`。
- `platforms/android/check-host.sh` 全部通过（含新冒烟和设备测试源码的编译）；`platforms/harmony/tests/run.sh` 通过，`scripts/test-harmony-arkts-subset.py` 通过。鸿蒙 HAP 没有编译：PR 上的 CI 不打 HAP，本机这次也没打。
- 真机验收还没做，留给设备轮次：Android 和鸿蒙开振动和按键音快速连打 26 键和九键，确认按下即振、抬手不再振；退格有、无撤回备份都只振一次；上滑清空松手、长按符号选项、九键滑出数字、空格拖光标和长按语音、滑行输入的反馈次数；Android 日文九键滑动输入假名只在按下时响一次，没有会话时按「中/英」不振，⇧ ⌫ 空格 ↵ 只振一次；鸿蒙按键音仍在抬手时出声，九键拼写列表滚动和点选不振。

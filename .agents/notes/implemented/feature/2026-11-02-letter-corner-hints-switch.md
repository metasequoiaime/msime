# Agent Note: 字母键角标可以隐藏

Status: implemented

## Problem

#10587（从 #10438 拆出）：三端 26 键字母键右上角无条件画角标符号（`1234567890`、`@#¥%&*()"`、`~…、?!-/`），一部分用户觉得键面太乱，希望能关掉；想保留的照旧显示。原先只有「滑动输入符号」的开关和方向，它只管手势，关掉后长按照样输入角标、角标也照样画：iOS 的 `updateLetterCaseControls` 里 `cornerLabel.isHidden = corner == nil`，Android 建键和 `updateLetterCornerHints` 都直接 `setCornerHint(LetterHintTable.face(...))`，HarmonyOS 的 `letterCornerHint` 注释写明无论「滑动输入符号」是否开启都绘制。

## Decision

- 三端各加一个「显示字母键角标」开关，默认开，老用户升级后键面不变。设置行放在「手势」组「滑动输入符号」和「滑动方向」之后、「九键滑动输入数字」之前；不跟着「滑动输入符号」置灰。
- 开关只管画不画。关掉后键面只留字母、主字母回到正中；按键上的角标符号照旧给出，长按照样弹出并输入它，开着「滑动输入符号」时滑动也照样输入。读屏文字本来就不读角标，不变。
- 存储跟随「滑动输入符号」，都是宿主本机设置：
  - iOS：App Group `keyboard.gesture.letterCornerHints`（`KeyboardLayoutPreference.letterCornerHints`，缺省开），不随设置同步。键盘出现时 `readGesturePreferences` 读一次，变了就调 `updateLetterCaseControls`；`LetterCaseInputs` 多了 `cornerHints` 一位，键面会重写。`cornerLabel` 在开关关闭时隐藏，`KeyboardKeyButton.cornerHint` 照旧赋值。「重置所有设置」清掉这个键。
  - Android：本地设置 `platform.android.letter_corner_hints`（`AndroidLocalSettings.LETTER_CORNER_HINTS`，缺省开），与 `swipe_down_symbols` 一样参与账号同步，client-services 的 `ANDROID_LOCAL_SETTINGS` 登记同名同步键；服务端字段表收录之前上传时会被滤掉，只存在本机。建键时和 `updateLetterCornerHints` 里按它决定 `setCornerHint` 给符号还是 `null`。本地设置变了 `reloadPreferences` 会 `render()`，`render()` 每次都调 `updateLetterCornerHints`，所以不用进 `touchGeometryKey()`。手势在 `bindLetterGestures` 里按 `LetterHintTable.hint(input)` 取符号，不读键面。
  - HarmonyOS：键盘自己的 `key-feedback.json` 里的 `letterCornerHints`（`FeedbackSettings`，缺省开），经 `KeyboardFeedbackBridge` 给共享设置页，账号同步只带三个反馈字段，`fromShared` 从已存设置沿用它。字母键的 `KeyOptions` 多一个 `showCornerHint`，`keyFaceHasCornerHint` 看它决定画不画；`cornerHint` 照旧交给 `letterHintTouch`。`faceKey()` 带上这一位，键盘开着时改了已画好的字母键会重建，`tests/run.sh` 锁住这几处接线。
- 共享设置页（HarmonyOS 用）：`MobileKeyboardFeedback.letterCornerHints` 可选，宿主给出时在「手势」组画这一行，保存时整份反馈记录原样送回。
- 只作用于 26 键字母键。九键、14/18 键、数字层和底行句号键上的角标不在这次范围里。

## Alternatives considered

- **和「滑动输入符号」合成一个开关**：设置少一行，「不画角标」和「不能滑出角标」看起来是一回事。不采用，三端现在的语义是关掉滑动后长按仍能输入角标；合并后关开关会顺带让角标不可见，用户就不知道长按还能出符号，或者为了藏角标而失去滑动。两件事分开，各管各的。
- **关掉角标时连长按也不再输入符号**：键面和行为完全一致，没有「看不见却按得出」的情况。不采用，符号层和 123 键已经有这些符号，但长按出符号是已经有人在用的手势，隐藏键面只是为了清爽，没必要连带收走输入路径；报告者也说的是「不用显示」。
- **iOS 和 HarmonyOS 也进账号同步**：三端行为对齐。不采用，「滑动输入符号」在这两端本来就只存在本机（iOS App Group、HarmonyOS 的反馈文件，账号同步只带三个反馈字段），这一项跟着它走；要同步得先把整组手势设置接进去，超出这次范围。

## Consequences

- **收益**：不需要角标的用户可以把键面收成纯字母，手势一个不少；三端同一行文字、同一个位置。
- **代价**：三端各多一个本机设置要维护；HarmonyOS 的 `FeedbackSettings` 多一个必填字段，以后每个整份构造它的地方都要带上；Android 本地设置的同步键从 18 个变成 19 个。
- **待确认**：三端关闭角标后在真机上看字母是否回到键面正中、长按和滑动是否仍输入角标；Android 设置页（`home/*` 不在 `check-host.sh` 的 javac 范围内）的编译交给 CI。

## Verification

- `rbuild cargo test -p msime-client-services --lib -- settings_sync`：41 项通过（同步表多了 `platform.android.letter_corner_hints`，导出和应用往返）。
- `rbuild cargo fmt --all -- --check`：通过。
- `rbuild bash platforms/android/check-host.sh`（编译机上的 Android SDK）：通过，含 `AndroidLocalSettingsSmoke`（缺省开、参与同步、与 client-services 同步表一致）和设置搜索索引检查（`PageId` 加了「显示字母键角标」）。
- iOS：编译机上 iOS 27.0 模拟器跑 `xcodebuild -scheme MSIMEClientTests -only-testing:MSIMEKeyboardTests/LetterCornerHintTests test`，8 项通过，新加的 `testCornerHintSwitchHidesTheSymbolButKeepsItsGestures` 核对缺省开、关掉后角标行隐藏而 `cornerHint` 和 `onCornerHint` 还在、键盘再次出现时就地收起和画回。
- HarmonyOS：`bash platforms/harmony/tests/run.sh` 在本机用 node 跑（编译机上没有 node），通过，含新加的 `keyboard-logic.test.ts` 分组（缺省开、旧文件读成开、非布尔值回退、页面读写、账号同步保留、移动和调整键盘时保留）和 `faceKey()`、`showCornerHint` 的接线锁。
- 共享设置页：同样在本机，`apps/desktop` 的 vitest 跑 `touch-keyboard-geometry-section.test.tsx` 等 32 个相关文件，837 项通过。

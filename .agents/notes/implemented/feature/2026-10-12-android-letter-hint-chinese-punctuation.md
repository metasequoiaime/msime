# Agent Note: Android 字母角标在中文标点下打中文标点

Status: implemented

## Problem

#7069：Android 26 键字母右上角的提示（`LetterHintTable`，q1…p0 / a@ … l" / z~ … m/）下滑或长按时按字面上屏，走 `commitNineKeyLiteral`：先发 `MSIME_COMMIT_RAW`，再把字符原样写进输入框。这条路不经过 Engine 的标点路由，也不看「中文标点」开关，于是中文模式下滑 v 得到半角 `?`；组字中下滑会把拼音原文和标点一起上屏（`nihao?`），而不是「你好？」。底行逗号键、句号键和 123 层早就按中文标点上屏，只有角标这一条路是 ASCII，用户只能长按逗号键去找中文标点。

## Decision

- 中文标点生效时（`MSIMEInputService.sendsChinesePunctuation()`，它已经排除了英文、日语、韩语、越南语、藏文和本地模式），j k l v b 的角标画 （ ） “ ？ ！，键面由 `LetterHintTable.face(letter, chinesePunctuation)` 给出，映射复用 123 层的 `ChineseSymbolFaces`，不另起一张中文角标表。其余 21 个角标（数字、`@ # % & * ~ - /`、¥、…、、）在任何状态下都不变。
- 下滑或长按松手时，`ImeLetterRows.bindLetterGestures` 对 `LetterHintTable.routesThroughEngine(hint)` 为真的提示（单个 ASCII 字符、且 `ChineseSymbolFaces` 有中文形式，即 ( ) " ? !）调用 `MSIMEInputService.punctuation()`；Engine 不收时（会话还没起来）退回 `commitNineKeyLiteral`。写法与注音九键的逗号键、`commitNineKeyPeriod` 相同。`ChineseSymbolFaces` 的每一项都在 `crates/engine/src/punctuation.rs` 的 SIMPLE 表、引号交替或书名号嵌套之内，所以这个判定与 Engine 是否转写一致。
- `punctuation()` 在组字中先按高亮候选结束组字再接中文标点（input-runtime 的 `punctuation` 路由），智能标点、成对补全和跨过后半个、引号交替都在里面，角标因此得到与逗号键相同的行为。l 键键面画前引号 “，实际上屏交给 Engine 交替成 “ 或 ”。
- 上屏走 `punctuation()` 而不是 `type()`：`type()` 末尾会 `letterCase.consumeLetter()`，按过一次性 Shift 后滑出标点会把 Shift 用掉；按字面上屏的其他角标从不消耗 Shift，两类角标保持一致。`type()` 多出来的微软双拼 `;` 判定和 `DeclinedKeyPolicy` 回退对 ( ) " ? ! 用不到。
- 中文标点不生效时一字不改：键面和输出仍是 ASCII 提示，经 `commitNineKeyLiteral` 上屏（藏文下滑 m 仍是 `/`）。
- 中文标点状态在手势触发时读，不在建键时记下。`render()` 里紧跟 `updateSymbolKeyFaces()` 调 `updateLetterCornerHints()`，遍历 `shuangpinKeyButtons`（只有 26 键字母层通用分支建的键，含分离式键盘右半边重复的 G、V；跳过微软双拼的 `;`）就地 `setCornerHint`，工具面板、Ctrl + .、设置页切换中文标点或切换中英文后角标立即跟着变，不必扩展 `layerPunctuationStale` 去重建整排键。
- 只改 Android。iOS（`LetterHintTable.swift`、`KeyboardViewController` 的 `onCornerHint`）和 HarmonyOS（`LetterHintTable.ts`、`KeyboardView.ets` 的 `typeLetterHint`）的角标仍按字面上屏；三端的规范表本身不变，`hint()` 仍是同一张表，差别只在键面和路由。

## 与其他宿主跟进时的注意点

- iOS 判断「Engine 会写中文标点」要用 `symbolLayerEngineWritesChinesePunctuation`，它同时看 `punctuation_lock`，不能只看中文标点开关；Android 宿主从不调用 `msime_client_set_punctuation_lock`，lock 一直是 0，所以这里只看 `sendsChinesePunctuation()`。iOS 的单测在 `platforms/ios/KeyboardTests/input/LetterCornerHintTests.swift`。
- HarmonyOS 的 `typeLetterHint` 对同一组字符改走 `KeyboardSession.shared.punctuation(code)`（与 `tapSymbol` 末尾相同），Engine 不收时再 `commitRaw` 加 `insert`；手势在 `keyboard/input/LetterHintGesture.ts`，测试在 `platforms/harmony/tests/keyboard-logic.test.ts`。

## Alternatives considered

- **另起一张中文角标表，中文标点下直接上屏中文字面**：最直接，iOS 和 HarmonyOS 也能各抄一份。但字面上屏要先 `MSIME_COMMIT_RAW`，组字中照样会把拼音原文和标点一起上屏，issue 里「你好？」这个核心场景修不好；它还绕开了成对补全、跨过后半个和引号交替，l 键永远只能打出前引号。复用 `ChineseSymbolFaces` 也保证角标、123 层和 Engine 三处的中文形式是同一份。
- **切换中文标点时重建整排键（扩展 `layerPunctuationStale`）**：与 123 层的做法一致，键面状态只在建键时算一次。但字母层重建会丢掉正在按的键、按键气泡和分离式键盘的就地状态，而角标只是 `KeyHintButton` 上画的一个字符，`setCornerHint` 遇到相同值直接返回，就地改代价最小。
- **松手时经 `type()` 上屏**：`type()` 是字母层其他键的统一入口，ASCII 标点也会转给 `punctuation()`。但它会消耗一次性 Shift，按 Shift 后滑出一个标点、下一个字母就变回小写，与按字面上屏的数字、`@` 等角标不一致。

## Consequences

- **收益**：中文标点下角标打出的标点与逗号键、123 层一致；组字中滑出标点得到「首选词 + 中文标点」，而按字面上屏得到的是「拼音原文 + ASCII 标点」，这一点对用户是可见的行为变化。成对标点开着时下滑 j 得到（|），光标在两半之间，规则与 [Android 成对标点自动补全](2026-10-08-android-paired-punctuation.md) 同一份。
- **代价与已知上限**：`sendsChinesePunctuation()` 在 view 为 null 时为假，键盘刚弹出、会话还没起来的一两秒内角标是 ASCII，view 到达后换成中文，有一次键面变化。（ ） “ 是全角字形，墨迹偏在字身一侧，`KeyHintButton.drawCornerHint` 按 `Paint.Align.RIGHT` 右对齐画，在角标位置可能显得偏移，需要设备截图确认；偏得明显时在那里按墨迹对齐。注音方案没有被 `shouldUseChineseFaces` 排除，但大千布局不画角标，( ) " ? ! 也不是注音键；以后注音换上标准 26 键时要复查 `punctuation()` 的注音分支。iOS 和 HarmonyOS 跟进之前，三端角标在中文标点下的键面和输出不一致。
- **范围外**：123 / #+= 设计层的字符键同样走 `commitNineKeyLiteral`（`ImeLetterRows.designLayerKey`），切到这一层（`layerButton` 的点击只切换 `keyboardLayer` 并重建键行）不结束组字，所以组字中切到 123 层点 ？ 仍会上屏「拼音原文 + ？」。这里不改。
- 逗号、句号键上「上下滑加左右滑」选标点（issue 的第二个建议）没有做，它是一种新手势，要先定方向、候选集、引导层和与长按菜单的关系。

## Verification

- `platforms/android/tests/keyboard/LetterHintTableSmoke.java` 锁住 26 键的 `face(·, false)` 与 `hint()` 一致、`face(·, true)` 只换 j k l v b、`routesThroughEngine` 只对 ( ) " ? ! 为真；`check-host.sh` 自动发现并运行它。
- `platforms/android/tests/device/keyboard/LetterHintPunctuationDeviceSmoke.java` 注入按下、移动、抬起三个触屏事件下滑 v：中文标点下空闲得到「？」，打 nihao 后得到「你好？」，在「更多」面板关掉中文标点后得到 `?`。JVM 冒烟覆盖不到经 Engine 结束组字这一步，所以要在设备上驱动真实手势；它由 `smoke.sh` 在 API 35 模拟器上运行。

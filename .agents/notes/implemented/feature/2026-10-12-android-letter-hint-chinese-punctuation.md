# Agent Note: Android 字母角标在中文标点下打中文标点

Status: implemented

## Problem

#7069：Android 26 键字母右上角的提示（`LetterHintTable`，q1…p0 / a@ … l" / z~ … m/）下滑或长按时按字面上屏，走 `commitNineKeyLiteral`：先发 `MSIME_COMMIT_RAW`，再把字符原样写进输入框。这条路不经过 Engine 的标点路由，也不看「中文标点」开关，于是中文模式下滑 v 得到半角 `?`；组字中下滑会把拼音原文和标点一起上屏（`nihao?`），而不是「你好？」。底行逗号键、句号键和 123 层早就按中文标点上屏，只有角标这一条路是 ASCII，用户只能长按逗号键去找中文标点。

#9483：英文键盘的 D、X、C 角标分别是 `¥`、`…`、`、`，手势触发后交给 `commitNineKeyLiteral`。这条入口曾把全部单个 UTF-16 字符交给 `commitEnglishLiteral`，后者只接受 32–126 的 ASCII，三个非 ASCII 角标因此被直接丢弃。字符表和滑动阈值都没有缺项。

## Decision

- 中文标点生效时（`MSIMEInputService.sendsChinesePunctuation()`，它已经排除了英文、日语、韩语、越南语、藏文和本地模式），j k l v b 的角标画 （ ） “ ？ ！，键面由 `LetterHintTable.face(letter, chinesePunctuation)` 给出，映射复用 123 层的 `ChineseSymbolFaces`，不另起一张中文角标表。其余 21 个角标（数字、`@ # % & * ~ - /`、¥、…、、）在任何状态下都不变。
- 下滑或长按松手时，`ImeLayoutRows.commitLetterHint` 对 `LetterHintTable.routesThroughEngine(hint)` 为真的提示（单个 ASCII 字符、且 `ChineseSymbolFaces` 有中文形式，即 ( ) " ? !）调用 `MSIMEInputService.punctuation()`；Engine 不收时（会话还没起来）退回 `commitNineKeyLiteral`。写法与注音九键的逗号键、`commitNineKeyPeriod` 相同。`ChineseSymbolFaces` 的每一项都在 `crates/engine/src/punctuation.rs` 的 SIMPLE 表、引号交替或书名号嵌套之内，所以这个判定与 Engine 是否转写一致。
- `punctuation()` 在组字中先按高亮候选结束组字再接中文标点（input-runtime 的 `punctuation` 路由），智能标点、成对补全和跨过后半个、引号交替都在里面，角标因此得到与逗号键相同的行为。l 键键面画前引号 “，实际上屏交给 Engine 交替成 “ 或 ”。
- 上屏走 `punctuation()` 而不是 `type()`：`type()` 末尾会 `letterCase.consumeLetter()`，按过一次性 Shift 后滑出标点会把 Shift 用掉；按字面上屏的其他角标从不消耗 Shift，两类角标保持一致。`type()` 多出来的微软双拼 `;` 判定和 `DeclinedKeyPolicy` 回退对 ( ) " ? ! 用不到。
- 中文标点不生效时，键面和输出仍是原角标提示，经 `commitNineKeyLiteral` 上屏（藏文下滑 m 仍是 `/`）。
- 英文键盘的 `commitNineKeyLiteral` 仅将单个 ASCII 字符交给原有英文纠错与边界处理；非 ASCII 文本复用下方的结束组字、全角转换和字面提交路径。D、X、C 在两种滑动方向下均提交原角标，不将中文顿号替换成逗号，也不扩展实体按键共用的 `commitEnglishLiteral(int)` 契约。
- 中文标点状态在手势触发时读，不在建键时记下。`render()` 里紧跟 `updateSymbolKeyFaces()` 调 `updateLetterCornerHints()`，遍历 `shuangpinKeyButtons`（只有 26 键字母层通用分支建的键，含分离式键盘右半边重复的 G、V；跳过微软双拼的 `;`）就地 `setCornerHint`。工具面板和 Ctrl + . 走 `toggleChinesePunctuation()`，那里本来就无条件 `rebuildKeyRows()`，新键建出来时按当时状态取键面，不靠这一步；就地更新要覆盖的是另外三种不重建字母层的情形：设置页改「中文标点」（`applyPreferences` 只调 `applyChinesePunctuation`）、切换中英文（`toggleInputLanguage` 布局不变时只就地改大小写）、以及会话起来前 view 为 null、到达后 `sendsChinesePunctuation()` 由假变真。为这三种情形扩展 `layerPunctuationStale` 去重建整排键代价过大，见下面被否掉的方案。
- 只改 Android。iOS（`LetterHintTable.swift`、`KeyboardViewController` 的 `onCornerHint`）和 HarmonyOS（`LetterHintTable.ts`、`KeyboardView.ets` 的 `typeLetterHint`）的角标仍按字面上屏；三端的规范表本身不变，`hint()` 仍是同一张表，差别只在键面和路由。

字母选项中的符号也经同一入口提交；选项滑动、方向设置及关闭规则见 [字母选项手势与设置](../bug-fix/2026-10-21-android-letter-option-gesture-settings.md)。

## 与其他宿主跟进时的注意点

- iOS 判断「Engine 会写中文标点」要用 `symbolLayerEngineWritesChinesePunctuation`，它同时看 `punctuation_lock`，不能只看中文标点开关；Android 宿主从不调用 `msime_client_set_punctuation_lock`，lock 一直是 0，所以这里只看 `sendsChinesePunctuation()`。iOS 的单测在 `platforms/ios/KeyboardTests/input/LetterCornerHintTests.swift`。
- HarmonyOS 的 `typeLetterHint` 对同一组字符改走 `KeyboardSession.shared.punctuation(code)`（与 `tapSymbol` 末尾相同），Engine 不收时再 `commitRaw` 加 `insert`；手势在 `keyboard/input/LetterHintGesture.ts`，测试在 `platforms/harmony/tests/keyboard-logic.test.ts`。

## Alternatives considered

- **将 `commitEnglishLiteral(int)` 扩展为接受全部 Unicode 码点**：能统一英文状态下的字符提交，但该入口还处理实体按键、英文九键和纠错边界，需要同时审视代理项、控制字符与调用方的按键过滤。角标已有适用的字面提交路径，因此只修复调用方错误分流。
- **另起一张中文角标表，中文标点下直接上屏中文字面**：最直接，iOS 和 HarmonyOS 也能各抄一份。但字面上屏要先 `MSIME_COMMIT_RAW`，组字中照样会把拼音原文和标点一起上屏，issue 里「你好？」这个核心场景修不好；它还绕开了成对补全、跨过后半个和引号交替，l 键永远只能打出前引号。复用 `ChineseSymbolFaces` 也保证角标、123 层和 Engine 三处的中文形式是同一份。
- **切换中文标点时重建整排键（扩展 `layerPunctuationStale`）**：与 123 层的做法一致，键面状态只在建键时算一次。但上面三种情形原本都不重建字母层，为角标改成重建，会把 #6997 第 5 项避开的问题带回来：重建先 `removeAllViews` 再建新键，新键布局前宽高为 0，这段时间的按压会丢，按在旧键上的手指收不到抬起（见 `ImeLetterRows.refreshLetterCase` 的注释）。角标只是 `KeyHintButton` 上画的一个字符，`setCornerHint` 遇到相同值直接返回，就地改代价最小。工具面板和 Ctrl + . 的切换本来就重建，不在这次的考虑之内。
- **松手时经 `type()` 上屏**：`type()` 是字母层其他键的统一入口，ASCII 标点也会转给 `punctuation()`。但它会消耗一次性 Shift，按 Shift 后滑出一个标点、下一个字母就变回小写，与按字面上屏的数字、`@` 等角标不一致。

## Consequences

- **收益**：中文标点下角标打出的标点与逗号键、123 层一致；组字中滑出标点得到「首选词 + 中文标点」，而按字面上屏得到的是「拼音原文 + ASCII 标点」，这一点对用户是可见的行为变化。成对标点开着时下滑 j 得到（|），光标在两半之间，规则与 [Android 成对标点自动补全](2026-10-08-android-paired-punctuation.md) 同一份。
- **代价与已知上限**：`sendsChinesePunctuation()` 在 view 为 null 时为假，键盘刚弹出、会话还没起来的一两秒内角标是 ASCII，view 到达后换成中文，有一次键面变化。（ ） “ 是全角字形，墨迹偏在字身一侧，`KeyHintButton.drawCornerHint` 按 `Paint.Align.RIGHT` 右对齐画，在角标位置可能显得偏移，需要设备截图确认；偏得明显时在那里按墨迹对齐。注音方案没有被 `shouldUseChineseFaces` 排除，但大千布局不画角标，( ) " ? ! 也不是注音键；以后注音换上标准 26 键时要复查 `punctuation()` 的注音分支。iOS 和 HarmonyOS 跟进之前，三端角标在中文标点下的键面和输出不一致。
- **范围外**：123 / #+= 设计层的字符键同样走 `commitNineKeyLiteral`（`ImeLetterRows.designLayerKey`），切到这一层（`layerButton` 的点击只切换 `keyboardLayer` 并重建键行）不结束组字，所以组字中切到 123 层点 ？ 仍会上屏「拼音原文 + ？」。这里不改。
- 逗号、句号键上「上下滑加左右滑」选标点（issue 的第二个建议）没有做，它是一种新手势，要先定方向、候选集、引导层和与长按菜单的关系。

## Verification

- #9483 代码提交 `07d7aa6ea0` 的完整 `bash scripts/verify-local.sh --quick`（`20261024-092526`）退出码为 0：契约、Rust workspace、Android 目标、Android Java/JNI 与共享 Apple 桥接检查通过，其他平台阶段按改动范围跳过。本机与远端 3015 份相关受控文件哈希一致；复制的只有已有 Rust/Android 缓存，受控源码只刷新时间戳，不改内容。笔记在本机单独校验。报告人的 API 30 实际设备尚未现场复验，继续保留 `Refs #9483`。
- #9483 的 `english-swipe` 专项使用独立的 26 键期望表，真实注入 DOWN/MOVE/UP 检查上滑、下滑全部角标、D 取消不提交、关闭滑动后正常输入字母及符号后继续输入字母。配置和输入模式在结束时恢复；该专项接入普通 `smoke.sh`。在 API 28（`emulator-5566`）和 API 35（`emulator-5590`）上均通过。设备包重编本分支 Java 与资源，复用已存在的 arm64 原生库；结果证明 Android 宿主手势路径，不代表本分支 Rust 核心重新构建验收。
- #9483 的 Android 宿主检查通过：258 项 JVM 冒烟、225 份设备源码编译、Java/API 与 JNI 检查通过；仓库契约、笔记、shell 语法与差异检查通过。契约检查中按环境跳过的阶段不算运行验证。本机空间低于 50 GB 门槛，没有新建本机 Rust 构建树；完整门禁在远端复用已完成阶段的 APFS 缓存。
- 推送到 PR 后的 Android host、Rust、契约、笔记、Windows/Linux/HarmonyOS、macOS 与 iOS CI 均通过；Android device job 按仓库规则跳过，因此仍不能替代 API 28/35 设备专项。
- `platforms/android/tests/keyboard/LetterHintTableSmoke.java` 锁住 26 键的 `face(·, false)` 与 `hint()` 一致、`face(·, true)` 只换 j k l v b、`routesThroughEngine` 只对 ( ) " ? ! 为真；`check-host.sh` 自动发现并运行它。
- `platforms/android/tests/device/keyboard/LetterHintPunctuationDeviceSmoke.java` 注入按下、移动、抬起三个触屏事件下滑 v：中文标点下空闲得到「？」，打 nihao 后得到「你好？」，在「更多」面板关掉中文标点后得到 `?`。JVM 冒烟覆盖不到经 Engine 结束组字这一步，所以要在设备上驱动真实手势；它登记在 `smoke.sh` 的默认套件里，在 API 35 模拟器上运行。
- 写这篇笔记时还没跑过的：这个设备冒烟本身、`build-apk.sh` 完整出包，以及手工验收（全角 （ ） “ 在角标位置的对齐、成对标点开时下滑 j 得到（|）、l 键连续下滑交替出 “ ”、按过 Shift 后滑出标点 Shift 不被消耗、各语言模式下与改动前一致）。实际跑过的只有 `check-host.sh`（含扩展后的 `LetterHintTableSmoke`，设备套件源码在 `-Werror` 下编译通过）和 Gradle 的 `compileFullReleaseJavaWithJavac`。`DeviceSmoke.tap` 的注释提到 Android 15 可能拒绝向输入法窗口注入坐标，而这个冒烟的下滑正是靠 `UiAutomation.injectInputEvent` 注入的；若在 API 35 上注入被拒或被吞，要改用 `smoke.sh` 里手写套件那套 root `sendevent` 触屏桥，而不是把它列进跳过名单了事。

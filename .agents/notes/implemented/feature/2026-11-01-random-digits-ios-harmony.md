# Agent Note: iOS 与鸿蒙的随机数字九宫格

Status: implemented

## Problem

#10254 要一个可选的随机数字九宫格：设置里的「随机数字」只决定工具栏上出不出现「随」，点「随」才打乱中文数字九宫格的 0–9，每输入一个数字重新打乱，英文键盘不受影响。Android 由 PR #10361 实现（分支 `feat/android-random-digits-10254`，决策笔记 `.agents/notes/implemented/feature/2026-10-30-android-random-digits.md`，写这篇时还没合进 develop）。按手机端 UI 一致性规则，键盘工具栏和设置的改动要三端一起有，iOS 和鸿蒙在 develop 上都还没有这个功能：iOS 的 1–9 由 `KeyboardViewController` 按 `rowIndex * 3 + column + 1` 固定生成，0 键写死；鸿蒙的数字层是 `NineKeyLayout.digits()` 的固定表，0 键写死在 `nineKeyZeroKey`。

## Decision

两端的行为逐条照搬 Android 的 `RandomDigitOrderPolicy` 和它在键盘里的接线：

- 键序策略各写一份同语义的纯代码：iOS `KeyboardExtension/Sources/keyboard/RandomDigitOrderPolicy.swift`（struct），鸿蒙 `entry/src/main/ets/keyboard/input/RandomDigitOrderPolicy.ts`。键位 0–8 是网格的 1–9，键位 9 是底行的 0；Fisher–Yates 打乱，结果恰好等于上一次时强制交换前两位；没开或没有资格时返回标准键序且不取随机数；打开后第一次在资格成立时取键面才打乱，离开数字页不取新随机数，回来沿用；每个数字上屏后重新打乱。随机源可注入，测试用固定序列。
- 随机资格与 Android `randomDigitsEntryVisible && numberPadDrawn` 相同：入口设置打开、工具栏没设成隐藏、方案是中文（全拼、双拼、五笔、粤拼、注音、笔画，九键、14/18 键和手写归在它们下面）、此刻是中文、没有临时模式，并且画着的就是数字九宫格。英文键盘不画「随」，数字页也不打乱。
- 点「随」只切换开关和高亮，不跳页；数字键按它此刻画的数字走数字页原来的上屏路径（iOS `typeNumberPadText`、鸿蒙 `typeNumberPadText`，本地模式和越南语、藏文里照旧交给会话），统计按这个数字记，读屏名称跟着键面换，然后重新打乱。符号、退格、空格、回车和左列不动。
- 键区不重建时资格也会变（在数字页上进出临时模式、切中/英、换方案、改工具栏设置），键面要当场按资格对一次：iOS 在 `render` 里调 `syncRandomDigits`，布局时 `applyNineKeyDigitLayer` 也按策略取键面；鸿蒙在 `syncStripHeight`（中/英、临时模式、方案和工具栏设置都挂着它）和 123 与 #+= 之间的切换（`moreSymbolsChanged`）里调 `syncNumberDigitFaces`。这些核对不在数字页时什么都不做。
- 运行时开关不保存，入口设置关掉或工具栏设成隐藏时清掉，取消隐藏后「随」仍是关着的。
- 工具栏那一行：「随」只在中文方案的键盘上算一个工具，只开着它时这一行留着，切到英文时「随」不在但这一行不消失，与 Android 的 `randomDigitsKeepsToolbarRow` 相同。

### 入口设置存在哪里

- iOS 放在本机的 `TouchToolbarLocalPreference.randomDigits`（App Group 键 `keyboard.toolbar.random_digits`，默认 false），和常用语、输入方式、显示方式一起；共享的 `touch_toolbar` 的校验器会丢掉不认识的成员，放不进去。开关在 App「键盘工具栏」页，说明文字与 Android 同一句；预览按同一条规则画「随」（`KeyboardToolbarPreviewRow`）。
- 鸿蒙放在共享偏好 `harmony_toolbar.random_digits`（client-core `HarmonyToolbarPreferences.random_digits`，默认 false），沿用这组偏好的先例：鸿蒙的本机设置就是共享偏好，设置页和键盘读写同一份文档。开关在共享设置页的「屏幕键盘 → 工具栏」（`packages/ui` 的 `harmonyToolbarOptions`），只在宿主报告 `touch_toolbar_tools` 时出现，选了「隐藏」时置灰。`TouchToolbarPolicy.rowHidden` 多一个 `chineseScheme` 参数：视图按引擎方案传，冷启动建面板时按偏好里的 `scheme` 传（`KeyboardSession.toolbarRowHidden`）。设置页「没有打开的工具，工具栏不显示」的说明用同一条规则，按偏好里的方案（`isChineseScheme(draft.scheme)`，经 `harmonyChineseScheme` 传进工具栏分组）决定「随机数字」算不算一个工具。
- 默认值等于缺省时 `harmony_toolbar` 照旧不写进文档；这个结构不拒绝未知字段，已发布的鸿蒙版本读到带 `random_digits` 的文档照样能读，只是保存时会丢掉它。

### 与 #10361 的关系

#10361 只改 `platforms/android` 和它的笔记，不碰本篇改动的共享文件（`crates/client-core/src/preferences.rs`、`packages/ui`），两个 PR 合并顺序无关。三端的设置名、说明文字、读屏名称（「随机数字」，状态「已开启」/「已关闭」）和工具栏字「随」相同。读屏状态 Android 用 `stateDescription`、iOS 用 `accessibilityValue`，鸿蒙用 `accessibilityDescription`，和功能面板的开关磁贴（`PanelTile`）一样，同时保留 `accessibilitySelected`。

## Related notes audit

检索活跃笔记中的「随机数字」「harmony_toolbar」「TouchToolbarLocalPreference」「数字九宫格」。[鸿蒙手机工具栏可选工具](2026-10-17-harmony-toolbar-tools.md) 持有 `harmony_toolbar` 和 `rowHidden`，本篇往里加一个字段、给 `rowHidden` 加方案参数，那篇的决定不变；[中文默认数字九宫格](2026-10-15-mobile-default-number-pad.md) 的数字上屏路径和统计身份继续沿用；[九键滑动输入数字](2026-10-29-nine-key-swipe-digits.md) 只作用于字母层，不受影响；[大屏单手侧边数字键盘](2026-10-29-tablet-one-hand-side-number-pad.md) 的侧边拨号盘不打乱，与 Android 相同。

## Alternatives considered

- **鸿蒙的入口也像 iOS、Android 那样只存本机**：不改 client-core，旧版本完全看不到这个键。但鸿蒙没有独立的本机设置文件，要新增桥方法和页面侧读写；而 `harmony_toolbar` 本来就是只给鸿蒙的一组工具栏开关，多一个字段是最小的改动。
- **鸿蒙 `rowHidden` 不看方案，把 `randomDigits` 当成普通工具**：签名不变、调用处少改。但日文、韩文等方案不画「随」，只开着它时这一行会只剩标志和收起，和 Android 不一致。
- **鸿蒙每次 build 都按策略现算键面**：不用另存 `numberPadDigits`。但策略第一次取键面会打乱，ArkUI 的 build 里不该改状态；按状态存键面、在资格可能变的地方核对，和 Android 只在资格翻转时重画是同一个思路。
- **「随」的高亮经 `toolbarTextButton` 的参数传进去**：少一个 builder。但按值传给 @Builder 的参数变了不会让它重画，开关点了图标不亮；所以「随」用自己的 builder，在里面直接读状态。

## Consequences

- 收益：三端行为一致，入口默认关，没打开的用户看不到任何变化；打开后键面、上屏、统计和读屏始终是同一个数字。
- 代价：鸿蒙每个数字上屏后网格的九个键随键面重建（ForEach 的键里有数字）；iOS 只换标题。网格每一行的 ForEach 键必须含整行三个键：原来只用首键，重排后新的一行与旧的一行首键相同时 ArkUI 会留着旧行和它里面的旧键，网格出现重复的数字、缺掉另一些（只看首键时多数重排都会撞上）。标准键序下各行的键不变，关着「随」时什么都不变；`run.sh` 钉住这个键的写法。鸿蒙给 `harmony_toolbar` 加了字段，降级到旧版本后再保存设置会丢掉这一项。
- 和 Android 一样，这项功能不承诺抵御录屏等攻击，只用于用户主动选择的输入体验；密码框、只收数字的输入框没有工具栏，也就没有随机。

## Verification

- `rbuild cargo test -p msime-client-core --lib preferences`：`harmony_toolbar` 的默认值、只开「随」的部分文档、序列化往返和已发布版本文档的读写。
- `bash platforms/harmony/tests/run.sh`：`random-digits.test.ts` 用固定随机源跑 1000 轮，覆盖排列完整、每次输入换序、离页不取随机数、返回沿用、关闭恢复、恒等随机排列仍换序和 `NineKeyLayout.digitsInOrder`；`kb-view.test.ts` 覆盖 `rowHidden` 的方案参数；`run.sh` 钉住视图和会话的接线。
- `apps/desktop` 的 vitest：设置页的开关、说明、默认值、置灰和整行说明，包括非中文方案下只开「随机数字」时仍显示「工具栏不显示」。
- iOS `RandomDigitsTests`：策略同上；键盘上设置只放出入口、点「随」不跳页、键面打乱且按键面上屏、每次换序、离页返回沿用、关闭恢复、英文无入口、隐藏工具栏清掉状态、只开「随」时这一行留着；临时模式（U 模式）里数字页是标准键序、输入数字不换序，退出模式后回到进入前的随机排列（对应 Android `RandomDigitsDeviceChecks` 的 U 模式检查；iOS 从工具菜单打开的模式退出时回到字母页，Android 留在数字页，这是两端原有的差别，测试重新打开数字页再核对）。`TouchToolbarPreferenceTests` 和 `KeyLayoutStyleTests` 覆盖新本机键的默认值和预览与键盘一致。
- 鸿蒙 `hvigorw assembleHap` 编过 ArkTS（原生库借用主检出里已暂存的 arm64 产物，只为编译，不是这个分支的原生构建），本篇改动的文件没有 ArkTS 告警。
- 没有在鸿蒙设备或模拟器上跑过，也没有在 iPhone 真机上验收：设置开关只放出「随」、点「随」高亮不跳页、键面打乱且上屏一致、读屏、回到拼音页再进来沿用、英文无入口这几条，两端都还要在设备上看一遍。

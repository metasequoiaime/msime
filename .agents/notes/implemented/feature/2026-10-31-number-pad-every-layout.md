# Agent Note: 移动端所有方案和键面的 123 都打开数字九宫格

Status: implemented

## Problem

[移动端数字入口默认使用九宫格](2026-10-15-mobile-default-number-pad.md)只让中文方案的常规数字入口借用数字九宫格：英文、本地模式、日文（罗马字和假名）、韩文、越南文、藏文、注音（iOS、HarmonyOS 的全部注音和 Android 的大千）以及 iOS 写半角标点的方案仍进横排 1–0 的 123 层；[Android 长空格布局](2026-10-27-android-long-space-layout.md)又让中文 26 键的长空格样式进带空格的 123 层，iOS 和 HarmonyOS 随后跟进。同一个 123 键在不同方案下进两种完全不同的面板，切到英文或外语后数字键位全变。维护者在 2026-10-31 决定：不管什么输入方式、什么布局，Android、iOS、HarmonyOS 按 123 都进数字九宫格。

## Decision

三端的数字入口在每种方案、每种触屏键面上都打开同一块数字九宫格：英文（含 Android 独立英文）、本地模式、中文 26 键的长空格样式、日文罗马字与假名网格、韩文、越南文、藏文、注音九键与大千、笔画、手写、14/18 键和 26 键一族，手机和平板（含 iPad 全尺寸键盘）一样。判定只剩「在符号层、不在 #+= 扩展页」：Android `KeyboardLayout.drawsNumberPad(Layer)`，iOS `KeyboardViewController.drawsNumberPad(symbols:moreSymbols:)`，HarmonyOS `NineKeyLayout.opensNumberPad(symbols, more, touch)`。九宫格只换显示，不改 Engine 里的方案和组码布局；数字、0、小数点和四则运算按字面上屏，只有本地模式、越南文 VNI、藏文组字这类由 Engine 把数字或符号当拼写的场合交给会话，大千的 1–0 本身是注音键，不走拼写这条路。

九宫格的标点跟着当前语言：中文标点生效时用中文写法；日文把「，」换成「、」，弯引号换成「」『』；英文、本地模式、韩文、越南文、藏文和中文半角标点下用半角写法。右列结束标点同理，中文和日文是「！」，其余是「!」。半角语境下左列在默认项之后按 `@ : ; ( ) ' " $` 的顺序追加原 123 页上的常用半角符号，已有的跳过；用户自定义过左列时保留用户的表，只做同样的写法转换，不追加。邮箱框里的 `@` 由此仍在键盘层上。藏文左列的 `'`（achung，如 `'od`）交给组字，和原 123 页一致，不先上屏组字。

#+= 扩展页仍是完整符号页，从那里按 123 回到数字九宫格。注音例外：大千的 `，` `.` `/` `;` `-` 被注音键占用，注音的 #+= 三端都保留原来每排十键的共享符号行（有 `；：/ - ( ) @`），而不是通用 #+= 页，按下的字符仍按字面上屏，不会被当作注音键。

九宫格的返回键印借出它的键面：英文和其他 26 键语言印「ABC」，韩文「한」，假名「あいう」，注音「注」，14/18 键和笔画印各自短名，原拼音九键仍印「九键」。中文 26 键一族和手写沿用各端原有的字样（Android「返回」，iOS 和 HarmonyOS「拼音」），这处三端差异早于本篇，不在本次改动范围内。#+= 页的返回键字样保持原样。

HarmonyOS 2in1 的屏幕键盘不变：它没有 #+= 层，123 进的是唯一一页共享符号行，换成九宫格会让这一页的括号和标点再也到不了。

## Historical audit

- [移动端数字入口默认使用九宫格](2026-10-15-mobile-default-number-pad.md)：部分取代。「日文、韩文、越南文、藏文、大千注音、英文和本地工具保留各自符号契约」以及据此做的语言判定被本篇推翻；数字字面上屏、`commitRaw` 组合提交、`Digit*`/`Nine*` 统计区分、返回键统一和 2in1 保留仍然成立。
- [Android 长空格布局](2026-10-27-android-long-space-layout.md)：部分取代。「26 键的普通数字入口使用带空格的 123 层，不借用九宫格」被本篇推翻，长空格用户在九宫格上仍有空格键（右列或底行，跟 `touch_number_pad_space`）；长空格的字母层底行和其余约定不变。

## Alternatives considered

- 只把英文接进九宫格，独立语言保留各自的 123 页：英文是最常见的第二语言，改动面小；但维护者明确要求所有方案一致，留下的例外仍会让同一个 123 键因方案不同而变成两种面板。
- 半角语境不追加符号，原 123 页上的 `; : ( ) $ @ " '` 只从「符」面板输入：与中文现状一致、九宫格左列最干净；但邮箱框里会失去键盘层上的 `@`，英文用户常用的括号和冒号也多一步，维护者选择在左列追加。
- 藏文作为唯一例外保留原 123 页：能原样保住 achung 的组字路径；但又回到「方案不同、123 不同」，改为在藏文左列放 `'` 并交给组字。
- 注音的 #+= 改用通用 #+= 页：与其他方案一致；但大千占用的标点在通用页上没有，注音用户会丢掉常用标点，维护者选择三端都保留共享符号行。

## Consequences

收益是 123 在任何方案下都是同一块九宫格，数字键位不再随方案变化，三端行为一致。代价：原来横排 123 页上的一部分符号要靠左列滚动或「符」面板；假名网格自己的数字层（☆♪→ 等弹出符号）和英文 123 页在工具栏没有表情入口时的表情后备不再出现；Android 中文关掉中文标点时，九宫格左列和右列显示半角写法。设计稿 123 层的代码（Android `KeyboardLayout.numberLayer`、iOS 大千共享行以外的 123 页、HarmonyOS `NumberSymbolLayout.numberLayer` 与假名网格的 `digitKeys`）在触屏上已经走不到，按仓库规则保留未删。只提供日文方案的版本有数字九宫格，但「按键布局」里的「数字键盘」组仍只在提供中文样式组时出现。

九宫格上的数字、0、小数点、出厂左列项（含半角追加项）和补充行按同一条规则决定去向：只有本地模式、越南文和藏文里、Engine 此刻把这个字符列为拼写符号（组字中或本地模式下的 `spelling_symbols`）时交给会话，藏文的 `'` 任何时候都交给会话；其余一律先原样结束组字再按字面上屏，包括中文、英文、日文、韩文和大千。用户自定义的左列项一律按字面上屏，藏文里也是。三端的判定分别是 Android `NumberPadPunctuation.typesThroughSession`、iOS `typeNumberPadText`、HarmonyOS `NineKeyLayout.numberPadToSession`。

## Verification

- Android：合入 develop `fc552ad648` 后 `rbuild env ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh` 退出 0，317 项 JVM 冒烟通过（含 `NumberPadPunctuationSmoke`：半角追加的顺序与去重、自定义表不追加、U 模式数字与 VNI 声调数字和藏文组字中的 `+ - . /` 交给会话、大千和英文数字按字面、注音 #+= 用共享符号行），259 份设备测试源码编译通过。`NumberPadDeviceSmoke` 新增的英文、长空格、日文、假名、韩文、大千、藏文 `'od` 用例只编译、没有在模拟器上运行：编译机磁盘接近满，本机没有现成的原生库。
- iOS：本机模拟器（iPhone Duo / iOS 27.1，测完删除）`build-for-testing` 成功；`NineKeyKeyboardTests`、`KeyLayoutStyleTests`、`KoreanDubeolsikTests`、`KeyboardFormFactorTests`、`EmailSuffixKeyboardTests`、`KeyboardCursorSwipeTests`、`FourteenKeyKeyboardTests`、`EighteenKeyKeyboardTests`、`LanguageSchemesTests`、`NineKeySidebarSettingsTests`、`JapaneseNineKeyTests`、`HandwritingTests` 共 351 项，7 项跳过，唯一失败是 `HandwritingTests.testThePadSpansTheRowAndTheBottomRowCarriesTheTools`，同一用例在未改动的基线上同样失败，属于 iPhone Duo 运行时。藏文 `'od`、`dga'` 经数字九宫格组字通过。测试宿主没有注音词库，大千 #+= 的界面用例跳过，只由纯函数用例覆盖。
- HarmonyOS：合入 develop 后 `bash platforms/harmony/tests/run.sh` 零失败，`scripts/test-harmony-arkts-subset.py` 通过，`hvigorw assembleHap` 构建成功。
- 三端都没有在真机或系统键盘里验收：滚动左列的观感、邮箱框里点 `@`、本地 Unicode 模式和 VNI 在九宫格上的实际输入、大千共享符号行的排布都待设备验收。

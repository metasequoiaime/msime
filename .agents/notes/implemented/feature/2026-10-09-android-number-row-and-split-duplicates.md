# Agent Note: Android 数字行，分离式键盘右半边重复 G、V

Status: implemented

## Problem

#6022（基于 0.3.1）有两个请求。一是 26 键字母上方加一行可开关的数字行 1–0：现在打数字要点左下角进 123 层，那一层里又没有切换中英文标点的键，来回切层很麻烦。二是平板横屏的分离式键盘右半边也放 G 和 V：现在 G（第二行）和 V（第三行）只在左半边，左右不对称，用双拼时右手够不着这两个键。

Android 的字母层只有 `KeyboardLayout` 的三行字母，`ImeLetterRows.rebuildKeyRows` 没有插数字行的地方，本地设置里也没有对应开关。iOS 有数字行，但只在 iPad 上。分离式键盘的 `splitRow` 只在 `SplitKeyboardPolicy.cutIndex` 算出的键边界插一个空隙，不复制任何键。

## Decision

### 数字行

- 新增本地设置「键盘 › 布局 › 数字行」（`platform.android.number_row`，布尔，默认关，所有设备都有，不随账号同步）。同步字段表（`crates/client-core/src/account/settings_sync.rs` 的 `ANDROID_LOCAL_SETTINGS`）不加这个键，与 `NINE_KEY_SWIPE`、`GLIDE_TYPING` 一样先只在本机。
- 画不画由 `KeyboardLayout.drawsNumberRow(enabled, layer, touchLayout, windowHeightDp)` 决定：开关打开、在字母层、画的是标准 26 键一族（`STANDARD_TOUCH_LAYOUT`）或韩文两套式（`KOREAN_LAYOUT`），并且窗口可用高度（`Configuration.screenHeightDp`）不低于 480 dp（`NUMBER_ROW_MIN_WINDOW_HEIGHT_DP`）。手机横屏时窗口只有 330–430 dp，默认高度的键盘约 300 dp，再加一行 63 dp 就超过窗口，工具栏或底行被挤出去、应用也没有可见区域；`KeyboardGeometry.windowHeightAdjustment` 只按三行字母算，管不到这一行。所以手机横屏时不画，存着的设置不变，转回竖屏由 `numberRowStale` 重建出来；竖屏手机、平板横屏和折叠屏内屏都在 480 dp 以上。九键、日文假名网格、手写、笔画、注音（大千的数字本身是注音键，注音 9 键是网格）不画；123 / #+= 层不改，它的第一行本来就是数字。
- `ImeLetterRows.addNumberRow` 在字母行之前加一行 1–0（`KeyboardLayout.NUMBER_ROW`），20sp，键位 id 与字符相同，参与热力图。高度角色与第一行字母相同（`KeyboardHeightRole(KEY_ROW_HEIGHT_DP, 3, 0, true)`），键盘整体高出一行，键盘高度设置和行距照样作用在这一行上。分离式键盘画着时这一行同样从中间分开（12345 | 67890）。
- 数字键经 `type()` 交给 Engine，和字母键一样：没有组字时 Engine 不收，宿主上屏这个数字（全角模式下是全角）；英文直输时直接上屏。组字中当前页有对应候选的 1–9 由 runtime 选词（`runtime.rs` 的 `Action::Character` 分支，与 iPad 数字行「组字中 1–9 选候选」一致）；当前页之外的 1–9 被 runtime 当作已处理吞掉，什么也不发生；`0` 和没有候选时的 1–9 被拒。被拒的数字原来直接 `commitText` 进组字区，把正在组的拼音换成这个数字（nihao 再按 0 只剩 0，候选栏还挂着 nihao 的候选，再选「你好」得到「0你好」），现在和被拒的标点一样先按首选结束组合再上屏（`DeclinedKeyPolicy.finishesComposition` 把数字也算进去），得到「你好0」。
- 开关变化后，`render()` 里的 `ImeLetterRows.numberRowStale()` 发现键行与应画状态不一致就重建键行，和 `splitStale` 一样。`rebuildKeyRows` 一开始把 `builtNumberRow` 复位，九键、手写、123 层这些提前返回的分支不会留下旧值，否则每次渲染都会判成过期而重建。

### 分离式键盘重复 G、V

- 字母层的第二、三行在右半边内侧重复左半边最内侧的那个字母（`SplitKeyboardPolicy.duplicatesInnerKey`）：asdfg | ghjkl、⇧zxcv | vbnm⌫；韩文两套式同一位置是 ㅎ、ㅍ。总是生效，没有子开关。第一行 qwert | yuiop 本来左右各五个，不重复；数字行和 123 / #+= 层也不重复。
- 重复的键从空隙里占一个键宽：`SplitKeyboardPolicy.gapWeight(rowWeight, duplicateWeight)` 从原空隙份额里扣掉它，整行总份额不变，所以三行键宽一致；这两行的两段恰好等宽（第二行 0.5 + 5 对 5 + 0.5，第三行 ⇧ + 4 对 4 + ⌫，⇧、⌫ 都是 1.4），空隙仍居中，只是比第一行窄一个键。只在重复之后两段正好相等时才重复（`duplicatesInnerKey` 按这一行的实际份额判断）：微软双拼显示第十个键 `;` 时第二行没有半键缩进，asdfg | hjkl; 本来五对五，再重复 G 就成了五对六、空隙偏开半个键，所以这时第二行不重复 G，第三行照样重复 V。
- 重复的键和左边那个用同一段代码建（`rebuildKeyRows` 里的 `characterKey`，原来的循环体原样挪进去）：输入、键面、提示角标、滑动输入符号、长按、键位 id 和无障碍描述都一样，也进 `shuangpinKeyButtons`，双拼提示和大小写键面随之刷新。
- 滑行输入只认左边的 G、V：交给 Engine 的滑行请求每个字母只有一个键位中心（`GlideTypingPolicy.request` 的 `keys` 是 26 个点），`ImeGlideTyping.measureKeys` 只量每个字母的第一个键，右边的重复键在滑行里和空隙一样不算字母键，点按照常输入。

## Alternatives considered

- **数字行总高度不变，把三行字母压矮** — 报告人说的「改善主键区长宽比」也能做到，键盘不会变高；但每个字母键都变矮，误触变多，而打开数字行的人要的是多一行键，不是更挤的字母。产品决定键盘整体加高一行，与 iPad 的做法相同。
- **数字行比字母行矮一些（比如功能行的 46 dp）** — 键盘只高出一点；但现有的高度角色只有「分摊键盘高度调整」和「固定高」两种，矮一号又要跟着键盘高度设置缩放，得另立一种规则；先和字母行同高，与 iPad 一致。
- **数字行只在平板上提供（与 iOS 一样）** — 和 iOS 对齐；但手机上来回切 123 层同样麻烦，报告人用的就是手机，开关默认关，不会改变不需要它的人的布局。
- **数字行随账号同步** — 换设备不用重开；但要同时改 Rust 的 `ANDROID_LOCAL_SETTINGS`、数组长度和服务端同步字段表，老客户端和服务端会拒收新键；按 `NINE_KEY_SWIPE` 的先例先只在本机。
- **123 层加一个中 / 英文标点切换键** — 直接回应「在数字界面里改不了中英文标点」；但产品决定这次不改 123 层，有了数字行，打数字不必再进 123 层。
- **重复 G、V 加一个子开关** — 不想要的人可以关；但重复的键只占原来空隙的一个键宽，不影响别的键，多一个开关的成本高于收益。
- **重复的键另外加宽、空隙保持 25%** — 第一行和下面两行的空隙一样宽；但那样第二、三行的键会比第一行窄，键宽在行之间不一致，打字时手感更差。
- **组字中数字一律不选词，先结束组合再上屏数字（和 123 层一样）** — 触屏的候选 chip 没有序号，点 2 选第二个候选不直观；但 iPad 数字行组字中 1–9 选候选，Engine 和 runtime 对数字的处理（v 模式、Unicode 输入这类以数字为输入的本地模式）也要先经过 Engine，宿主拦在前面会把这些一起截掉。保留 runtime 的选词，只修被拒数字的去处。
- **微软双拼时把 G、H 都跨断口重复，两边各六个** — 对称；但右半边第二行多出 H 之后比第一行宽一个键，键宽在行之间不一致，而且这种情况只在显示 `;` 的微软双拼出现。
- **让滑行也认右边的 G、V** — 两边都能滑；但 Engine 的滑行请求每个字母只收一个中心点，要么改共享的滑行契约和 Engine，要么在宿主里把经过右边重复键的轨迹点搬到左边，轨迹就不是手指真实走过的形状了。

## Consequences

- **收益**：打开数字行后 26 键上不用切层就能打数字；分离式键盘两边拇指都够得着 G、V，左右对称。
- **代价**：打开数字行后键盘高出一行；手机横屏时没有数字行；组字中按当前页之外的 1–9 什么也不发生；切到 123 层、九键、手写等没有数字行的键面时，键盘高度会跟着变，不再是「各布局之间总高度不变」。分离时第二、三行的空隙比第一行窄一个键。滑行输入时右边的 G、V 不算字母键。
- **未覆盖**：本地 arm64 API 35 手机模拟器上用 adb 截图看过第一版：设置页有「数字行」开关，打开后 qwerty 上方出现 1–0、键盘高一行，空闲时点 5 上屏 5，组字 ni 时点 2 选中第二个候选「尼」；用 `wm density 240` 加旋转模拟平板横屏、打开分离后是 12345|67890、asdfg|ghjkl、⇧zxcv|vbnm⌫，点右边的 G、V 上屏 g、v。review 之后的三处修改（被拒数字先结束组合、手机横屏不画数字行、微软双拼不重复 G）没有在设备上看过。真平板、韩文两套式和微软双拼的分离布局、滑行经过右边的 G、V、浮动键盘加数字行、TalkBack 下重复键的朗读，都没有在设备上确认。

## Verification

- `ANDROID_SDK_ROOT=… bash platforms/android/check-host.sh`：`KeyboardLayoutSmoke`（数字行内容、只在 26 键和韩文的字母层画、关着时不画、手机横屏的窗口高度不画、窗口高度未知时照画）、`SplitKeyboardPolicySmoke`（哪几行重复、扣掉一个键宽后的空隙份额与整行总份额、第二三行两段等宽、重复的是字母不是缩进或 ⇧、微软双拼十键行和韩文九键行）、`DeclinedKeyPolicySmoke`（组字中被拒的数字先结束组合，字母不算）、`AndroidLocalSettingsSmoke`（数字行默认关、不同步）、`SettingsSearchIndexSmoke`（「数字行」进了键盘页的搜索关键词）。
- Gradle `compileFullReleaseJavaWithJavac` 编译 `ImeLetterRows`、`KeyboardOptionsPage` 的改动。

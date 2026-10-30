# Agent Note: 大屏单手模式空出来的一侧放数字键盘，iPad 补上单手模式

Status: implemented

## Problem

#10203（拆自用户反馈 #10202）：平板上想要「左边数字键盘、右边九键」的单手布局，双手握着平板时右手拇指打九键，左手拇指打数字。

[大屏上的单手模式缩到手机大小并贴到屏幕边](2026-10-29-tablet-one-handed-size.md)（#10204）之后，Android 和鸿蒙在大屏上开单手模式时，「侧栏 + 键区」这一块是手机大小（Android 424dp，鸿蒙 431vp），贴到一侧，另一侧是一大块键盘底色，没有按键。那篇笔记把这个布局单独留给了 #10203。

iOS 的单手模式原来只在 iPhone 上画（`KeyboardOneHandLayout.effective` 对 `.tablet` 一律返回 `.off`，功能面板的「单手模式」磁贴在 iPad 全宽键盘上置灰），iPad 上根本没有可以放侧边面板的单手模式。

维护者的决定：

- 做成大屏单手模式的一个变体，用 #10204 空出来的那一侧放侧边面板，不另开一种平板布局；分离式键盘和浮动键盘不变。
- 侧边面板第一版只放数字键盘；文本编辑面板作为另一种侧边面板，不在第一版里。
- 只在主键区是九键时显示。
- 三端都做；iPad 为此补上单手模式，让同一个布局在 iPad 上也成立。

## Decision

### 共享偏好与同步

`crates/client-core/src/preferences.rs` 有 `touch_one_handed_side_panel: TouchOneHandedSidePanel`：

- 取值 `off`（默认，空着，即 #10204 之后的样子）和 `number_pad`。等于默认值时不写进文档（`skip_serializing_if`），默认文档与加字段之前逐字节相同，老用户什么都看不到变化。
- 不认识的字符串原样留着：手写 `Serialize` / `Deserialize`，不认识的写法读成 `TouchOneHandedSidePanel::Unknown(String)`，存文档、账号同步导出时照原样写回（`as_str` / `from_wire`）；不是字符串照样报错。三端宿主读到不认识的值（包括以后的 `text_edit`）和不是字符串的值都按 `off` 画（Android `OneHandSidePadPolicy.parse`，iOS `TouchOneHandedSidePanel(in:)`，鸿蒙 `OneHandedPolicy.sidePanel`）。这里不照 `TouchNumberPadSpace` 那样把不认识的值读成默认：那样旧版本下一次保存时会把较新版本写下的值删掉（`off` 不写进文档），账号同步里还会把它当成本机改动（本机导出 `off`，基线是云端的 `text_edit`），拿 `off` 覆盖云端较新设备的选择，再扩散到所有设备。`touch_number_pad_space` 等其他宽松读取的触屏枚举有同样的缺口，不在这次范围里。
- 做成取值而不是开关：以后加文本编辑面板时只多一个取值 `text_edit`，今天的版本读到它按 `off` 画；要是今天做成布尔，以后改成字符串时还没升级的版本会把整份偏好文档判成类型错误。
- 账号同步走触屏宿主共用的 `TouchHostKeys`，与 `touch_number_pad_space` 同一条路：Android `platform.android.one_handed_side_panel`，鸿蒙 `platform.harmony.one_handed_side_panel`，字符串。导出总是写（`off` 也写），云端才能被改回空着；应用时不认识的面板也照原样收下，下一次导出写回同一个值，与这次记下的基线相同，不会被当成本机改动。服务端字段表收录这两个键之前，上传前的字段表过滤会去掉它们，这一步在仓库外（后端字段表）。iOS 的同步走 `ios_settings.rs` 的自有映射，与其他按键布局偏好一样不同步这一项。
- 共享 TS 模型 `packages/ui/src/core/preferences-model.ts` 有 `touch_one_handed_side_panel?: "off" | "number_pad"`；`platforms/macos/tests/settings/preference_coverage.py` 把它列为 macOS 不适用。

单手模式本身仍是各端本机的设置（Android `platform.android.one_handed`，iOS App Group `keyboard.oneHanded`，鸿蒙按键反馈文件的 `oneHanded`），这一项只决定单手模式生效时空出来的那一侧放什么。

### 什么时候显示

全部满足时才显示侧边数字键盘，任何一条不满足时那一侧照旧空着：

1. `touch_one_handed_side_panel` 是 `number_pad`。
2. 单手模式此刻实际生效：存着的模式是 `left` / `right`，没有画分离式键盘，没有浮动（Android `ImeFrame.effectiveOneHanded`；鸿蒙 `KeyboardView.oneHandActive`，即 `OneHandedPolicy.active` 再加上没有浮动；iOS `KeyboardOneHandLayout.effective`）。
3. 大屏，并且空出来的宽度放得下（见下一节的侧边宽度大于 0）。手机上 #10204 的块宽是 0，这一条自然不成立。
4. 主键区此刻画的是九键一族的字母层：拼音九键、九键双拼、笔画，与设置页「按键布局 › 9 键」那一组是同一族（iOS `TouchKeyLayoutOffering.nineKeySchemes`）。英文、本地模式（这时画 26 键）、123 数字层（主键区已经是数字九宫格）、#+= 层、注音、日文假名九键、手写、14/18 键和 26 键都不显示。
5. 按键没有被面板盖住：符号、表情、剪贴板、展开候选等面板打开时侧边面板和按键一起隐藏。Android 和鸿蒙靠结构保证（面板与按键在同一行，整行收起或不画），所以 Android `OneHandSidePadPolicy.shown` 与鸿蒙 `OneHandedPolicy.sidePadShown` 只汇总前四条；iOS 的面板只遮住按键列，侧边面板另外加进 `setKeyRowsCovered`。九键长按选项（鸿蒙 `SURFACE_NINE_KEY_HOLD`）和键盘高度预览（`SURFACE_GEOMETRY`）画在同一行里，侧边面板照常显示。

### 尺寸与摆放

侧边面板贴在空出来那一侧的屏幕边上，不贴着九键：用户是两只手的拇指各管一边。它和侧栏之间剩下的地方是一块不接触摸的空白（键盘底色）。排列次序三端相同：键在右时 `[面板, 空白, 侧栏, 键区]`，键在左时 `[键区, 侧栏, 空白, 面板]`。

宽度规则三端相同：空出来的宽度 `side` 不到 150 时不显示，否则取 `min(side, 300)`，单位 Android dp、鸿蒙 vp、iOS pt。上限 300 让三列键每列约 100，和九键数字层的键差不多大；空出来的地方再宽（平板横屏有八九百），多出来的都是空白。下限 150 是三列键的最小可按宽度。`side` 按各端这一行实际可用的宽度算，所以几个数略有出入：

- Android：`side = screenWidthDp - 424`（`KeyboardFormFactorPolicy.oneHandedSidePadWidthDp`；块宽 424 已含侧栏，行里没有另外的间距，键盘表面在大屏单手时铺满窗口）。竖屏 800dp 时面板 300；最小的 600dp 平板 side 176，面板 176，空白为 0；横屏 1280dp 面板 300，中间空 556。
- 鸿蒙：`side = (屏幕宽 - 2 × 4) - 431 - 6`（`OneHandedPolicy.sidePadWidthVp`）。减 8 是键盘根的左右内边距 `KeyboardMetrics.TOUCH_ROOT_HORIZONTAL_PADDING_VP`，不减的话 600vp 的设备会把这一行撑出屏幕；减 6 是面板与块之间的 `Blank(RAIL_GAP_VP)` 的最小宽度。600vp 设备上面板 155。
- iOS：`side = (键盘宽 - 8 - 8) - 431 - 6`（`KeyboardOneHandLayout.sidePadWidth`），减 16 是 iPad 的左右内边距（`KeyboardFormFactor.padding`），块宽 `min(可用宽, round((360 + 6) / 0.85)) = 431` 与鸿蒙同一个公式，减 6 是空白与侧栏之间 stack view 的间距（面板和空白之间不留间距）。iPad mini 竖屏 744pt 时面板 291。

键面：3 列 × 4 行，与电话拨号盘相同，不随别的设置变：

```
1 2 3
4 5 6
7 8 9
. 0 ⌫
```

数字的排列固定是 1 2 3 在上，与三端九键数字层相同（[撤掉九宫格数字顺序](../simplification/2026-10-11-remove-number-keypad-settings.md)之后已经固定）；`touch_number_pad_space` 不作用于它（它没有空格）。前三行与主键区九键网格同高，第四行与底行同高，整块面板和主键区一样高。对齐方式各端按自己的布局定：Android 面板里的键块贴行的底边（`Gravity.BOTTOM`），鸿蒙这一行在面板显示时按底边对齐，这样九键网格上面另有一行时面板也与网格和底行对齐；iOS 用约束把面板三行的高度绑到 `nineKeyContainer`、最后一行绑到 `actionRow`。键距、行距用用户的 `touch_key_spacing_tenths` / `touch_row_spacing_tenths`，圆角、颜色、按下效果、按键音和振动都跟主键区的键同一个工厂。

### 按键行为

- 数字和 `.`：走各端数字层已有的字面上屏路径（Android `ImeLayoutRows.commitNineKeyLiteral`，iOS `insertNumberPadLiteral`，鸿蒙 `KeyboardView.insertNumberPadLiteral`）：正在组的拼音先按原样上屏，再上屏这个字符；全角开关照常作用。字号与九键数字层的数字相同。
- ⌫：与主键区九键右列的 ⌫ 同一个动作和连删（组字时删组字的最后一码，没组字时删光标前一个字），图标同一个。
- 统计：数字记普通数字键（Android `KeyPressIds.forCharacter`，iOS `TypingKeyID.character`，鸿蒙 `KeyIdPolicy.character`），`.` 记 `Period`，⌫ 记 `Backspace`。热力图不画侧边面板，这些 id 本来就有。
- 不挂滑行输入、横滑移动光标、长按选项和上滑角标。

### 命中与触点几何

- 侧边面板是一个独立的按键区：落在面板里键与键之间空隙的触摸交给最近的键（Android 一个新的 `KeyboardKeyArea`，iOS 面板的行留在 `KeyAreaStackView` 之下、按 `KeyGapRouting` 的规则归属，鸿蒙用与主键区相同的 `fixedKey`）。面板与侧栏之间的空白不接触摸，也不把触摸交给任何键：Android 是 `OneHandRow` 里一个普通的 `View`；iOS 是一个普通 `UIView`（不是 stack view，`KeyAreaStackView.hitTest` 不会为它找键）；鸿蒙是 `Blank()`，不挂 `onClick`。
- 双拼触点的键位几何（#10135）只量 26 键字母和微软双拼的 `;`，侧边面板只在九键一族时出现，不进几何、不影响注册。iPad 新增的单手模式让 26 键字母变窄、贴边，iOS 的 `currentKeyLayout` 本来就按实际矩形（`KeyAreaStackView.layoutFrame`）量，换代次重新注册，没有改 `KeyboardViewController`，只补了 `TouchPointTests` 的用例。

### 无障碍

- 数字键读「数字 1」…「数字 0」，与三端数字层的数字键相同（Android 经 `keyboardKey` 读作「按键 数字 1」）；`.` 读「小数点」；⌫ 读「删除」。
- 面板与侧栏之间的空白不是无障碍元素。
- 测试用标识：iOS 面板 `accessibilityIdentifier = "oneHandSidePad"`，键 `sidePadDigit0`…`sidePadDigit9`、`sidePadDecimal`、`sidePadDelete`，空白 `oneHandSideSpacer`；Android 面板 `setTag(ImeSidePad.TAG)`（`"oneHandSidePad"`）；鸿蒙面板 `.id('oneHandSidePad')`。

### 设置页

三端文字逐字相同，各端测试钉住：

- 标题：「大屏单手时侧边显示数字键盘」
- 说明：「单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。」

开关打开写 `number_pad`，关掉写 `off`（client-core 存成缺键）。这一项只在大屏上列出：

- Android：`KeyboardOptionsPage` 第一个 `if (showLargeScreenOptions)` 块里「分离中间空隙」之后，文字直接写在页里（保持现有的设置搜索索引测试能读到），`check-host.sh` 逐字钉住；`PageId` 的设置搜索关键词有「大屏单手时侧边显示数字键盘」「侧边数字键盘」「单手」。
- iOS：`KeyboardLayoutSettingsView.tabletGroup`（只在 iPad 上出现）里「横屏分离式键盘」之后，`accessibilityIdentifier("appOneHandSidePadSwitch")`，经 `TouchOneHandedSidePanelPreference.save(_:)` 用 `MetasequoiaInputSessionBridge.updateSharedPreferences` 写共享文档。这一组的说明加了一段 iPad 单手模式：「功能面板的单手模式在 iPad 全宽键盘上把按键缩到手机大小、贴到一侧；横屏分离式键盘画着时单手模式让位。」「恢复默认」不动它（它不是这一页的间距或高度）。
- 鸿蒙：共享 React 设置页 `TouchKeyboardGeometrySection` 的可选属性 `oneHandedSidePanel?: boolean` 和 `onOneHandedSidePanelChange`，`screen-keyboard-page.tsx` 只在宿主报告 `touch_one_handed_side_panel` 时传入。手机排法（`layoutRows` 给出时）放进开头的「布局」组；手机排法的其他行不画说明，这一行破例带说明：它只在平板和可折叠手机上出现，不在窄屏上挤，没有说明的话打开后在 26 键或双手模式下看不到任何变化。鸿蒙平板走的也是手机排法。其他排法放在「布局」组的「横屏分离式键盘」之后、带说明。能力位在 `packages/ui/src/core/host-capabilities.ts`（缺省按不提供处理），鸿蒙 `Settings.ets` 按设备形态报告：`SettingsFormFactorCapabilities.resolve(deviceType, foldable)` 的 `oneHandedSidePanel` 在 `tablet` 为真、`phone` 且 `display.isFoldable()` 为真、直板手机和 `2in1` 为假；`isFoldable` 调用出错时按不可折叠并记日志。可折叠手机在折叠状态（外屏）下也列出这一项，与 Android 不同：Android 的 `KeyboardOptionsPage` 按打开设置页那一刻的 `smallestScreenWidthDp >= 600` 列出大屏一组，折叠时看不到，展开后才出现。这是有意的出入：这一项是偏好，只在展开后的大屏上生效，鸿蒙的能力位在设置桥里按设备形态算一次，按当前屏幕算的话折叠时打开设置页就找不到它，说明里也写着「折叠屏展开」时生效；Android 的大屏一组是整组按当前窗口列出的，单为这一项改成按设备能不能折叠要另找判断（`FEATURE_SENSOR_HINGE_ANGLE` 只在 API 30 以上有），不值得。

## 各端实现

### Android

- `KeyboardFormFactorPolicy`：`ONE_HANDED_SIDE_PAD_MAX_WIDTH_DP = 300`、`ONE_HANDED_SIDE_PAD_MIN_WIDTH_DP = 150`、`oneHandedSidePadWidthDp(smallestWidthDp, screenWidthDp)`。
- 纯逻辑 `keyboard/OneHandSidePadPolicy.java`：`PREFERENCE_KEY`、`Panel parse(String)`、`shown(panel, effectiveMode, widthDp, drawnLayout, layer, numberPadDrawn)`；「九键一族」是 `QUANPIN_NINE_KEY_LAYOUT`（九键双拼同一个布局 id）和 `STROKE_LAYOUT`、`LETTERS` 层、没画数字九宫格。键面按键行建出来时的布局（`ImeLetterRows.drawnTouchLayout`）算，不按还没重建的会话快照。
- `MSIMEInputService` 读进 `oneHandedSidePanel`，它进了 `touchGeometryKey`（变了就重画）和 `SKIN_HINT_KEYS`（冷启动第一帧就按它画）。
- `core/ImeSidePad.java` 建面板：一个 `KeyboardKeyArea`，上面三行一块（`KeyboardHeightRole.nineKeyGrid()`），下面一行（`bottomRow()`），键按 `addNineKey` 排；`.` 复用 `ImeLayoutRows.nineKeyDecimalKey`（为此改为包内可见），⌫ 用 `backspaceKey` + `bindBackspaceRepeat`。面板建好后一直留着，直到键盘视图本身重建（`ImeFrame.column()` 里 `forget()`）；键距、行距和高度变化时 `ImeStyler.applyKeyboardGeometry` 经新的 `applySidePadGeometry` 一起重排，配色跟着整树上色，每次摆上时再补一遍当前皮肤。没有照原先的设想在键行重建时作废面板：间距和高度的变化不一定重建键行，作废的做法反而可能留下旧的几何。
- `ImeFrame.applyOneHanded`：在 #10204 的块宽之外再算面板宽度和 `shown`，排法签名带着面板宽度，变了才重排；显示时 `OneHandRow` 按上面的次序排，空白 weight 1。面板在 `row` 里，`setKeysCollapsed` / `setKeysCovered` 让整行收起或隐藏，面板跟着走。`applyOneHanded` 每次渲染都会被调到，切 123、切中英、切方案都经过渲染。

### iOS

- `KeyboardOneHandLayout`：`effective(_:formFactor:split:)` 只在 iPad 画横屏分离式键盘时返回 `.off`；`tabletKeysWidth = 360`、`blockWidth`、`sidePadWidth`，`keysWidth` 多了 `formFactor` 参数。
- `KeyboardViewController`：`oneHandRow` 里有空白 `oneHandSideSpacer` 和侧边面板 `oneHandSidePad`；iPad 单手时侧栏和按键列是常量宽度，空白吃掉剩下的宽度。两条宽度约束都是 required：字母行里 iPad 才画的键（Tab、iPad 的 ⌫ 和回车、行内 ，。、第二个 ⇧）隐藏时，它们按字母键宽度定的 999 约束总被零宽约束打破，字母越窄违反得越少，按键列宽度若同为 999 就被布局引擎压到约 164pt，空白跟着变宽（第一版就是这样，CI 上 `OneHandModeTests` 的块位置和空白宽度全偏 196pt）。窗口窄到放不下块和间距时由没有宽度约束的空白让出，不冲突。`applyOneHanded` 用 `OneHandArrangement`（模式、块宽、面板宽）判断要不要重排，也在 `viewDidLayoutSubviews` 里跑，转屏和窗口尺寸变化会重算；分离与否在 `updateKeyboardLayout` 开头先定。显示条件 `oneHandSidePadWanted` 是 `numberPad && isChineseMode && !isInLocalMode && !showsSymbols && nineKeySchemes.contains(inputScheme)`（iOS 的数字九宫格只在 `showsSymbols` 时画）。侧边面板不参与横滑移动光标。
- iPad 单手时键面用手机的：`keyFaceFormFactor` 在 `formFactor == .tablet && appliedOneHanded != .off` 时为 `.phone`。读它的是字母字号、中间行缩进、`canShowFullKeys`（Tab）、行内标点、手机/平板底行、`applyTabletLetterKeys`、`chineseTwentySixKeyboard`、`symbolLayerSymbolFirst` 和 `layoutInputs`；键盘高度、上下内边距、`topRowGap`、按键高度仍读 `formFactor`。字号在按键渲染配置时才由变换器读 `keyFaceFormFactor`，单手开关一变就给字母键重新装变换器（`applyLetterFont`）并请求配置更新；把同一份配置原样赋回去不算变化，按键不重画，磁贴打开单手后字母会停在 iPad 的 20pt。「数字行与 Tab 键」的数字行（和数字符号页的补充行）按 `formFactor` 取（`fullKeyNumberRow`）：iPad 单手时 Tab 不画，1 到 0 这一排照画，排数不变，单手开关不改键盘高度。
- 升级前存下的单手值不带到全宽键盘上：`keyboard.oneHanded` 以前只在手机形态（iPhone、iPad 浮动键盘和窄窗口）上画。`KeyboardLayoutPreference.oneHanded` 的写入顺带打上 `keyboard.oneHandedDockedTablet` 标记；`applyOneHanded` 在全宽键盘（不分离）上读到非关闭的值而没有标记时，经 `adoptOneHandedOnDockedTablet` 把它改成关闭并打上标记，只做一次。浮动键盘照旧用存下的值。
- 功能面板的「单手模式」磁贴在 iPad 上不再置灰。
- 偏好：`SharedUI/preferences/TouchOneHandedSidePanelPreference.swift` 的 `TouchOneHandedSidePanel(in:)`、`TouchOneHandedSidePanelPreference.save` / `load` 和设置页文字。

### 鸿蒙

- `OneHandedPolicy.ts`：`OneHandedSidePanel` 枚举、`SIDE_PAD_MAX_WIDTH_VP` / `SIDE_PAD_MIN_WIDTH_VP`、`sidePadWidthVp(contentWidthVp, largeScreen)`、`sidePanel`、`sidePadShown`（前四条）、`sidePadDigitRows` 和两个读屏名称。
- `KeyboardSession.ets` 在读 `touch_number_pad_space` 的同一处读 `touch_one_handed_side_panel`。
- `KeyboardView.ets`：`readOrientation` 里算 `oneHandSidePadVp`，`applySettings` 重读偏好；`handedKeys` 按上面的次序排；`@Builder oneHandSidePad()` 的前三行高 `gridHeight()`、第四行高 `rowHeight(3)`，数字和 `.` 用 `fixedKey`，⌫ 用 `deleteKey`。「九键一族字母层」是触屏、不在符号层、不是英文、不是假名、不在本地模式、没借数字九宫格，并且是九键网格或笔画。
- 笔画键盘的网格只有两行，面板的前三行平分网格高度，没法和笔画网格逐行对齐；拼音九键和九键双拼是三行，逐行对齐。

## Alternatives considered

- **做成布尔开关 `touch_one_handed_side_number_pad`**：第一版的设置页就是一个开关，布尔最直接，读的人一眼就懂。不用它：维护者已经说了文本编辑面板会是另一种侧边面板，到时要么再加一个互斥的布尔（两个都开时画什么要另定规则），要么把布尔改成字符串，而还没升级的版本读到字符串会把整份偏好文档判成类型错误。现在做成取值，代价只是多一个枚举。
- **存成各端本机设置，跟单手模式本身放在一起**（Android `platform.android.*` 本机键、iOS App Group、鸿蒙按键反馈文件）：单手模式就是本机设置，侧边面板也只在大屏上有意义，放在一起最顺。不用它：鸿蒙的设置页是共享 React 页，只会写共享偏好，存成本机设置要另开一条桥；按键布局类的选项（`touch_number_pad_space`、`touch_number_row`、`touch_keyboard_split_portrait`）都是共享偏好，同一个账号换一台平板也能带过去；「只在大屏上生效」在画的时候判断就够了。
- **默认打开**：用户不用去设置里找，开了单手模式就能看到。不用它：会改变现在大屏单手用户看到的键盘；默认值是产品决定，维护者定的是默认关。
- **另开一种平板布局，不挂在单手模式上**：不受单手模式侧栏的约束，可以把九键和数字键盘排得更自由。不用它：维护者决定做成单手模式的变体；另开一种布局就要先定它和分离式、浮动、单手三者的分工，#10204 已经把大屏单手的几何做好了，挂在它上面只多一块面板。
- **侧边面板铺满空出来的整块**：没有空白，看起来最满。不用它：平板横屏空出来八九百，三列键每列近 300，数字离拇指太远，键也大得不像键盘；限宽 300 贴边，空白留在中间。
- **侧边面板贴着九键（靠里）而不是贴屏幕边**：两块键挨在一起，看起来是一整块键盘。不用它：这个布局是给两只手的拇指用的（issue 原话「左边放数字键盘，右边放九键」），贴里的话左手拇指够不着。
- **4 × 4 计算器排法，加空格和跨两行的键**：和桌面小键盘一样，能多放一个空格。不用它：空格、回车都在主键区底行，离得不远，重复放没有必要；跨行的键要单独处理行高，而 3 × 4 正好和九键的三行网格加底行逐行对齐。
- **26 键也显示**：用 26 键的人也有数字需求。不用它：维护者定的是只在九键时显示；26 键有「顶部数字行」可选。
- **三端用同一个侧边宽度公式（屏幕宽减块宽）**：数字三端完全一样，读的人好对照。不用它：鸿蒙和 iPad 的按键行两边有内边距、块与面板之间有固定间距，照搬 Android 的公式在最窄的大屏上会把这一行撑出屏幕；各端按这一行实际可用的宽度算，规则（下限 150、上限 300、贴屏幕边）相同。
- **iPad 全宽键盘的单手模式另存一个键（例如 `keyboard.tablet.oneHanded`）**：iPad 浮动键盘是手机形态，本来就能开单手模式并存下 `keyboard.oneHanded`；共用一个键的话，升级后停靠的全宽键盘会突然按那个值变成单手。不用它：Android 和鸿蒙在手机、平板、折叠屏内外屏之间都只有一个单手取值，iOS 另存一个就要让磁贴按形态读写不同的键，同步、恢复默认也都要多一份。升级带来的那一次变化另用一次性标记 `keyboard.oneHandedDockedTablet` 挡住（见「各端实现 › iOS」），不靠拆键。
- **升级前存下的单手值直接带到全宽键盘上，不做迁移**：最简单，侧栏的退出按钮一下就能关。不用它：这是默认行为的变化，在浮动键盘上开过单手的 iPad 用户一升级，停靠的键盘就缩成手机大小、换成手机键面，没有经过单独授权；一次性标记只多一个 App Group 键。
- **iPad 单手时保留 iPad 的键面（数字行、Tab、iPad 底行）**：键面与平时一样，不用切换。不用它：iPad 26 键一排十一二个键，挤进 360pt 后每个键不到 30pt；iPad 浮动键盘本来就用手机键面，单手块和它一样大。

## Consequences

- 设置页关掉开关即回到 #10204 的样子；代码回滚不影响文档格式，字段留在 client-core 里没人读，与[撤掉的数字键盘两项设置](../simplification/2026-10-11-remove-number-keypad-settings.md)一样无害。
- iPad 键面切换牵涉面大：`keyFaceFormFactor` 与 `formFactor` 归错一处就会在 iPad 单手时画出半套 iPad 键面，或者让键盘高度随单手开关跳一排。数字行这一处按 `formFactor` 取，`OneHandModeTests` 钉住开着「数字行与 Tab 键」时单手前后请求的高度相同。
- 升级前在浮动键盘上存下的单手值，第一次在全宽键盘上画时被改成关闭，浮动键盘上也随之关闭一次；这之后用户在任何形态上打开的单手模式都照存下的值画。
- 组字中点侧边数字沿用数字层的做法，拼音按原样上屏再上屏数字，而不是先上屏首选；有反馈再改，届时三端一起改。
- 折叠屏展开、折叠和转屏：Android 按 `Configuration` 重算，排法签名带着宽度会重排；鸿蒙在 `readOrientation` 的显示变化回调里重算；iPad 在 `viewDidLayoutSubviews` 和 `updateKeyboardLayout` 里重算。
- 两个同步键要进后端字段表才会真的上传，在那之前同步不带这一项，本机照常生效。
- 以后加 `text_edit` 时，设置页的开关要改成选择（空着 / 数字键盘 / 文本编辑），今天的版本读到 `text_edit` 按空着画。

## Verification

- 共享偏好：`cargo test -p msime-client-core` 和 `cargo test -p msime-client-services --lib account::` 通过；默认文档不含这个键；不认识的写法存回文档不丢，账号同步应用后下一次导出写回同一个值。
- Android：`platforms/android/check-host.sh` 通过，`KeyboardFormFactorPolicySmoke` 覆盖侧边宽度（手机 0、600dp 为 176、800dp 和 1280dp 为 300、宽度未知为 0、573dp 为 0、574dp 为 150），`OneHandSidePadPolicySmoke` 覆盖 `parse` 和每一条显示条件，`SettingsSearchIndexSmoke` 搜得到新的一行，`check-host.sh` 钉住接线和设置文字；Gradle `compileFullReleaseJavaWithJavac` 通过。
- iOS：`MSIMEClientTests` 的 `build-for-testing` 和 `MSIMEApp` 编译通过。`OneHandModeTests` 的 iPad 用例（块宽、手机键面、数字行保高、升级迁移、分离让位、浮动、面板位置与对齐、显示条件、上屏和删除、空隙归属、偏好读写、设置文字）和 `TouchPointTests.testTheOneHandedTabletKeyboardRegistersTheDockedBlock` 的运行结果见 PR 的验证段。
- 鸿蒙：`platforms/harmony/tests/run.sh` 通过，`keyboard-logic.test.ts` 覆盖 `sidePadWidthVp`、`sidePanel`、`sidePadShown`、键序、读屏名称和能力位，`run.sh` 钉住接线；`hvigorw assembleHap` 编译通过，改动的文件没有新的 ArkTS 警告。共享 React 页的 `touch-keyboard-geometry-section` 和 `harmony-one-handed-side-panel` 测试覆盖开关只在能力位为真时出现、文字逐字相同、写对取值。
- `scripts/test-mobile-*-parity.py`、`test-preferences-field-parity.py`、`test-harmony-bridge-parity.py` 通过。
- 没有在 Android 平板、iPad 或鸿蒙平板的模拟器和真机上看过实际画面和手点输入。

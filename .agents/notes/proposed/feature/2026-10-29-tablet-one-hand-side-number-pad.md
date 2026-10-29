# Agent Note: 大屏单手模式空出来的一侧放数字键盘，iPad 补上单手模式

Status: proposed

## Problem

#10203（拆自用户反馈 #10202）：平板上想要「左边数字键盘、右边九键」的单手布局，双手握着平板时右手拇指打九键，左手拇指打数字。

[大屏上的单手模式缩到手机大小并贴到屏幕边](../../implemented/feature/2026-10-29-tablet-one-handed-size.md)（#10204）之后，Android 和鸿蒙在大屏上开单手模式时，「侧栏 + 键区」这一块是手机大小（Android 424dp，鸿蒙 431vp），贴到一侧，另一侧是一大块键盘底色，没有按键。那篇笔记把这个布局单独留给了 #10203。

iOS 的单手模式只在 iPhone 上画（`KeyboardOneHandLayout.effective` 对 `.tablet` 一律返回 `.off`，功能面板的「单手模式」磁贴在 iPad 全宽键盘上置灰），iPad 上根本没有可以放侧边面板的单手模式。

维护者的决定：

- 做成大屏单手模式的一个变体，用 #10204 空出来的那一侧放侧边面板，不另开一种平板布局；分离式键盘和浮动键盘不变。
- 侧边面板第一版只放数字键盘；文本编辑面板作为另一种侧边面板，不在第一版里。
- 只在主键区是九键时显示。
- 三端都做；iPad 为此补上单手模式，让同一个布局在 iPad 上也成立。

## Proposal

### 共享偏好与同步（本分支已提交）

`crates/client-core/src/preferences.rs` 新增 `touch_one_handed_side_panel: TouchOneHandedSidePanel`：

- 取值 `off`（默认，空着，即 #10204 之后的样子）和 `number_pad`。等于默认值时不写进文档（`skip_serializing_if`），默认文档与加字段之前逐字节相同，老用户什么都看不到变化。
- 读文档是宽松的：手写 `Deserialize`，不认识的字符串读作 `off`，不是字符串照样报错，与 `TouchNumberPadSpace` 相同；另有严格的 `parse` / `as_str` 给同步用。
- 做成取值而不是开关：以后加文本编辑面板时只多一个取值 `text_edit`，今天的版本读到它按 `off` 画；要是今天做成布尔，以后改成字符串时还没升级的版本会把整份偏好文档判成类型错误。
- 账号同步走触屏宿主共用的 `TouchHostKeys`，与 `touch_number_pad_space` 同一条路：Android `platform.android.one_handed_side_panel`，鸿蒙 `platform.harmony.one_handed_side_panel`，字符串。导出总是写（`off` 也写），云端才能被改回空着；应用时按严格写法解析，本机不认识的面板只跳过这一个键、不改本机的选择。服务端字段表收录这两个键之前，上传前的字段表过滤会去掉它们，这一步在仓库外（后端字段表）。iOS 的同步走 `ios_settings.rs` 的自有映射，与其他按键布局偏好一样不同步这一项。
- 共享 TS 模型 `packages/ui/src/core/preferences-model.ts` 加了 `touch_one_handed_side_panel?: "off" | "number_pad"`；`platforms/macos/tests/settings/preference_coverage.py` 把它列为 macOS 不适用。

单手模式本身仍是各端本机的设置（Android `platform.android.one_handed`，iOS App Group `keyboard.oneHanded`，鸿蒙按键反馈文件的 `oneHanded`），这一项只决定单手模式生效时空出来的那一侧放什么。

### 什么时候显示

全部满足时才显示侧边数字键盘，任何一条不满足时那一侧照旧空着：

1. `touch_one_handed_side_panel` 是 `number_pad`。
2. 单手模式此刻实际生效：存着的模式是 `left` / `right`，没有画分离式键盘，没有浮动（Android `ImeFrame.effectiveOneHanded`；鸿蒙 `OneHandedPolicy.active`；iOS 改后的 `KeyboardOneHandLayout.effective`）。
3. 大屏，并且空出来的宽度放得下（见下一节的 `sidePadWidth` 大于 0）。手机上 #10204 的块宽是 0，这一条自然不成立。
4. 主键区此刻画的是九键一族的字母层：拼音九键、九键双拼、笔画，与设置页「按键布局 › 9 键」那一组是同一族（iOS `TouchKeyLayoutOffering.nineKeySchemes`）。英文、本地模式（这时画 26 键）、123 数字层（主键区已经是数字九宫格）、#+= 层、注音九键、日文假名九键、手写、14/18 键和 26 键都不显示。
5. 按键没有被面板盖住：符号、表情、剪贴板、展开候选等面板打开时侧边面板和按键一起隐藏，它们在同一行里，跟着那一行收起或隐藏。九键长按选项（鸿蒙 `SURFACE_NINE_KEY_HOLD`）和键盘高度预览（`SURFACE_GEOMETRY`）画在同一行里，侧边面板照常显示。

### 尺寸与摆放

侧边面板贴在空出来那一侧的屏幕边上，不贴着九键：用户是两只手的拇指各管一边。它和侧栏之间剩下的地方是一块不接触摸的空白（键盘底色）。

宽度：空出来的宽度 `side = 窗口宽 - 块宽`，`side < 150` 时不显示，否则取 `min(side, 300)`，单位 Android dp、鸿蒙 vp、iOS pt。上限 300 让三列键每列约 100，和九键数字层的键差不多大；空出来的地方再宽（平板横屏有八九百），多出来的都是空白。下限 150 是三列 × 约 44 的最小可按宽度。

- Android：块宽 424（`KeyboardFormFactorPolicy.oneHandedBlockWidthDp`），竖屏 800dp 时 side 376，面板 300；最小的 600dp 平板 side 176，面板 176；横屏 1280dp 面板 300，中间空 556。
- 鸿蒙：块宽 431（`OneHandedPolicy.blockWidthVp`），算法相同。
- iOS：可用宽度是键盘宽减去 iPad 的左右内边距（`KeyboardFormFactor.padding`，8 + 8），块宽 `min(可用宽, round((360 + 6) / 0.85)) = 431`（与鸿蒙同一个公式：键区 360、侧栏占块宽 15%、侧栏与键区间距 6），`side = 可用宽 - 块宽 - 6`。iPad mini 竖屏 744pt 时 side 291，面板 291。

键面：3 列 × 4 行，与电话拨号盘相同，不随别的设置变：

```
1 2 3
4 5 6
7 8 9
. 0 ⌫
```

数字的排列固定是 1 2 3 在上，与三端九键数字层相同（[撤掉九宫格数字顺序](../../implemented/simplification/2026-10-11-remove-number-keypad-settings.md)之后已经固定）；`touch_number_pad_space` 不作用于它（它没有空格）。前三行与主键区九键网格的三行同高、逐行对齐，第四行与底行同高，所以整块面板和主键区一样高，各行的上下边对齐。键距、行距用用户的 `touch_key_spacing_tenths` / `touch_row_spacing_tenths`，圆角、颜色、按下效果、按键音和振动都跟主键区的键同一个工厂。

### 按键行为

- 数字和 `.`：走各端数字层已有的字面上屏路径（Android `ImeLayoutRows.commitNineKeyLiteral`，iOS `insertNumberPadLiteral`，鸿蒙 `KeyboardView.insertNumberPadLiteral`）：正在组的拼音先按原样上屏，再上屏这个字符；全角开关照常作用。字号与九键数字层的数字相同。
- ⌫：与主键区九键右列的 ⌫ 同一个动作和连删（组字时删组字的最后一码，没组字时删光标前一个字），图标同一个。
- 统计：数字记普通数字键（Android `KeyPressIds.forCharacter`，iOS `TypingKeyID.character`，鸿蒙 `KeyIdPolicy.character`），`.` 记 `Period`，⌫ 记 `Backspace`。热力图不画侧边面板，这些 id 本来就有。
- 不挂滑行输入、横滑移动光标、长按选项和上滑角标。

### 命中与触点几何

- 侧边面板是一个独立的按键区：落在面板里键与键之间空隙的触摸交给最近的键（Android 用一个新的 `KeyboardKeyArea`，iOS 让面板的行留在 `KeyAreaStackView` 之下、按 `KeyGapRouting` 的规则归属，鸿蒙用与主键区相同的 `fixedKey`）。面板与侧栏之间的空白不接触摸，也不把触摸交给任何键：Android 是 `OneHandRow` 里一个普通的 `View`；iOS 是一个普通 `UIView`（不是 stack view，`KeyAreaStackView.hitTest` 不会为它找键）；鸿蒙是 `Blank()`，不挂 `onClick`。
- 双拼触点的键位几何（#10135）只量 26 键字母和微软双拼的 `;`，侧边面板只在九键一族时出现，不进几何、不影响注册。iPad 新增的单手模式会让 26 键字母变窄、贴边，iOS 的 `currentKeyLayout` 本来就按实际矩形（`KeyAreaStackView.layoutFrame`）量，换代次重新注册，不用改，只补测试。

### 无障碍

- 数字键读「数字 1」…「数字 0」，与三端数字层的数字键相同（Android 经 `keyboardKey` 读作「按键 数字 1」）；`.` 读「小数点」；⌫ 读「删除」。
- 面板与侧栏之间的空白不是无障碍元素。
- 测试用标识：iOS 面板 `accessibilityIdentifier = "oneHandSidePad"`，键 `sidePadDigit0`…`sidePadDigit9`、`sidePadDecimal`、`sidePadDelete`，空白 `oneHandSideSpacer`；Android 面板 `setTag(OneHandSidePad.TAG)`；鸿蒙面板 `.id('oneHandSidePad')`。

### 设置页

三端文字逐字相同，各端测试钉住：

- 标题：「大屏单手时侧边显示数字键盘」
- 说明：「单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。」

开关打开写 `number_pad`，关掉写 `off`（client-core 存成缺键）。这一项只在大屏上列出：

- Android：`KeyboardOptionsPage` 第一个 `if (showLargeScreenOptions)` 块里「分离中间空隙」之后，用 `savePreference("touch_one_handed_side_panel", checked ? "number_pad" : "off")`；`PageId` 的设置搜索关键词加「单手」「侧边数字键盘」。
- iOS：`KeyboardLayoutSettingsView.tabletGroup`（只在 iPad 上出现）里「横屏分离式键盘」之后，`accessibilityIdentifier("appOneHandSidePadSwitch")`，经新的 `TouchOneHandedSidePanelPreference.save(_:)` 用 `MetasequoiaInputSessionBridge.updateSharedPreferences` 写共享文档（与 `TouchKeyLayoutStylePreference.save` 同一个写法）。这一组的说明加一段 iPad 单手模式：「功能面板的单手模式在 iPad 全宽键盘上把按键缩到手机大小、贴到一侧；横屏分离式键盘画着时单手模式让位。」「恢复默认」不动它（它不是这一页的间距或高度）。
- 鸿蒙：共享 React 设置页 `TouchKeyboardGeometrySection` 的「布局」组，新增可选属性 `oneHandedSidePanel?: boolean` 和 `onOneHandedSidePanelChange`，`screen-keyboard-page.tsx` 只在宿主报告 `touch_one_handed_side_panel` 时传入（`draft.touch_one_handed_side_panel === "number_pad"`），写 `{ touch_one_handed_side_panel: enabled ? "number_pad" : "off" }`。手机排法（`layoutRows` 给出时）照例不画说明。能力位加进 `packages/ui/src/core/host-capabilities.ts`（缺省按不提供处理），鸿蒙 `Settings.ets` 的能力投影里按设备形态报告：`SettingsFormFactorCapabilities` 新增 `oneHandedSidePanel`，`tablet` 为真，`2in1` 为假，`phone` 在可折叠设备（`display.isFoldable()`）上为真。

## 各端实现

### Android

- `KeyboardFormFactorPolicy`：新增 `ONE_HANDED_SIDE_PAD_MAX_WIDTH_DP = 300`、`ONE_HANDED_SIDE_PAD_MIN_WIDTH_DP = 150` 和 `oneHandedSidePadWidthDp(int smallestWidthDp, int screenWidthDp)`：块宽为 0（手机）或宽度未知时返回 0，`screenWidthDp - oneHandedBlockWidthDp(...)` 小于下限返回 0，否则取上限内的值。
- 新的纯逻辑 `keyboard/OneHandSidePadPolicy.java`：`PREFERENCE_KEY = "touch_one_handed_side_panel"`，`Panel parse(String)`（不认识按 `OFF`），`shown(Panel panel, String effectiveMode, int widthDp, int displayedLayout, KeyboardLayout.Layer layer, boolean numberPadDrawn)`：第 4 条的「九键一族」是 `QUANPIN_NINE_KEY_LAYOUT` 和 `STROKE_LAYOUT`、`LETTERS` 层、没画数字九宫格。
- `MSIMEInputService`：在 `applyTouchGeometry` 和重读偏好（`numberPadSpaceFrom` 的旁边）读这一项存进 `oneHandedSidePanel`；进 `SKIN_HINT_KEYS`，冷启动第一帧就按它画。
- 新的 `core/ImeSidePad.java`（或放进 `ImeFrame` 的一个内部类，看大小）：建一个 `KeyboardKeyArea`（`spacedKey` 用 `s::followsKeySpacing`），竖排：上面一块三行（`setTag(KeyboardHeightRole.nineKeyGrid())`）、下面一行（`setTag(KeyboardHeightRole.bottomRow())`），键用 `s.keyboardKey(digit, "数字 " + digit, () -> s.imeLayoutRows.commitNineKeyLiteral(digit))`、`nineKeyDecimalKey` 同样的小数点和 `s.backspaceKey(...)` + `bindBackspaceRepeat`，键按 `addNineKey` 的权重排；建好后 `s.imeStyler.applyKeyboardGeometry(pad)` 和 `applyKeyboardHeight(pad)` 让键距、行高与主键区相同。键行重建（`ImeLetterRows.rebuildKeyRows`，换皮肤、改间距、改高度都会走到）时作废，下次用到时重建。
- `ImeFrame.applyOneHanded`：`blockDp > 0` 时再算 `padDp = oneHandedSidePadWidthDp(...)` 和 `OneHandSidePadPolicy.shown(...)`；`applied` 签名加上这两项，变了才重排。显示时 `OneHandRow` 的子视图按 `[面板, 空白(weight 1), 侧栏, 键列]`（键在右）或 `[键列, 侧栏, 空白, 面板]`（键在左）排，面板宽 `s.pixels(padDp)`、高 `MATCH_PARENT`（行高就是键列的高度），这时 `row.setGravity(Gravity.NO_GRAVITY)`；不显示时摘掉面板和空白，回到 #10204 的排法。面板在 `row` 里，`setKeysCollapsed` / `setKeysCovered` 已经让整行收起或隐藏，面板跟着走。
- `applyOneHanded` 每次渲染都会被 `ImeStyler.refreshSeasonIfNeeded` 调到，切 123、切中英、切方案都经过渲染，不用另挂触发点。
- 设置页：`KeyboardOptionsPage`、`PageId`，见上文。

### iOS

- `KeyboardOneHandLayout`（`KeyboardOneHandGutterView.swift`）：
  - `effective(_:formFactor:split:)`：手机照旧返回存着的值；`.tablet` 在画分离式键盘时返回 `.off`，否则返回存着的值。调用方 `applyOneHanded` 传 `splitKeyboardShown`；`updateKeyboardLayout` 改了分离与否之后再调一次 `applyOneHanded`。
  - `tabletKeysWidth = 360`，`blockWidth(available:formFactor:)`：手机返回 `available`（照旧按行宽 15% 切侧栏），`.tablet` 返回 `min(available, round((360 + gap) / (1 - gutterRatio)))`。
  - `sidePadWidth(available:formFactor:)`：手机返回 0；`.tablet` 按上文的 side、下限 150、上限 300。
  - `keysWidth(available:mode:formFactor:)` 改成按块宽算，`updateLetterRowInsets` 跟着传。
- `installOneHandRow`：`oneHandRow` 里加一个空白 `oneHandSideSpacer`（普通 `UIView`，`isAccessibilityElement = false`）和侧边面板 `oneHandSidePad`（竖排 `UIStackView`，三行数字 + 一行 `. 0 ⌫`，键用 `makeKey` 建，与九键数字层同一种键面），默认都隐藏。iPad 单手时侧栏宽度改成块宽的 15% 的常量约束、键列宽度是常量，`oneHandRow` 的 `distribution` 保持 `.fill`，空白吃掉剩下的宽度；面板宽度是 `sidePadWidth` 的常量约束。面板前三行的高度等于 `nineKeyContainer` 的高度（约束到它的 `heightAnchor`，已经含两道行距），第四行等于 `actionRow` 的高度。
- `applyOneHanded`：排列次序与 Android 相同，`appliedOneHanded` 之外再记面板是否显示、宽度多少，变了才重排。显示条件里的「九键一族字母层」是 `(nineKey || strokes) && !showsSymbols && !numberPadShown`。功能菜单等面板盖住键区时，现在是把 `oneHandRow` 的祖先一起压暗遮住（`OneHandModeTests.testTheStoredModeComesBackAndPanelsCoverTheColumn`），侧边面板在同一行里，跟着被遮住，不用另做。
- iPad 单手时键面用手机的：新增 `keyFaceFormFactor`，`formFactor == .tablet && appliedOneHanded != .off` 时为 `.phone`，否则等于 `formFactor`。键面相关的判断改读它：`layoutInputs.formFactor`、`applyTabletLetterKeys`、`canShowFullKeys`（不画 iPad 数字行和 Tab）、`showsLetterRowPunctuation`、`usesPhoneBottomRow` / `usesTabletBottomRow`、`letterFontSize`、`updateLetterRowInsets` 的中间行缩进、`symbolLayerSymbolFirst` 里的 `formFactor == .phone`、`phoneLetterVerticalPadding` 一类。键盘高度、上下内边距、`topRowGap` 和按键高度（`keyHeight`、`keyboardHeight`、`keyRows`）仍读 `formFactor`，单手开关不改键盘外框的高度。`KeyboardViewController` 里 `formFactor` 共 36 处，逐处归到这两类之一，实现时在 PR 里列出归类。
- 功能面板的「单手模式」磁贴不再按 `formFactor == .phone` 置灰；iPad 浮动键盘和窄窗口本来就是 `.phone`，行为不变。
- 侧边面板读 `session.sharedPreferences` 里的 `touch_one_handed_side_panel`（与 `TouchToolbarPreference(in:)` 同一种读法，新的 `TouchOneHandedSidePanel(in:)` 放在 `SharedUI/preferences`，不认识的值按 `.off`），偏好变了与按键布局偏好一样重排。
- 设置页：`KeyboardLayoutSettingsView.tabletGroup`，见上文。

### 鸿蒙

- `OneHandedPolicy.ts`：新增 `SIDE_PAD_MAX_WIDTH_VP = 300`、`SIDE_PAD_MIN_WIDTH_VP = 150`、`sidePadWidthVp(screenWidthVp: number, largeScreen: boolean): number`（块宽为 0 或宽度未知时返回 0），`sidePanel(value: string | null | undefined)`（`off` / `number_pad`，其他按 `off`），以及 `sidePadShown(...)` 汇总上文五条。
- `KeyboardSession.ets`：读共享偏好时取 `touch_one_handed_side_panel` 交给视图（与 `numberPadSpace` 同一处）。
- `KeyboardView.ets`：`readOrientation` 里在 `oneHandBlockVp` 旁边算 `oneHandSidePadVp`。`handedKeys()` 的外层 `Row` 在 `sidePadShown()` 时按 `[面板, Blank(), 块]`（键在右）或 `[块, Blank(), 面板]`（键在左）排，`justifyContent` 改为 `FlexAlign.SpaceBetween`；不显示时与 #10204 相同。面板是新的 `@Builder oneHandSidePad()`：`Column` 里上面一块高 `this.gridHeight()`、三行 `Row`，下面一行高 `this.rowHeight(3)`，数字键用 `fixedKey`（`label: '数字 N'`，动作 `softKeyPress(KeyIdPolicy.character(d))` 再 `insertNumberPadLiteral(d)`），⌫ 用 `deleteKey('100%', true, 0)`，`.` 与 `nineKeySeparatorKey` 数字层那一格同样的写法但不看 `this.symbols`。字号与数字层的数字相同。
- 设置：`SettingsFormFactorCapabilities.ts` 的 `oneHandedSidePanel`，`Settings.ets` 写进能力文档；共享 React 页见上文。

## Alternatives considered

- **做成布尔开关 `touch_one_handed_side_number_pad`**：第一版的设置页就是一个开关，布尔最直接，读的人一眼就懂。不用它：维护者已经说了文本编辑面板会是另一种侧边面板，到时要么再加一个互斥的布尔（两个都开时画什么要另定规则），要么把布尔改成字符串，而还没升级的版本读到字符串会把整份偏好文档判成类型错误。现在做成取值，代价只是多一个枚举。
- **存成各端本机设置，跟单手模式本身放在一起**（Android `platform.android.*` 本机键、iOS App Group、鸿蒙按键反馈文件）：单手模式就是本机设置，侧边面板也只在大屏上有意义，放在一起最顺。不用它：鸿蒙的设置页是共享 React 页，只会写共享偏好，存成本机设置要另开一条桥；按键布局类的选项（`touch_number_pad_space`、`touch_number_row`、`touch_keyboard_split_portrait`）都是共享偏好，同一个账号换一台平板也能带过去；「只在大屏上生效」在画的时候判断就够了。
- **默认打开**：用户不用去设置里找，开了单手模式就能看到。不用它：会改变现在大屏单手用户看到的键盘；默认值是产品决定，维护者定的是默认关。
- **另开一种平板布局，不挂在单手模式上**：不受单手模式侧栏的约束，可以把九键和数字键盘排得更自由。不用它：维护者决定做成单手模式的变体；另开一种布局就要先定它和分离式、浮动、单手三者的分工，#10204 已经把大屏单手的几何做好了，挂在它上面只多一块面板。
- **侧边面板铺满空出来的整块**：没有空白，看起来最满。不用它：平板横屏空出来八九百，三列键每列近 300，数字离拇指太远，键也大得不像键盘；限宽 300 贴边，空白留在中间。
- **侧边面板贴着九键（靠里）而不是贴屏幕边**：两块键挨在一起，看起来是一整块键盘。不用它：这个布局是给两只手的拇指用的（issue 原话「左边放数字键盘，右边放九键」），贴里的话左手拇指够不着。
- **4 × 4 计算器排法，加空格和跨两行的键**：和桌面小键盘一样，能多放一个空格。不用它：空格、回车都在主键区底行，离得不远，重复放没有必要；跨行的键要单独处理行高，而 3 × 4 正好和九键的三行网格加底行逐行对齐。
- **26 键也显示**：用 26 键的人也有数字需求。不用它：维护者定的是只在九键时显示；26 键有「顶部数字行」可选。
- **iPad 全宽键盘的单手模式另存一个键（例如 `keyboard.tablet.oneHanded`）**：iPad 浮动键盘是手机形态，现在就能开单手模式并存下 `keyboard.oneHanded`；共用一个键的话，升级后停靠的全宽键盘会突然按那个值变成单手。不用它：Android 和鸿蒙在手机、平板、折叠屏内外屏之间都只有一个单手取值，iOS 另存一个就要让磁贴按形态读写不同的键，同步、恢复默认也都要多一份；受影响的只是在浮动键盘上开过单手模式的 iPad 用户，侧栏上的退出按钮一下就能关掉。这条记进风险。
- **iPad 单手时保留 iPad 的键面（数字行、Tab、iPad 底行）**：键面与平时一样，不用切换。不用它：iPad 26 键一排十一二个键，挤进 360pt 后每个键不到 30pt；iPad 浮动键盘本来就用手机键面，单手块和它一样大。

## Acceptance criteria

- 共享偏好：`cargo test -p msime-client-core` 和 `cargo test -p msime-client-services --lib account::` 通过（本分支已跑过）；默认文档不含这个键。
- Android：`platforms/android/check-host.sh` 通过，`KeyboardFormFactorPolicySmoke` 覆盖侧边宽度（手机 0、600dp 为 176、800dp 为 300、宽度未知为 0、放不下下限为 0），新的 `OneHandSidePadPolicySmoke` 覆盖显示条件的每一条和 `parse` 的未知值；平板模拟器（sw600dp 以上）上截图：竖屏、横屏、键在左、键在右、切 123 时面板消失、打开符号面板时面板随按键隐藏、分离式和浮动时不出现；在面板上点数字、小数点和 ⌫ 输入正确，组字中点数字先上屏拼音再上屏数字。
- iOS：`OneHandModeTests.testTheTabletKeyboardIgnoresIt`（iPad 不画侧栏、磁贴置灰）改写成 iPad 单手生效；另增加 iPad（regular 宽度）用例：块宽 431、按键宽 360、分离时单手让位、侧边面板只在九键一族字母层出现、宽度与位置（贴屏幕边）、`sidePadDigit5` 上屏 `5`、`sidePadDelete` 删字、123 层和 26 键不出现、浮动（compact）时与以前相同；iPad 单手时 26 键是手机键面（没有 Tab 和 iPad 数字行，是手机底行）；`TouchPointTests` 增加 iPad 单手的 26 键几何比全宽窄、贴边；App 设置页开关写 `touch_one_handed_side_panel`；iPad 模拟器截图。
- 鸿蒙：`platforms/harmony/tests/run.sh` 通过，`keyboard-logic.test.ts` 覆盖 `sidePadWidthVp`、`sidePanel`、`sidePadShown`；`hvigorw assembleHap` 编译通过，改动的文件没有新的 ArkTS 警告；共享 React 页的测试覆盖开关只在能力位为真时出现、写对取值；平板模拟器截图。
- 三端设置页的标题和说明逐字相同，有测试钉住。
- 落地时把这篇转为 `implemented/feature/`，`## Proposal` 改为现在时的 `## Decision`。

## Risks

- **iPad 键面切换牵涉面大**：`formFactor` 在 `KeyboardViewController` 里 36 处，归错一处就会在 iPad 单手时画出半套 iPad 键面或者高度跳动。缓解：逐处归类并在 PR 里列出，用例覆盖 26 键、九键、14/18 键、手写和符号层在 iPad 单手下的样子。
- **浮动键盘存下的单手值进到全宽键盘**：见备选里的说明。受影响的是在 iPad 浮动键盘上开过单手模式的用户，升级后停靠的全宽键盘会是单手；侧栏的退出按钮能关。
- **组字中点侧边数字**：沿用数字层的做法，拼音按原样上屏再上屏数字，而不是先上屏首选。和主键区数字层一致，但有人可能期待先选字；有反馈再改，届时三端一起改。
- **折叠屏展开、折叠和转屏**：Android 按 `Configuration` 重算，`applyOneHanded` 的签名带着宽度会重排；鸿蒙在 `readOrientation` 里重读；iPad 在 trait 和窗口尺寸变化时走 `updateKeyboardLayout`。没有真机，折叠过程中的中间态只能在模拟器上看。
- **服务端字段表**：两个同步键要进后端字段表才会真的上传，在那之前同步不带这一项，本机照常生效。
- **验收缺口**：维护者这一轮不在本机起模拟器、没有真机，上面列的截图和手点验收要在 PR 上或之后补上，补不上就写进 PR 的验证段。
- **回滚**：设置页关掉开关即回到 #10204 的样子；代码回滚不影响文档格式，字段留在 client-core 里没人读，与[撤掉的数字键盘两项设置](../../implemented/simplification/2026-10-11-remove-number-keypad-settings.md)一样无害。
- **文本编辑面板不在第一版**：以后加 `text_edit` 时，设置页的开关要改成选择（空着 / 数字键盘 / 文本编辑），今天的版本读到 `text_edit` 按空着画。

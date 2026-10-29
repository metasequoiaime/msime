# Agent Note: 26 键「符」与「123」互换和数字键盘空格位置两项共享偏好

Status: implemented

## Problem

#10056：从别的输入法转过来的用户，肌肉记忆里的键位和水杉不一样，经常按错。他要两个开关：拼音 26 键底行交换「符」和「123」；数字键盘交换 0 和空格。

26 键底行三端都是 `123 |（🌐）| 符 | …`（[逗号长按出句号](2026-10-16-comma-key-secondary-period.md)之后的次序），没有让 符 在前的选项。九宫格数字键盘的 0 和空格三端本来就不一样：Android 是 0 占底行中间、空格在右列（[Android 九键数字层把 0 移到底行中间](2026-10-16-android-nine-key-numeric-bottom-row.md)），iOS 是底行 `0 | 空格`、0 占大份，鸿蒙是底行 `空格 | 0`、空格占大份。维护者的决定：三端都做，默认值都不变；数字键盘做成在这三种排法之间切换，三端提供同样的三个选项，各端默认是自己原来的排法。

## Decision

共享偏好（`crates/client-core/src/preferences.rs` 的 `Preferences`）在触屏那组字段里新增两项，等于默认值时不写进文档（`skip_serializing_if`），默认文档与加字段之前逐字节相同：

- `touch_twenty_six_symbol_first: bool`，默认关。打开后手机 26 键字母层底行把 符 挪到 123 左边：`符 | 123 |（🌐）| …`，其余键的次序和宽度份额不变，地球键仍紧跟 123，与九键外框的前两键同序。只作用于底行里 符 排在 123 后面的行：中文 26 键的标准和「逗号句号在空格两侧」，以及英文、韩文等其他语言的 26 键底行（它们同样以 `123 | 符` 打头，切中/英时 符 不换位置）；长空格样式的 符 在工具栏、表情键样式的 😀 占了 符 的位置，这两种不变；14/18 键、手写和大千注音的底行没有 符，九键外框本来就以 符 打头，iPad 全尺寸键盘和鸿蒙 2in1 有自己的底行，都不受影响。密码框和只收数字的框里底行样式退回标准，互换不退回：它是键位习惯，标准底行同样有 符。
- `touch_number_pad_space: TouchNumberPadSpace`，九宫格数字键盘（拼音九键、九键双拼、笔画自己的数字层，以及 26 键、14/18 键、手写借来的数字九宫格）里空格放在哪，0 总在底行。取值：
  - `platform_default`（默认）：各端原来的排法，Android 是 `right_column`，iOS 是 `after_zero`，鸿蒙是 `before_zero`。
  - `right_column`：空格在右列最下面一格（右列 ⌫ | . | 空格），0 占底行里原来 0 和空格合起来的份额。
  - `after_zero`：0 和空格都在底行，0 在左占大份，空格与中/英同宽（1.05）。
  - `before_zero`：0 和空格都在底行，空格在左占大份，0 与中/英同宽（1.05）。

  整行总份额在三种排法下都不变，大份 = 原来的空格份额减 1.05。空格不在右列时右列最下面一格是 ！，与 iOS、鸿蒙原来的数字层右列相同（Android 的数字层右列这一格原来就是空格，换走后放 ！，按字面上屏、记 `SoftPunctuation`，不带字母层那个 ！ 上滑出 0 的手势）。空格搬到右列时用的仍是同一个空格键或同一套手势（点按、拖动移光标、长按语音）。右侧大回车的数字层三端已经统一（[按键布局样式](2026-10-27-touch-key-layout-styles.md)），右列没有空位放空格，这一项不作用于它。

读文档是宽松的：`TouchNumberPadSpace` 手写 `Deserialize`，不认识的字符串按 `platform_default` 读，不是字符串照样报错，与 `TouchTwentySixKeyStyle` 等相同；另有严格的 `parse` / `as_str` 给同步用。

### 互换与借来的数字九宫格

[数字九宫格的返回键和符号入口留在字母层那一格](../bug-fix/2026-10-29-mobile-number-pad-entry-position.md)的规则照旧成立：返回键留在字母层 123 那一格，符 留在字母层 符 那一格。打开互换后 26 键字母层以 符 打头，从它借来的数字九宫格因此按九键一族的次序排（`符 | 返回 |（🌐）| …`）：

- Android：`KeyboardActionRow.numberPadLayerFirst(sourceLayout, style, symbolFirst)` 按来源键面此刻的样式和互换判断第一格，`numberPadEntries` 照它排。
- iOS：`updateKeyboardLayout` 的 `symbolFirstSource`（借来的数字页、来源是中文 26 键、样式里有 符）让借来的数字页也走 `TouchBottomRowLayout.nineKeyDigits`，底行有 符 时符号栏里的第二个 符 照旧隐藏。
- 鸿蒙：`KeyboardView.letterRowSymbolLeads()` 按字母层设计底行（不看此刻在不在数字层）算出以 符 打头时，`NineKeyLayout.actionRow` 的 `nineKeyDigits` 为真，借来的数字九宫格带上 符。

表情键样式没有 符，借来的数字页照旧以返回键打头。

### 互换与 123 / #+= 层

英文、本地模式和写 ASCII 标点的方案（韩文、越南文、藏文，以及日文罗马字）的 26 键，123 打开的是自带底行的 123 / #+= 层（`返回 | 😀 或 符号 | …`），中文 26 键的 #+= 也是这一层。互换后字母层的 123 在第二格，这一层的返回键要是还在第一格，用户点 123 再在同一处点一下回不去，点到的是 😀 或 符号。所以同一条规则也用在这一层：字母层底行确实被互换过（原来 `123 | 符` 打头，换后 `符 | 123`）时，这一层的底行换成 `😀 或 符号 | 返回 | …`，两键的宽度份额正好是字母层的 符（1.05）和 123（1.25），返回键与 123 同一格、同宽。字母层没被互换的不变：长空格样式（这一层左下是逗号）、表情键样式、14/18 键、手写、九键和笔画。

- Android：`KeyboardActionRow.symbolFirstMoves(layout, style)` 判断字母层是否真的换了，`ImeLetterRows.rebuildDesignLayer` 把它传给 `KeyboardLayout.numberLayer` / `moreSymbolLayer` 的 `symbolFirst`，建层时记下（`builtLayerSymbolFirst`），设置变了由 `layerPunctuationStale` 重建。
- iOS：`updateKeyboardLayout` 的 `symbolLayerSymbolFirst`，`arrangeActionRow` 的 `.symbolLayer` 按它把 😀、符号、逗号排在返回键前面。
- 鸿蒙：`KeyboardView.designLayerSymbolFirst()`（复用 `letterRowSymbolLeads`，九键和笔画除外）决定 `designLayerFace` 底行先画 `designLayerPanelKey` 还是 `designLayerBackKey`。

### 账号同步

两项随账号同步，走触屏宿主共用的 `TouchHostKeys`，与 `touch_quanpin_shift` 和三种底行样式的先例相同：

| 共享偏好 | Android 键 | 鸿蒙键 | 值 |
| --- | --- | --- | --- |
| `touch_twenty_six_symbol_first` | `platform.android.twenty_six_symbol_first` | `platform.harmony.twenty_six_symbol_first` | 布尔 |
| `touch_number_pad_space` | `platform.android.number_pad_space` | `platform.harmony.number_pad_space` | 字符串 |

导出总是写这两个键，默认值也写（`platform_default`），云端才能被改回各端原来的排法；`platform_default` 应用到另一端时就是那一端自己的出厂排法，用户点过的具体排法则原样同步过去。应用时按严格写法解析，本机不认识的排法只跳过这一个键。服务端字段表收录这四个键之前，上传前的字段表过滤会去掉它们。iOS 的同步走 `ios_settings.rs` 的自有映射，与其他按键布局偏好一样暂不同步。

### 设置页

三端都放在「按键布局」子页里，文字三端逐字相同，各端测试钉住：

- 26 键一组的单选之后是开关「26 键「符」与「123」互换」，说明「符 排在 123 左边；长空格和表情键样式没有 符，不受影响。」打开时这组每个样式的底行示意图也把 符 画在 123 左边。
- 9 键一组之后是「数字键盘」一组单选：「空格在右列」「空格在 0 右边」「空格在 0 左边」，组末说明「九宫格数字键盘里空格的位置；右侧大回车不受影响。」没选过时勾本端原来的那一项；点哪一项写哪一项的写法（包括与本端默认相同的那一项），不写 `platform_default`。这一组没有示意图。本版本至少有一组按键布局（即有「按键布局」入口）就列，九宫格数字键盘从各类中文键面都进得去。
- 键盘页「按键布局」入口的副标题从「各类键盘的底行样式和顶部数字行」改为「各类键盘的底行样式、键位和顶部数字行」：子页多了互换开关和数字键盘空格位置，只说底行样式和数字行的副标题会让人找不到这两项。「键位」把两项都盖住，副标题不逐项列举，免得在手机上折成两行。三处文字分别是 Android `KeyboardOptionsPage` 的入口行、iOS `TouchKeyLayoutOffering.entrySubtitle`、共享 React 页的 `keyLayoutEntryDescription`，[按键布局样式](2026-10-27-touch-key-layout-styles.md)的「设置页说明文字」一并更新。

文字的位置：Android 是 `TouchKeyStyle.SYMBOL_FIRST_TITLE`、`SYMBOL_FIRST_NOTE`、`NUMBER_PAD_SPACE_GROUP`、`NUMBER_PAD_SPACE_NOTE` 和 `NumberPadSpace.title`；iOS 是 `TouchKeyLayoutOffering.symbolFirstTitle`、`symbolFirstSubtitle`、`numberPadTitle`、`numberPadFooter` 和 `TouchNumberPadSpace.title`；鸿蒙走共享 React 设置页，文字在 `packages/ui/src/keyboard/touch-key-layout-styles.ts` 的 `symbolFirstTitle`、`symbolFirstDescription`、`numberPadSpaceTitle`、`numberPadSpaceNote`、`numberPadSpaceOptions`。共享 React 页只在鸿蒙手机上给出这一页，`numberPadSpace(preferences)` 因此把缺键和 `platform_default` 读成鸿蒙的 `before_zero`。

[撤掉「26 键按 123」和「九宫格数字顺序」两项设置](../simplification/2026-10-11-remove-number-keypad-settings.md)曾因为设置页上的数字键盘选项看不懂、很少有人找而把它们撤掉。这一次的数字键盘选项由维护者为 #10056 明确要求，放在「按键布局」子页里，选项名直接说空格在哪，组末一句说明；那篇撤掉的两项（26 键按 123 画什么、1–9 的排列）不因本篇恢复。

### 热力图与触点几何

三端统计热力图的 26 键底行跟着互换画（Android `KeyHeatmapLayout.soft(style, numberRow, symbolFirst)`，iOS `TypingKeyHeatmap.letterRows(for:)`，鸿蒙 `phoneHeatmapLayoutRows` 经 `withSymbolFirst`），统计 id 不变。热力图不画数字键盘，空格位置不进热力图。双拼触点的键位几何（#10135）只上报 26 个字母键和微软双拼的 `;`，底行和数字键盘不在里面，两项都不影响它。

## 各端实现

### Android

- 排布在 `KeyboardActionRow.designEntries(..., style, symbolFirst, space)` 和 `numberPadEntries(..., style, symbolFirst, space)`，旧重载按「不互换、`platform_default`」转发，原有调用方和冒烟不变。数字层空格位置的份额常数是 `NUMBER_PAD_SMALL_KEY_WEIGHT`（= 中/英的 1.05）。
- `TouchKeyStyle` 新增 `SYMBOL_FIRST_PREFERENCE`、`NUMBER_PAD_SPACE_PREFERENCE` 和 `NumberPadSpace`（`resolved()` 把 `platform_default` 换成 `RIGHT_COLUMN`）。`MSIMEInputService` 在 `applyTouchGeometry` 与重读偏好时读两项，两项进了 `touchGeometryKey` 和皮肤片段 `SKIN_HINT_KEYS`（冷启动第一帧就按用户的选择画）；`ImeBottomRow.updateActionRow` 的底行签名带着两项。
- `ImeLayoutRows.rebuildNineKeyRows` 的数字层右列按 `NumberPadSpace.inRightColumn()` 放空格（`adoptBarKey` 搬同一个 `spaceButton`）或 ！，并记下建成时的状态；换了空格位置时 `numberPadSpaceStale()` 让下一次渲染重建键行。
- 设置页 `KeyLayoutStylesPage`：26 键一组末尾的开关、「数字键盘」一组单选；`BottomRowPreview` 多一个 `symbolFirst` 参数；设置搜索关键词加了这几个选项名。后来按键布局页改版（[按键布局设置页改版](2026-10-29-key-layout-page-redesign.md)），开关移到 26 键分段卡片里，「数字键盘」改成等宽胶囊，`BottomRowPreview` 删除，`symbolFirst` 经 `KeyHeatmapLayout.soft` 画进新的整块预览。

### iOS

- `TouchKeyLayoutStyle` 多两项 `symbolFirst`、`numberPadSpace`，App Group 镜像键 `keyboard.style.symbolFirst`、`keyboard.style.numberPadSpace`，各有 `save`。`activeKeyLayoutStyle` 在密码框和数字框里只退回样式，两项照旧带着。
- `TouchBottomRowLayout.symbolFirst`、`nineKeyDigits(…, space:)`、`borrowedNumberPad(globe:language:space:)`。手机 26 键标准底行仍走原来的约束，`arrangeActionRow` 按 `symbolFirst` 换次序，宽度不变；非标准样式在 `styledBottomRow` 里套 `symbolFirst`。
- 九键一族自己的数字层（和互换后借来的数字页）按 `nineKeyDigits(space:)` 排；`styledActionWidths` 在行里没有空格时改以 0 为基准。借来的数字页（返回键打头的那种）在 `after_zero` 时仍走原来的约束，另两种由 `borrowedNumberPadWidths` 定宽：不带地球键时与上方五列对齐（各占一列的键与数字键同宽，占两列的拿剩下的），带地球键时按 `borrowedNumberPad` 的份额相对返回键定宽。
- 「空格在右列」时右列第三格是 `numberPadColumnSpace`：`makeSpaceKey` 做出的第三个空格键，叠在 ！ 上、按它的四边约束，进 `spaceKeys`（标题和读屏随底行空格更新）和 `KeyAreaStackView.overlayKeys`（分到相邻键距里靠它的那一半）；这时底行空格隐藏，！ 和右侧大回车时一样留着占位、不画不接触摸（`applyNineKeyClosingCovered`）。语音面板关掉时读屏焦点回到此刻显示着的那个空格键（`closeKeyboardPicker`），不落在藏起来的底行空格上。
- 设置页 `KeyboardKeyLayoutSettingsView`：`choiceGroup` 的示意图变成可选，多一个 `trailing` 放互换开关。

### 鸿蒙

- `TouchLayoutStylePolicy.symbolFirst(row, enabled)`、`symbolLeads(row)`、`numberPadSpace(value)`（缺键、`platform_default`、不认识的值都按 `before_zero`）；`NineKeyLayout.actionRow` 多一个 `space` 参数，缺省是 `before_zero`，`tallReturnRow` 用的标准行不受影响。
- `KeyboardSession` 读两项进 `PreparedPreferences`；`KeyboardView` 的 `twentySixSymbolFirst`、`numberPadSpace` 随 `applySettings` 换，进了 `faceKey()`。`designRowEntries` 套 `symbolFirst`，`nineKeyActions` 传 `space`；`touchNineKeyGrid` 右列最下面一格在数字层「空格在右列」时画 `spaceKey`，其余时候是 ！。
- 共享 React 设置页 `TouchKeyLayoutSettings`：`StyleChoices` 的示意图变成可选，多一个 `children` 放互换开关；热力图 `phoneHeatmapLayoutRows` 经 `withSymbolFirst` 画互换后的底行，`TouchKeyLayoutStyle` 多一个可选的 `twentySixSymbolFirst`。

### 既有笔记审计

- [按键布局样式与顶部数字行](2026-10-27-touch-key-layout-styles.md)：部分重叠，沿用。两项照它的做法进共享偏好、`TouchHostKeys` 和同一个设置子页；它的底行样式表不变，互换是叠在样式表上的一步。
- [数字九宫格的返回键和符号入口留在字母层那一格](../bug-fix/2026-10-29-mobile-number-pad-entry-position.md)：规则沿用，互换后的 26 键按「字母层以 符 打头」那一支排；它「三端仍不同的地方」里 0 与空格的排法现在可选，默认值不变。
- [Android 九键数字层把 0 移到底行中间](2026-10-16-android-nine-key-numeric-bottom-row.md)：部分被本篇取代。它的排法是 Android 的默认（`platform_default` / `right_column`），「没有新增偏好」「本次只改变 Android」两点不再成立：现在有共享偏好，三种排法三端都能选。
- [逗号长按出句号](2026-10-16-comma-key-secondary-period.md)：沿用，26 键默认仍是 `123 | 符`。
- [撤掉数字键盘两项设置](../simplification/2026-10-11-remove-number-keypad-settings.md)：不被取代，见上文「设置页」。
- [Android 语言与数字键交换](2026-10-14-android-language-number-key-swap.md)：沿用，交换叠在互换之后，符 仍在最左；数字层同理。

## Alternatives considered

- **数字键盘只给两个选项（「0 在中间」/「空格在中间」）**：设置页最简单，也与 issue 里「交换 0 和空格」的说法最接近。不采用：Android 与 iOS 都是 0 占大份，但空格一个在右列、一个在 0 右边，两选项装不下三端各自原来的排法，默认值就得改，而维护者要求各端默认不变、三端选项相同。
- **借这次把三端数字键盘统一成一种默认**：三端从此一致，不需要 `platform_default`。不采用：会改变现有用户的手感，维护者明确要求默认值不变。
- **`touch_number_pad_space` 用 `Option` 表示「没选过」**：少一个枚举值。不采用：同步要能把「回到各端原来的排法」传给别的设备，一个明写的 `platform_default` 字符串在同步键里直接表达这件事，`None` 在导出时还得另找写法。
- **按 #10061 报告人的设想做「符 / 123 / 中」三键自定义排序**：一项覆盖所有排法。不采用：要在三端设置页加排序面板、迁移 Android 本机的「交换语言键与数字键」，还要和各样式组合；#10056 要的就是一个开关，[数字九宫格入口位置](../bug-fix/2026-10-29-mobile-number-pad-entry-position.md)当时也把排序面板留给了后续。
- **两项做成各端本机设置**：改动小。不采用：同一账号的几台设备要各设一遍，三端取值会漂移；照 `touch_quanpin_shift` 和底行样式的先例做成共享偏好。
- **数字键盘空格位置也作用于右侧大回车**：用户选了什么，所有数字层都一样。不采用：右侧大回车的数字层三端刚统一成 `切换键 | 0 | 空格 | 中/英`，右列被两倍高的回车占着，放不下「空格在右列」；只调 0 和空格次序又会让这个外框多一种排法，设置页组末说明写明它不受影响。
- **密码框和数字框里互换也退回**：与底行样式一致。不采用：样式退回是因为长空格、表情键会把 符 挪走，互换只换两个键的位置，退回反而让用户在这些框里按错。

## Consequences

- 收益：习惯「符 在前」或别家数字键盘的用户可以在三端各改一处，随账号同步到 Android 和鸿蒙；默认文档、默认键盘逐键不变，老用户无感。
- 代价：多两项只对手机触屏有意义的共享字段，桌面宿主在 `preference_coverage.py` 里登记不读的理由；`platform_default` 让同一个默认值在三端画成三种样子，同步过去的是「各端自己的默认」而不是某一种具体排法。Android 空格不在右列时右列多出一个 ！；iOS 借来的数字页在带地球键的另两种排法下按份额排，不再与上方的列对齐（不带地球键时仍对齐）；鸿蒙「空格在右列」时右列的空格很窄，方案名可能显示不全。
- 服务端账号字段表要收录四个新键才会真正上传；收录时同样有[按键布局样式](2026-10-27-touch-key-layout-styles.md)「Consequences」里写的新键基线问题（第一次带本机改动上传的设备会用本机值盖掉云端），本次不改合并规则。

## Verification

- `rbuild cargo test -p msime-client-core -p msime-client-services` 通过：新增默认值与不改写旧文档、存储往返、线上写法与不认识的值，Android 同步写法与跳过、`platform_default` 应用回各端默认，以及导出键清单的 Android、鸿蒙两处快照。
- `python3 scripts/test-preferences-field-parity.py`、`python3 platforms/macos/tests/settings/preference_coverage.py .`、`python3 scripts/test-ios-key-layout-settings.py` 通过。
- Android：`rbuild` 上 `bash platforms/android/check-host.sh` 通过（298 项 JVM 冒烟、255 份设备测试源码编译），`TouchKeyStyleSmoke` 新增两组（互换的键序、宽度、各样式与各键面、借来的数字九宫格；三种空格位置的份额、借来的数字页、交换中/英、右侧大回车与字母层不受影响、设置文字），check-host 新增接线钉子；Gradle `compileFullDebugJavaWithJavac`（Studio）通过。
- iOS：Mac Studio 上经 `rbuild` 重编模拟器原生库，一次性 iPhone 17 Pro Max / iOS 27.0 模拟器跑 `KeyLayoutStyleTests` 36 项 0 失败，其中新增 7 项：两项偏好的读法和保存写法、底行表、真 `KeyboardViewController` 上互换后的 26 键底行（标准、逗号句号在空格两侧、英文，宽度与不互换时相同）和借来的数字九宫格（符 | 返回，符号栏不再有第二个 符）、拼音九键和全拼借来的数字页在三种空格位置下的底行键序、右列空格叠在 ！ 那一格上、回到字母层后底行空格回来、表情面板盖住按键时右列空格和各行一起藏起来（不画、不接触摸、VoiceOver 读不到，关掉面板再回来；把 `setKeyRowsCovered` 换回改之前的写法时这一项 3 处断言失败），以及热力图。`NineKeyKeyboardTests` 119 项里 `testNineKeyKeepsItsOwnBottomRow` 有两处宽度断言失败（0 宽 122 对 121、回车 62 对 61.46，按空格宽度推算单位时差了 1/3pt）；把五个 iOS 源文件换回基线提交 `c3db764c0c`（#10061 的分支）后在同一台 Studio 上单跑这一项，失败的数字完全相同，是基线分支上已有的问题，本分支没有改它。
- 鸿蒙与共享设置页：本机 `pnpm install --frozen-lockfile`（从本机 pnpm 仓库硬链接）后 `bash platforms/harmony/tests/run.sh` 0 失败（`touch-layout-styles.test.ts` 新增两组，`run.sh` 新增接线守卫和 `faceKey()` 里的两项）；`apps/desktop` 的 `tests/settings` 476 个文件 3060 项 vitest 通过，`tsc --noEmit` 通过。没有跑 `hvigorw assembleHap`，ArkTS 的编译只到 node 下的 `tsc`。
- 评审后补上 123 / #+= 层跟着互换和语音面板关闭后的读屏焦点：`rbuild` 上 `check-host.sh` 通过（`TouchKeyStyleSmoke` 新增 `symbolFirstMoves` 各键面与样式、互换后 123 层和 #+= 层底行的键序与份额）；`bash platforms/harmony/tests/run.sh` 0 失败（鸿蒙这处改在 `.ets` 里，`run.sh` 的 `tsc` 不编译它）；iOS 的 `KeyLayoutStyleTests` 新增英文 123 / #+= 层与长空格 123 层的断言，两个 Swift 文件本机只过了 `swiftc -parse`，模拟器上的运行交给 PR 的 iOS Simulator 任务。
- `pnpm run verify-notes` 的三个脚本通过。
- 没有在任何模拟器或真机上装包看过键盘：三端的真实画面、右列空格的手势（长按语音、拖动光标）和 ！ 的上屏、设置页到键盘的端到端路径都只到单元测试与编译这一级。

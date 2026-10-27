# Agent Note: 触屏按键布局样式与顶部数字行的共享偏好

Status: implemented

## Problem

手机触屏键盘的底行只有一种排法。Android 先后加了两个本机设置来满足不同习惯：[长空格布局](2026-10-27-android-long-space-layout.md)（`platform.android.long_space_layout`）和[数字行](2026-10-09-android-number-row-and-split-duplicates.md)（`platform.android.number_row`），都只存在 Android 本机、不随账号同步，iOS 与鸿蒙没有对应的选择。产品决定把「底行样式」按键面类型做成三端共享的选择，并把顶部数字行一起提成共享开关。

## Decision

共享偏好（`crates/client-core/src/preferences.rs` 的 `Preferences`）在触屏那组字段里新增四项，默认值就是现在的键盘：

- `touch_twenty_six_key_style: TouchTwentySixKeyStyle`，作用于全部中文 26 键字母层（全拼、26 键双拼、五笔 26 键、粤拼等）；英文 26 键、iPad 全尺寸键盘和鸿蒙 2in1 的整排键盘不受它影响。取值 `standard`（默认，123 | 符 | 空格 | ，长按。 | 中/英 | 回车）、`comma_period`（123 | 符 | ， | 空格 | 。 | 中/英 | 回车）、`long_space`（123 | ， | 长空格 | 中/英 | 回车，符号入口移到工具栏，接替 Android 的 `long_space_layout`）、`emoji`（123 | 😀 | 空格 | ， | 中/英 | 回车，符号经长按 123 进入）。
- `touch_fourteen_key_style: TouchFourteenKeyStyle`，14 键与 18 键字母层（全拼、双拼、五笔 14 键）共用。取值 `standard`（默认，123 | ， | 空格 | 。 | 中/英 | 回车）、`long_space`、`emoji`。
- `touch_nine_key_style: TouchNineKeyStyle`，九键字母层（拼音九键、九键双拼、笔画共用这个框架），除 `tall_return` 外左侧符号栏和右侧功能列不变。取值 `standard`（默认，符 | 123 | 空格 | 中/英 | 回车）、`comma_period`、`emoji`、`tall_return`（设置页名「右侧大回车」，排在表情键之后：左列是跨三行的符号栏，栏下方底行放与它同宽的 符；九宫格下方底行 123 |（🌐）| 空格 | 中/英 恰好与九宫格同宽；右列与现有右列同宽，自上而下 ⌫、各端标准九键右列原有的第二格（Android 重输，iOS 与鸿蒙的拼音九键和九键双拼是分词），回车跨第三行和底行、两倍高且中间无缝。现有右列的 ！ 去掉，只在这个样式里接到符号栏默认字母符号表末尾，自定义列表原样使用。九键数字层照同一框架，三端相同：符 留在原处，九宫格下方是 切换键 |（🌐）| 0 | 空格 | 中/英，空格是 1.05 份，0 占剩余宽度；右列是 ⌫、小数点（.）和两倍高的回车。数字层的空格仍是原来那个空格键，各端已有的手势（长按语音、拖动移光标）照旧。中列宽度三端同一规则，字母层和数字层都适用：123（数字层是切换键）、🌐、中/英 保持本端同一地球键状态下标准九键底行里的绝对宽度，剩下的宽度归字母层的空格，或数字层扣掉 1.05 份空格后的 0。中/英 与回车不在同一行，不适用回车让位和点击区规则。热力图照这个框架画，统计 id 不变。较旧的版本读到它按 `standard` 处理）。
- `touch_number_row: bool`，默认关，在 26 键、14 键和 18 键字母层上方画 1 到 0（九键、手写不画）；窗口太矮（如手机横屏）时由宿主按自己的高度门槛不画，存着的值不变。接替 Android 的 `platform.android.number_row`。

三个样式按键面类型分开存，而不是一个枚举套所有键面：每类键面能放的排法不同（14 键本来就有逗号和句号，没有 `comma_period`；九键没有地方放长空格），分开存时每个值都是这类键面真能画出来的样子。

### 文档与读取

- 四项等于默认值时都不写进文档（`skip_serializing_if`），默认文档与加字段之前逐字节相同，旧版本照样能读；缺这几个键的旧文档读出默认值，读完不改写文件。
- 三个枚举各有手写的宽松 `Deserialize`，做法与 `CandidateTranslationScope` 相同：不认识的字符串（较新版本加的样式）按 `standard` 读，整份文档照常读出；不是字符串照样报错。另有严格的 `parse` / `as_str`，同步与宿主用它们。
- 不往 `TouchKeyboardLayout`、`TouchShuangpinLayout`、`TouchKeyboardScheme` 里加成员：样式是同一键面的底行变化，不是新的键面或方案。

### 账号同步

四项随账号同步，走触屏宿主共用的 `TouchHostKeys`（`crates/client-services/src/account/settings_sync.rs`），与 `touch_quanpin_shift` 的[先例](2026-10-26-touch-quanpin-shift-preference.md)相同：

| 共享偏好 | Android 键 | 鸿蒙键 | 值 |
| --- | --- | --- | --- |
| `touch_twenty_six_key_style` | `platform.android.twenty_six_key_style` | `platform.harmony.twenty_six_key_style` | 字符串 |
| `touch_fourteen_key_style` | `platform.android.fourteen_key_style` | `platform.harmony.fourteen_key_style` | 字符串 |
| `touch_nine_key_style` | `platform.android.nine_key_style` | `platform.harmony.nine_key_style` | 字符串 |
| `touch_number_row` | `platform.android.number_row` | `platform.harmony.number_row` | 布尔 |

导出总是写这四个键（默认值也写，云端才能被改回默认）。应用时样式按严格写法解析，本机不认识的样式只跳过这一个键并列进 `skipped`，本机的选择不变。服务端字段表收录这些键之前，上传前的字段表过滤会去掉它们。`platform.android.number_row` 与 Android 本机设置文件里那个不同步的旧键同名：旧键不在 `ANDROID_LOCAL_SETTINGS` 里，宿主不会把它当本地同步值传进来，两者不冲突。iOS 的同步走 `ios_settings.rs` 的自有映射，`touch_quanpin_shift` 在那里也没有键，这次同样不加。

### 宿主

桌面宿主（macOS、Windows、Linux 与 `apps/desktop`）保留但不读这四项。macOS 的偏好覆盖门禁（`platforms/macos/tests/settings/preference_coverage.py`）为它们登记了不适用理由；`packages/ui/src/core/preferences-model.ts` 带上了类型，共享 React 设置页给鸿蒙手机提供了这四项的控件，所以字段对照（`scripts/test-preferences-field-parity.py`）不把它们列进 `RUST_ONLY`。Android、iOS 与鸿蒙的键面、设置入口、迁移和验证见下面的「各端实现」。

Android 把 `platform.android.long_space_layout = true` 迁成 `touch_twenty_six_key_style = long_space` 时，只迁中文 26 键的底行：英文 26 键回到标准底行，独立句点键回来，不再由逗号下滑接替（[长空格布局](2026-10-27-android-long-space-layout.md)里对英文键面的那项改动不随 `long_space` 保留）。要保留它需要另做决定，不能当成 `long_space` 的附带效果。

Android 的拼音九键（含九键双拼）右列 ！ 原本上滑输入 0、长按选上标 ⁰ 或下标 ₀，是九键字母层直接输入 0 的地方（九宫格长按只给 1–9）。`tall_return` 去掉这个 ！ 时，接到符号栏默认表末尾的那个 ！ 带着同样的手势和右上角的 0，自定义表里的 ！ 不带；笔画右列的 ！ 本来没有这个手势（网格里有 0 键），笔画不带。iOS 与鸿蒙的 ！ 没有这组手势，不涉及。Android 笔画的数字层借的是九键数字九宫格，`tall_return` 下按笔画外框的列宽（0.9 / 3 / 0.9）画，和笔画字母层同一个外框，切 123 / 笔 时 符、切换键和回车不横移。

### 设置页说明文字

三端设置页的说明逐字相同，改动时三端一起改，各端的测试逐字钉住：

- 键盘页「按键布局」入口副标题：「各类键盘的底行样式和顶部数字行」。入口只在本版本至少提供 26 键、14/18 键、9 键中的一组时列出。
- 26 键组末：「长空格：符号键移到工具栏；表情键：长按 123 打开符号。」
- 14 键与 18 键组末：「表情键：长按 123 打开符号。」
- 9 键组末：「右侧大回车：回车竖占两行，符号键在左下角。」
- 「顶部数字行」开关说明：「在 26 键、14 键和 18 键上方加一行 1 到 0；横屏时不显示。」各端的高度门槛写法不同（见「有意保留的差异」），这句只说用户实际会遇到的情况。

文字各端集中放：Android 是 `KeyLayoutStylesPage` 的静态常量（`TWENTY_SIX_KEY_NOTE`、`FOURTEEN_KEY_NOTE`、`NINE_KEY_NOTE`、`NUMBER_ROW_NOTE`），入口副标题写在 `KeyboardOptionsPage` 的入口行里，iOS 是 `TouchKeyLayoutOffering` 的常量（`entrySubtitle`、`twentySixKeyFooter`、`letterGridFooter`、`nineKeyFooter`、`numberRowSubtitle`），鸿蒙是 `packages/ui/src/keyboard/touch-key-layout-styles.ts` 导出的 `keyLayoutEntryDescription`、`twentySixKeyStyleNote`、`fourteenKeyStyleNote`、`nineKeyStyleNote`、`numberRowDescription`。

## 各端实现

三端都按上面四项画键盘，默认样式的底行与加样式之前逐键相同，各端的测试钉住这一点。下面是每端的接法、有意保留的差异，以及每端实际跑过什么、没验证什么。

### Android

底行由 `KeyboardActionRow.designEntries(..., TouchKeyStyle.Row)` 统一排布，`TouchKeyStyle.Styles.forLayout` 决定当前键面读哪一类样式。标准 26 键上的中文方案（全拼、双拼、五笔、粤拼，view 还没给出方案时按中文）读 26 键样式；英文、韩文和其他语言的 26 键画标准底行。14 键与 18 键读 14 键样式，拼音九键、九键双拼与笔画读九键样式，手写、注音和日文假名网格没有可选样式。密码框与纯数字框里三类样式一律画标准底行。份额不含地球键，地球键在 123 后占 1：

- 26 键：`comma_period` 是 123 1.25 | 符 1.05 | ， 1 | 空格 3 | 。 1 | 中 1.05 | 回车 1.9，逗号长按改为常用标点菜单；`long_space` 就是原来的长空格行（空格 5.05，符号入口在工具栏，逗号下滑输入句号，123 进带逗号的 123 层）；`emoji` 的 😀 占 符 的位置和 1.05 份，是常驻的表情图标键，记 `SoftEmoji`，点按打开表情面板，符号面板经长按 123 打开。
- 14/18 键：`long_space` 的空格是 5，逗号同样下滑输入句号（句号没有独立键之后的补位），长按仍是常用标点菜单；`emoji` 是 123 | 😀 1 | 空格 4 | ， 1 | 中 | 回车。
- 九键：`comma_period` 是 符 | 123 | ， 1 | 空格 2 | 。 1 | 中 | 回车，`emoji` 是 符 | 123 | 😀 1.05 | 空格 2.95 | 中 | 回车，这两种样式不改数字层。

右侧大回车（`tall_return`）用于拼音九键、九键双拼、笔画以及它们的数字层（含笔画借出的数字九宫格），判定在 `KeyboardActionRow.drawsTallReturn`；26 键等其他键面借用的数字九宫格不用。外框连同底行是一整块，高度角色是三行加底行，与标准外框总高相同；`designEntries` 对它返回空，功能行隐藏，与注音九键相同。底行各格用底行高度角色，九宫格加高时增量照旧只给上方。列宽沿用各自外框：九键 3 / 14 / 3，笔画 0.9 / 3 / 0.9（`StrokeKeyboardLayout.SIDE_COLUMN_WEIGHT`、`GRID_WEIGHT`）。

- 左列：跨三行的符号栏，栏下是与左列同宽的 符；数字层照旧在符号栏下方保留 #+=。默认字母符号表末尾接上 ！（`NineKeySidebarPolicy.letterSymbols(stored, true)`），与默认表相同的存储也按默认算，因为「恢复默认」会把默认表写回存储；自定义表原样使用。笔画的符号栏在这个样式下一屏最多五项，！ 不用滚动。
- 中列：九宫格正下方是 `KeyboardActionRow.tallReturnEntries`，恰好与九宫格同宽。123 1.25、地球键 1、中/英 1.05 保持标准九键底行里的绝对宽度，空格占剩下的：拼音九键 4.175，带地球键 3.875，笔画 3.48125。交换中/英与 123 时两键对调。
- 右列：⌫、重输、回车。回车格从第三行顶边画到底行底边，中间无缝；⌫ 和重输先包进没有外边距的格子再与回车格分高度，否则键距外边距先被扣掉，两键会比九宫格的行矮。九键双拼的网格是四行，⌫ 照旧占两格、重输一格，回车从第四行起，约 1.75 行高。
- 数字层：符 和切换键横向位置不变，0 在空格的位置，右边是与中/英同宽（1.05，`KeyboardActionRow.TALL_RETURN_DIGIT_SPACE_WEIGHT`）的常驻空格键，长按语音和拖动光标跟着它走，0 占剩下的宽度；小数点（按字面上屏）在右列重输的位置，回车仍两倍高。三端这一层排法相同。
- 中/英与回车不在同一行，回车不让出点击区。
- 换样式、进出密码框或地球键变化时，`ImeLayoutRows.tallReturnStale` 让下一次渲染重建键行。建外框时当场把功能行按空排布重排：偏好重读换了键面时只重建键行、不一定接着渲染，否则会留下上一个键面的功能行。外框里的常驻键经 `ImeBottomRow.placeKey` 挂入，与功能行同样上色。

顶部数字行读 `touch_number_row`，画在 26 键（含英文）、韩文、14 键和 18 键的字母层上方（14/18 键整行铺满，包括左侧拼音栏的位置），窗口低于 480 dp（`KeyboardLayout.NUMBER_ROW_MIN_WINDOW_HEIGHT_DP`）时不画；这些键面的 123 层保留补充的第五行，切层时高度不跳。密码框和 SSH 应用的自动数字行照旧，与共享开关取或。样式和数字行进了 `touchGeometryKey`，偏好一变键盘就地重排底行、按需重建键行，不重启进程，也不打断组字。四个偏好键记在键盘进程的皮肤片段（`SKIN_HINT_KEYS`）里，`:ime` 冷启动的第一帧就按用户的样式和数字行画；check-host 钉住 `applyTouchGeometry` 读到的每个样式键都在片段里。唯一的例外是从带本机数字行的旧版本升级后的第一次冷启动：旧片段里没有这几个键，那一帧仍按默认画，迁移后的实时偏好到了才补上。

迁移（`KeyLayoutStyleMigration`）：本机 `platform.android.long_space_layout` 为真且共享的 26 键样式读出来是标准（缺键、`standard` 或不认识的写法）时写 `long_space`；本机 `platform.android.number_row` 为真且共享数字行为关时写开。写入按修订号保存，被拒就重读再算，最多三次；真改了才记成本机改动（`SyncSignals.markDirty`）。写好之后删掉两个本机键，失败时保留，下次再迁。迁移在这些地方先跑：云同步每一轮导出之前（`RustCloudSync.run`）、键盘建会话读实时偏好之前、本地备份导出之前、本地备份恢复写回本地设置之后，以及键盘设置页和按键布局子页读取时。云同步那一轮迁移没成功时，`RustCloudSync.run` 在 `host_inputs` 里带 `settings_locked: true`：设置（连同皮肤库）这一轮既不导出也不应用，待上传标记保留，常用语和词库照常同步，下一轮再迁，不会把共享偏好里的默认值当成用户的选择传上去。本地备份不需要这样处理：迁移没完成时旧键还在备份的 android-local 里，恢复时会再迁。两个旧键的常量仍保留声明，否则读文件时会被丢掉；check-host 钉住只有迁移类引用它们。

设置与热力图：键盘页「布局」组的「按键布局」子页有 26 键、14 键与 18 键、9 键三组单选，每个选项下面画这种样式底行的预览，9 键第四项「右侧大回车」的预览画外框最后两行、回车跨两行；最后是放在无标题组里的「顶部数字行」开关，本版本既没有 26 键也没有 14/18 键时不列，与鸿蒙同一规则。本版本不提供的键面不列对应的组；原来的「长空格布局」和「数字行」两个开关已删除。各版本有哪几组集中在纯 Java 的 `TouchKeyStyle.Groups.offeredBy(AppEdition)`，子页、键盘页和设置搜索共用，TouchKeyStyleSmoke 的 `groups()` 逐版本钉住。键盘页的「按键布局」入口只在本版本至少有一组时列出，与鸿蒙 `screen-keyboard-page.tsx` 的 `keyLayoutOffered` 同一条规则；日语、越南语、藏语版不列入口，设置搜索（`KeyboardFragment.applySearch`）也不列 `KEY_LAYOUT_STYLES`，免得打开一页空白。子页三组卡片下方各有一行说明（`GroupCard.footer`），文字和顶部数字行开关说明、入口副标题见下面的「设置页说明文字」，check-host 钉住这些文字和接线。统计页热力图按当前的 26 键、九键样式画，26 键打开数字行时多一行 `Digit1`…`Digit0`；右侧大回车由 `KeyHeatmapLayout.nine(TALL_RETURN)` 画，`Key` 的 `rows` 让回车跨两行，统计 id 不变。`platforms/android/README.md` 的「数字行」与「按键布局」两节描述了这些。

验证：

- 本机 JVM 层：`check-host.sh` 通过，289 项 JVM 冒烟、248 份设备测试源码编译。TouchKeyStyleSmoke 钉住每种样式的键序和宽度、整行总宽不变、偏好的宽松读取、各键面读哪类样式、数字行门槛、迁移规则，以及右侧大回车两种外框的列宽、中列各键的份额和字母层与数字层外侧键对齐；NineKeySidebarPolicySmoke 钉住 ！ 的接法和带上滑 0 的那一项；KeyHeatmapLayoutSmoke 钉住热力图外框。第一、二轮新加的 check-host 钉子在临时副本上改坏代码确认会失败，第二轮修复时新加的钉子没有做这一步。
- 构建：在 Mac Studio 上经 `rbuild` 用本分支重编原生库，打出 arm64 APK 和设备测试包，打包时带上语言词库（不带时笔画词库缺失，键盘切不到笔画）。
- API 35 arm64 模拟器：在设置子页逐个切换 26、14、9 键的样式和数字行后回到试用键盘截图核对，逗号句号样式实际打出「，。」，表情键打开表情面板，数字行打开时 123 层不跳高，热力图跟着样式变。冷启动录屏对照：片段里有这几个键时杀掉 `:ime` 后第一帧就有数字行和 😀，删掉后第一帧没有、下一帧才高出一行。最终那次全新安装里 NumberRowDeviceSmoke（含 AutomaticNumberRowDeviceSmoke）、KeyboardHeightDeviceSmoke（含 PortraitSplitDeviceChecks）、ToolbarEdgesDeviceSmoke 和 NineKeyNumericLayoutDeviceSmoke 都通过。后者带的 TallReturnDeviceChecks 量真实键帽：拼音九键字母层与数字层、侧栏 ！ 上滑出 0 与长按出 ⁰、点回车下半段在多行框里换行、数字层 1 0 . 空格 上屏 `10. `、换回标准样式后功能行与右列 ！ 回来，以及九键双拼和笔画的字母层与数字层。把笔画数字层的列宽改回九键列宽的对照包在笔画阶段如期失败（符 与切换键横移 40 px）。
- 数字层改成三端一致的 0 | 空格、小数点进右列之后：`check-host.sh` 通过（289 项 JVM 冒烟、248 份设备测试源码编译）；TouchKeyStyleSmoke 钉住数字层的键序、空格 1.05 和切换键、地球键、中/英在两层里的绝对宽度，KeyHeatmapLayoutSmoke 钉住右列小数点的接线和预览里长空格样式写「空格」，三处在本机改坏产品代码后都如期失败。经 `rbuild` 重打 APK 后在新建的 API 35 arm64 模拟器上跑 NineKeyNumericLayoutDeviceSmoke 通过，其中 TallReturnDeviceChecks 量到数字层 切换键 | 0 | 空格 | 中、空格与中同宽、小数点在右列 ⌫ 与回车之间，数字层 1 0 . 空格 上屏 `10. `，换回标准样式后标准底行的 123 和中与大回车外框里的同宽。设备测试的新断言没有拿旧包对照跑过。
- NumberRowDeviceSmoke 与 PortraitSplitDeviceChecks 曾在 #+= 阶段失败，原因是 develop 上 #9925 把切回字母的说明改成「返回原键盘」、英文 123 键面改成「返回」，测试还在找旧名字；用 develop 合并基点打的包同样失败，本分支只改了这两套测试的标签。另有 10 份设备测试仍用旧标签，属于 develop 上要另修的问题。
- 第四轮（入口按版本显示、说明三端统一）：`check-host.sh` 通过（289 项 JVM 冒烟，含按键布局和 SettingsSearchIndexSmoke，设备测试源码编译）；新加的 check-host 钉子在真代码上通过，副标题改字、入口条件换成 `if (true)`、去掉组末说明三种改坏都如期失败。check-host 不编 home/ 下的页面，所以经 `rbuild` 在 Mac Studio 上用 Gradle 编了 full 与 japanese 两个 flavor 的 Debug Java，通过。没上模拟器，组末说明和日语版不列入口都没在设备上看过。
- 没验证：真机，平板和分离式键盘（含右侧大回车），旧键迁移与迁移失败时锁住设置同步（发布包不可调试，没法预先写入本地设置文件或让迁移失败，只有 JVM 冒烟、check-host 钉子和编译），长空格逗号下滑的真实手势，横屏门槛，账号同步往返。偏好重读换了键面却没接着渲染时，标准九键与注音外框可能在下一次按键前还留着上一个键面的功能行；这是本次之前就有的行为，右侧大回车外框自己处理了，其他外框没改也没验证。

### iOS

偏好由 `platforms/ios/SharedUI/preferences/TouchKeyLayoutStylePreference.swift` 的 `TouchKeyLayoutStyle` 读写，做法与 `TouchShuangpinLayoutPreference` 相同：共享文档是权威，App Group 里的 `keyboard.style.twentySixKey`、`keyboard.style.fourteenKey`、`keyboard.style.nineKey`、`keyboard.numberRow` 只是镜像。键盘在 `viewDidLoad` 和每次重载文档（`synchronizeSharedTouchPreferences`）时刷新镜像，样式和数字行门槛都在 `KeyboardLayoutInputs` 里，变了就当场重排，不碰会话，正在组的字照常继续。缺键、不是字符串或不认识的写法按 `standard` 读；设置页每次只写刚改的一项，文档接受了才写镜像。iOS 以前没有对应的本机设置，没有旧值要迁移。iOS 的账号同步走 `ios_settings.rs` 的自有映射，这四项不在里面，与 `touch_quanpin_shift` 一样暂不随账号同步。

各样式的键序和份额只有一张表 `TouchBottomRowLayout`，键盘、统计热力图和设置页预览共用。标准样式仍走原来的约束，默认底行与以前完全相同；其他样式按表、以空格为基准加宽度约束。地球键跟在 123 后面占 1 份；本机「隐藏中/英键」打开时中/英的 1.05 份并进空格；九键双拼字母层的 0 排在空格后面。

适用范围：

- 26 键样式只作用于手机的中文 26 键：全拼、双拼、五笔 26 键、粤拼。日文罗马字、韩文、越南文、藏文、英文和本地模式保留标准底行，与 Android、鸿蒙一致。
- 密码框（安全输入，或内容类型为 password / newPassword）和只收数字的框（numberPad、decimalPad、asciiCapableNumberPad、phonePad）里三类样式都退回标准底行（`KeyboardViewController.keepsStandardBottomRows`），顶部数字行照画；每次换输入框都重新判定。
- iPad 没开「数字行与 Tab 键」时与手机一样画顶部数字行。开着时 iPad 全尺寸 26 键画自己的数字行和 Tab，14、18 键仍按顶部数字行开关（这一轮起 14/18 键在这种情况下也画数字行，以前不画）。14/18 键和九键的样式在 iPad 上同样生效；iPad 26 键的底行是它自己的。

逗号与长空格：

- 26 键 `comma_period`：逗号长按打开常用标点，句号是独立键，没有长按。
- `long_space` 的字母层（26、14、18 键）逗号下滑输入句号，自定义了常用标点也一样；长按在 26 键默认标点下是句号，14/18 键和自定义了常用标点时打开常用标点菜单（`holdsCornerHint`）。26 键的 符 移到工具栏品牌键后面的「符」按钮，工具栏设成隐藏时这一行仍显示，只放品牌、符号和收起；123 进带空格的 123 层，不借数字九宫格，123 层和 #+= 层原来的表情或符号入口换成逗号。
- 26 键 `emoji` 复用 123 层的表情键（记 `SoftEmoji`），符号面板经长按 123 打开。九键「底栏带标点」的逗号跟随标点键面，长按打开常用标点。

右侧大回车用于拼音九键、九键双拼和笔画的字母层与数字层；26/14/18 键和手写借来的数字九宫格不跟。

- 左列：符号栏（宽度仍是 14%）跨三行，标点是 ，。？！，！ 是只在这个样式里出现的新按钮（iOS 没有自定义符号栏列表）；栏下底行是与它同宽的 符。
- 中列：九宫格下方是 123 |（🌐）| 空格 |（九键双拼的 0）| 中/英，左右与九宫格对齐。123、🌐、中/英 与同一地球键状态下拼音九键的标准底行同宽，0 是 1.05 份，空格占剩下的部分；表里写作 符 和回车各 1.281（9.15 × 14%），总份额仍是 9.15，带地球键时与标准底行一样按 10.15 份算，符 和回车各 1.421（`TouchBottomRowLayout.nineKeyTallReturn`、`nineKeyRowWeight(globe:)`）。第一版带地球键时仍按 9.15 份算，123 和中/英比标准底行宽了约 11%。九键双拼的标准底行另加了 0、整行 10.2 份，右侧大回车仍按拼音九键的份额算。这些宽度约束都是必需优先级：用 999 时会与分离键盘里隐藏的第二个空格的 999 约束打平，切过数字层后空格和中/英宽度会错。中列的绝对宽度还要扣键距：标准九键底行是 stack view 先从行宽里扣掉键数减一道键距、再按份额分，第二版只对份额（行宽 × 份额 / 总份额），键距 6pt 时 123 比标准底行宽约 3.3pt、中/英 约 3pt。现在 `tallReturnActionWidths` 以底行宽度为基准，常数扣掉拼音九键标准底行（符 | 123 |（🌐）| 空格 |（中/英）| 回车，`TouchBottomRowLayout.nineKeyStandardKeyCount`）的键距总和，按行和键距缓存（`TallReturnWidthsKey`）；于是 123、🌐、中/英 与同一地球键和中/英状态下的标准底行逐点等宽，0 和数字层的空格与标准底行的 符 等宽。常数扣的是建约束时的键距，所以凡是改了键距的路径都要重排底行：键盘里调完高度点「完成」、共享文档没写成时，`commitTouchKeyboardGeometry` 按文档对齐 App Group 后接着调用 `updateKeyboardLayoutIfNeeded`（几何在 `layoutInputs` 里），与重载共享偏好的路径相同；否则键距从 6pt 变成 3pt 时 123、🌐、中/英 会比标准九键底行窄约 1.6pt，直到下一次重排。
- 右列：沿用 iOS 现有的 ⌫ 和中间键（拼音是「分词」，笔画是「重输」，数字层是「.」）。回车 `tallReturnKey` 是叠在按键列上的独立键，从第三格顶边伸到底行底边；右列原来的 ！ 留着占位，但不画、不接触摸、不进无障碍，底行原来的回车隐藏。回车不在任何一行里，所以经 `KeyAreaStackView.overlayKeys` 加入键距分配，相邻键距里靠它的一半归它；其他样式下它隐藏、不参与。
- 数字层：符 在符号栏下方（栏里自己的 符 隐藏，运算符占满），123 的位置放回到字母层的键（九键，笔画是「笔」），0 占空格的位置，空格在 0 右边、与中/英同宽，回车仍两倍高。
- 中/英与回车不在同一行，回车不让点击区。

顶部数字行画在 26 键（含英文和本地模式）、14、18 键的字母层，键面 1 到 0，记 `Digit1`…`Digit0`，走 `handleSymbol`，组字中 1–9 由 runtime 选词。手机横屏不画，存着的值不变。数字页和符号页显示补充行（`symbolExtraRow`）保持五排高度。键盘整整高一排（竖屏默认 289 → 348pt），26 键上下留白按排数算（`phoneLetterVerticalPadding(rows:)`），字母键仍是 44pt；留白在同一趟排版里数字行和补充行的显隐定下来之后才算，当场开关数字行、横屏转回竖屏、从九键换到 26 键时字母键都不会变成 43pt 或 45pt。

设置与热力图：「键盘」页「中文键盘 / 双拼键盘 / 五笔键盘」之后的「按键布局」行，进去是三组单选，每项带底行缩略图（右侧大回车画两排），最后是顶部数字行开关。本版本不提供的键面类型不列它那一组（`TouchKeyLayoutOffering`）：26 键只看中文 26 键（全拼、双拼、五笔、粤拼），日文罗马字、韩文、越南文和藏文不算；既没有中文 26 键也没有 14/18 键时不列顶部数字行开关，与 Android、鸿蒙同一规则。键盘页的「按键布局」入口（连同它上面的分隔线）只在本版本至少提供一组时显示（`TouchKeyLayoutOffering.offersAnyGroup`）。入口副标题、三组组末说明（14 键与 18 键组以前没有说明，这次补上）和顶部数字行开关说明见上面的「设置页说明文字」，原来与 Android、鸿蒙不同的写法已换掉。统计热力图读文档里的样式画 26 键和九键底行，数字行打开时多一排数字，右侧大回车画成跨两行的换行，侧栏的 ！ 是唯一的 `SoftPunctuation` 格；画在键盘上的键不再列进「其他键」。

验证（模拟器级）：

- 在 Mac Studio 上经 `rbuild` 用本分支重编模拟器原生库，`MSIMEApp` Debug 与 `MSIMEClientTests` 编译通过。测试宿主带上锁定版本的粤语、注音、笔画词库，笔画用例实际跑了。
- 一次性 iPhone 17 / iOS 27.0 模拟器：`MSIMEKeyboardTests` 1013 项，1 项跳过，0 失败。其中 `KeyLayoutStyleTests` 23 项用真 `KeyboardViewController` 逐样式量底行键序和宽度（显示和隐藏中/英各量一遍），覆盖数字行门槛与高度、44pt 字母键、长空格工具栏入口与 123 层逗号、组字中改样式、中文 26 键范围、密码框与数字框退回、iPad 数字行、右侧大回车的外框与数字层（含九键双拼和笔画）、按下大回车结束组字、侧栏 ！、键距分配和热力图；`KeyGapRoutingTests` 12 项；`MSIMESharedTests/TypingStatisticsTests` 14 项。把 `root.overlayKeys = ...` 那一行注掉后，键距用例在大回车一侧的三个点上如期失败（显示和隐藏中/英共 6 处）。
- 设置页：本机模拟器装真 App，经 `msime://settings/layout` 进按键布局，选「长空格」「右侧大回车」后保存，显示勾选和两排预览。
- 带地球键的中列宽度和设置页分组改过之后：`KeyLayoutStyleTests` 25 项在一次性 iPhone 17 / iOS 27.0 模拟器上 0 失败，`MSIMEApp` Debug 编译通过。测试里的键盘（手机和 iPad）都不带地球键，带地球键时的宽度只由底行表的用例（`testTallReturnMiddleKeysKeepTheStandardNineKeyWidths`）钉住，没在画出来的键盘上量过；设置页按版本分组只测了 `TouchKeyLayoutOffering`，没在单一语言版本的 App 里看过页面。
- 第四轮（中列扣键距、入口按版本显示、说明三端统一）：Mac Studio 上经 `rbuild`，一次性 iPhone 17 / iOS 27.0 模拟器，`MSIMEApp` Debug 编译通过，`KeyLayoutStyleTests` 26 项 0 失败。右侧大回车的用例改成在同一个键盘上先量标准样式的底行、当场换成右侧大回车再比（手机显示和隐藏中/英、九键双拼按拼音九键比、笔画、数字层、iPad），容差一个像素（1/3pt，iPad 的 中/英 量到 113.667 对 114.0，是 3x 取整）。把控制器换回上一版后这三个用例如期失败（123 54.33 对 51.0，中/英 45.67 对 42.67，iPad 138.67 对 135.33）。设置文字由 `testSettingsTextsMatchTheOtherPlatforms` 和 `scripts/test-ios-key-layout-settings.py`（入口在 `offersAnyGroup` 条件里、副标题和各组说明取自共享常量）钉住。
- 第四轮的键距修正：新用例 `testTallReturnFollowsTheKeySpacingTheFailedGeometryCommitFallsBackTo` 先量 3pt 键距下的标准九键底行，再在 6pt 下换成右侧大回车，只把共享文档改成 3pt 并用 `chflags UF_IMMUTABLE` 让 preferences.json 写不进去，点 键盘高度 → 完成 后核对键距回到 3、大回车外框与 3pt 的标准宽度一致。修正前单跑失败（123 51.0 对 52.67，中/英 42.67 对 44.33），修正后 `KeyLayoutStyleTests` 27 项加 `MorePanelFoldTests` 的两项键盘高度用例共 29 项 0 失败。
- 第四轮没验证：带地球键时在画出来的键盘上量宽度（测试宿主不带地球键），单一语言版本里没有入口的样子，真机。
- 没验证：真机，把键盘装成系统键盘在别的 App 里打字，真实 iPad 和真实旋转（横屏经 traitOverrides 模拟），真实的逗号下滑与长按手势（只断言配置），真实的安全输入框和数字键盘框（输入框类型经 `applyInputContext` 驱动），热力图界面（模拟器上统计页要设备密码，只核对了数据）。大回车记 `Return` 只核对了热力图表和按键闭包：单测宿主里 `countKeyPress` 的隐私门不放行。

### 鸿蒙

键序与份额在 `platforms/harmony/entry/src/main/ets/keyboard/TouchLayoutStylePolicy.ts`（`designRow`、`commaHold`、`commaSwipesPeriod`、`fieldRowStyle`、`numberRowExtraRows`、`symbolExtraRow`、`firstPanel`）和 `NineKeyLayout.actionRow(digitLayer, english, style)`；`KeyboardView.designActionRow` 与 `nineKeyActionRow` 按这些表画，默认样式与之前逐键相同。鸿蒙没有地球键。中文 26 键指字母层是 26 键一类、不是 14/18 键、且是中文（不是英文、韩文、越南语、藏文、日语罗马字）的键面；英文 26 键、大千注音和 2in1 保持原来的底行。

- 26 键：`comma_period` 与 Android 份额相同，逗号长按打开常用标点；`long_space` 是 123 | ， | 空格 5.05 | 中 | 回车，逗号下滑或长按输入句号，符号入口移到工具栏标志右边（「符」），工具栏这一行一直画着、设成隐藏时只留标志、符号和收起，123 进带空格的数字符号层，该层与 #+= 层左下的 😀/符号 换成逗号；`emoji` 是 123 | 😀 1.05 | 空格 4 | ， | 中 | 回车，😀 记 `SoftEmoji`，符号面板经长按 123 进入。
- 14/18 键：`long_space` 是 123 | ， | 空格 5 | 中 | 回车，逗号下滑输入句号、长按仍打开常用标点；`emoji` 是 123 | 😀 1 | 空格 4 | ， 1 | 中 | 回车，逗号长按打开常用标点。
- 九键：`comma_period` 是 符 | 123 | ， 1 | 空格 2.2 | 。 1 | 中 | 回车，逗号长按打开常用标点（`NineKeyLayout.commaOpensQuickPunctuation`）；`emoji` 是 符 | 123 | 😀 1.05 | 空格 3.15 | 中 | 回车；英文九键不用九键样式。
- 密码框和只收数字的框（数字、带小数点数字、电话号码，`EditorTraits.digits`）里，三类键面的所有样式都退回标准底行（`EditorPolicy.keepsStandardRows`、`TouchLayoutStylePolicy.fieldRowStyle`），这个状态进 `faceKey()`。

右侧大回车由 `NineKeyLayout.tallReturn(english, style)` 决定画不画，`KeyboardView.tallReturnNineKeyFrame` 画外框，只在触屏键盘上、拼音九键 / 九键双拼 / 笔画及其数字层使用；英文九键、假名网格和其他中文键面借用的数字九宫格保持原样。三列沿用触屏网格原有的份额：左右各 1.15、中间 6（`NineKeyLayout.SIDE_SHARE`、`GRID_SHARE`），底行与上面三列对齐。

- 左列：跨三行的符号栏，平时是 ，。？！（`NineKeyLayout.sideMarks(true)` 只在这个样式里接上 ！；鸿蒙没有自定义符号栏列表），组字时是读音栏；栏下是同宽的 符。
- 中列：九宫格下方恰好与九宫格同宽：123 | 空格 | 中/英，123 和 中/英 与标准九键底行同宽，空格占剩下的部分（`NineKeyLayout.tallReturnRow`）。标准底行五个键、四道键距铺满整块键面，外框三列两道键距、中列再有自己的键距，键距不成比例，所以份额定不准：外框量到自己的宽度（`tallReturnFaceWidth`）后给 123 和 中/英 定 vp 宽度，空格用 `layoutWeight` 拿剩下的；量到之前、以及外框消失后（宽度清零，免得在别的键面上转屏后再切回来，第一帧拿旧宽度定出比网格还宽的固定键）按 `NineKeyLayout.actionRow` 的份额排（按没有键距换算，123 约 1.13、空格约 3.91、中/英 约 0.95，共 6），设置页示意图和热力图也用这一份。
- 右列：⌫，其下是鸿蒙原有的第二格「分词」，回车占一个网格行加行距加底行，两倍高、中间无缝；原来的 ！ 格去掉。右列与网格逐行对齐：一格的高度按这块网格自己的行数算（`NineKeyLayout.tallReturnGridRowHeight`，视图里 `tallReturnGridRows` 对笔画字母层取 `StrokeLayout.ROWS.length`，拼音九键、九键双拼和数字层取三行），回车上方每个网格行一个键。笔画网格只有两行、右列本来没有第二格，所以只有 ⌫ 占第一行，回车跨第二行和底行，与拼音九键回车跨第三行和底行同一个规则。第四轮之前右列按拼音九键的三行切，⌫ 占三行里的两行加一道行距，回车从笔画第二行中间开始。
- 数字层：左列 + − * / #+=，符 留在原处；九宫格下方是 切换键 | 0 | 空格 | 中/英，切换键和 中/英 与标准底行同宽，空格是标准底行的 1.05 份（`NineKeyLayout.TALL_RETURN_DIGIT_SPACE_WEIGHT`），0 占剩下的宽度（`NineKeyLayout.tallReturnFill`）；空格照旧接语音长按和拖动移光标；右列是 ⌫、小数点和两倍高的回车。
- 回车不让出点击区（`NineKeyLayout.languageBesideReturn` 对这一行为假），其他九键行照旧让。

顶部数字行：开关打开、字母层是 26/14/18 键一类（九键、假名、手写、笔画、大千注音都不画）、不是 2in1、也不是手机横屏（`KeyboardFormFactorPolicy.phoneLandscape`）时，在字母上方画 1 到 0，平板也画。行高是不带高度调整的 44vp，面板经 `KeyboardMetrics.totalHeightVp` 的 extraRows 正好高一行；同一键面的 123、#+= 层和借用的数字九宫格顶部画同高的补充行，切层时面板高度不变。数字记 `Digit0`–`Digit9`；组字中经 `KeyboardSession.pressElseCommitThenType` 先交给引擎（当前页 1–9 选词，韩语汉字列表打开时选汉字），收没收只看引擎的 handled，不收时先结束组合再上屏数字，与 Android `DeclinedKeyPolicy` 相同。韩语、越南语、藏文的引擎会先上屏音节或单词再把数字交回宿主，按「组字变了」当作收下会丢数字；`tests/run.sh` 钉住这条接线。

偏好与第一帧：`KeyboardSession` 读四个字段，样式宽松解析，`faceKey()` 含四个值和输入框状态，设置页保存后 `onViewPreferencesReloaded` 当场重建键并重定面板高度，不打断组字；鸿蒙没有本机旧设置要迁移。视图把键面里决定面板高度的两件事（字母层是不是 26/14/18 键一类、是不是中文 26 键）记进键盘自己的 `touch_panel_face.json`，变了才重写；扩展能力挂视图之前用它和当前偏好算出第一帧的数字行与长空格工具条（`TouchLayoutStylePolicy.firstPanel`），视图出现时从同一个形状开始核对高度，记错了也会被纠正。视图在扩展能力挂上尺寸回调之前请求的高度由 `TouchHeightRelay` 暂存，挂上时补发。没有记录时与以前相同；手写多出的一行和一打开就在密码框里的面板仍在视图出现后纠正。

设置与热力图：共享 React 页在鸿蒙手机「屏幕键盘 › 布局」里、中文键盘之后加「按键布局」入口，子页三组单选各带示意图（数据在 `packages/ui/src/keyboard/touch-key-layout-styles.ts`，测试直接对照鸿蒙键盘的样式表），9 键第四项「右侧大回车」画两行、回车跨两行；「顶部数字行」开关在无标题组里，版本既没有 26 键也没有 14 键时不列。版本没有的键盘类型不列它那一组（26 键看全拼/双拼/五笔/粤拼，14/18 键看全拼/双拼/五笔，9 键看全拼/双拼/笔画）；一组都没有时（`keyLayoutOffered` 为假）不列入口、不挂子页内容，设置搜索也扫不到。入口副标题、组末说明和顶部数字行开关说明见上面的「设置页说明文字」；组末说明用 `SettingsGroupNote` 画在每组单选之后，并作为这组单选的读屏描述（`aria-describedby`），`apps/desktop` 的 `touch-key-layout-settings.test.tsx` 逐字钉住五处文字。设置项搜索能找到子页里的选项并打开子页。打字统计热力图按当前 26 键与 9 键样式换底行，数字行打开时在 26 键上方画 1 到 0；右侧大回车由 `phoneHeatmapLayoutRows` 按九键加这个样式画，回车 `rowSpan` 为 2、下伸的位置是占位，统计 id 不变。

验证：

- 源码与单测：`bash platforms/harmony/tests/run.sh` 0 失败。`touch-layout-styles.test.ts` 覆盖样式表与默认行不变、逗号规则、数字行门槛与补充行、输入框退回、右侧大回车的行与份额和不让位、第一帧形状，以及 `TouchHeightRelay` 在视图出现的三种时机下都把面板定到较高的高度；`run.sh` 的接线守卫逐一改坏被守的代码（九处）确认会失败。`apps/desktop` 的 `tsc --noEmit` 与全量 vitest（867 个文件、5569 项）通过；所有 `scripts/test-harmony-*.py` 通过。
- 编译：本机 `hvigorw assembleHap` 成功，CompileArkTS 实际执行。本分支的原生库在 Mac Studio 上经 `rbuild` 编出；main 检出里 Oct 25 的预编译库不能用，它的 client-core 没有这四个字段，设置页保存时会丢掉新键。
- HarmonyOS 6.0.2 手机模拟器：设置子页的选项、示意图和保存；键盘侧看了右侧大回车的字母层与数字层、输入数字、读音栏、点回车下半段上屏，九键「底栏带标点」的逗号长按菜单，26 键数字行，标准九键不变，以及冷启动第一帧按记下的键面定高（两个方向）。日志里看到了视图先于回调请求高度的情况，补发后面板到了正确高度，26 键带数字行没有被截。模拟器没有共享沙箱，设置页的改动传不到键盘，键盘侧是临时在代码里写死样式和数字行看的，这些改动已还原、没有提交。
- 本地模拟器要临时把 `~/sdk` 链到 `~/command-line-tools/sdk`（实例配置 `hardware-qemu.ini` 里的 sdk 根目录不存在，找不到 hdc），主机负载高时还要在启动期间临时挪开崩溃服务（它约 21 秒的 snapshot 启动检查连 `-bootmode coldboot` 也会杀掉）；两处用完都已复原。
- 没验证：设置到键盘的端到端路径（需要带 data-group-id 签名的真机）、密码框和数字框在真实输入框里退回标准底行、笔画的右侧大回车外框、逐帧确认第一帧不再跳高、真机。
- 第三轮（右侧大回车中列改成与标准底行同宽、数字层 切换键 | 0 | 空格 | 中/英）：`bash platforms/harmony/tests/run.sh` 0 失败，`touch-layout-styles.test.ts` 用 390/6、360/4 和没有键距三组宽度钉住 123、中/英 与标准底行同宽、数字层空格是 1.05 份、这一段加键距正好是网格宽度，`run.sh` 新守卫（外框量宽度、四个键按 `nineKeyActionWidth` / `nineKeyActionShare` 定宽、0 排在空格左边）逐条改坏确认会失败；`apps/desktop` 里设置页与热力图的三个测试文件通过；`stage-settings.sh` 后本机 `hvigorw assembleHap` 成功，CompileArkTS 实际执行。这一轮没有上模拟器看键帽宽度。外框消失时把量到的宽度清零（`onDisAppear`），`run.sh` 守卫加了一条，删掉清零时守卫会失败；改后 `run.sh` 0 失败，本机 `hvigorw assembleHap` 成功。
- 第四轮（笔画右列按两行对齐、设置页说明）：`bash platforms/harmony/tests/run.sh` 0 失败。`touch-layout-styles.test.ts` 按两行和三行核对右列一格与网格一行同高、回车顶边与网格最后一行顶边对齐、回车高度是最后一行加行距加底行，并断言笔画不再是旧的三行切法；`run.sh` 钉住视图按网格行数取高度、笔画分支不再写 `* 2 + rowSpacing`，把笔画 ⌫ 的高度改回旧写法时新守卫如期报错。`apps/desktop` 的 `touch-key-layout-settings` 与 `typing-statistics-keys` 两个 vitest 文件 28 项通过，`tsc --noEmit` 通过；`stage-settings.sh` 后本机 `hvigorw assembleHap --no-daemon` 成功（CompileArkTS 无错误）。这一轮没上模拟器，笔画右侧大回车外框仍没在画出来的键盘上看过。

### 有意保留的差异

下面这些三端不一样，都来自本次之前各端已有的键或规矩，本次没有拉齐，要改需要另做产品决定。右侧大回车的数字层排法和中列宽度规则三端已经一致（见 Decision），不在这里：

- 右列第二格沿用各端标准九键右列原有的那一格：Android 是重输；iOS 拼音九键和九键双拼是分词（重输在 ⌫ 长按），笔画是重输；鸿蒙拼音九键和九键双拼是分词，笔画没有第二格：笔画网格只有两行，⌫ 占第一行，回车跨第二行和底行。
- Android 九键双拼的网格是四行，右侧大回车里 ⌫ 照旧占两格、重输一格，回车从第四行起，约 1.75 行高。
- 符号栏 ！ 的上滑 0 与长按上下标只有 Android 拼音九键和九键双拼有。
- iPad 全尺寸 26 键（开着「数字行与 Tab 键」时）画 iOS 自己的底行，26 键样式不作用于它。
- iOS 的 `long_space` 字母层里，用户自定义了常用标点时逗号长按打开常用标点菜单（26 键默认标点下长按是句号）。
- 九键标准底行的份额：Android 空格 4、回车 1.9，iOS 与鸿蒙空格 4.2、回车 1.6；九键各样式的空格份额随之不同（底栏带标点 2 对 2.2，表情键 2.95 对 3.15）。右侧大回车中列按各端自己的标准底行定宽，所以三端的绝对宽度也不同。
- iOS 九键双拼的底行多一个 0 键（排在空格后），右侧大回车里也保留。这时 iOS 的右侧大回车仍按拼音九键标准底行的份额（9.15，带地球键 10.15）定宽，不按九键双拼自己那一行（10.2，带地球键 11.2），123 与中/英比九键双拼的标准底行宽约 11%；常数里扣的键距也按拼音九键标准底行的键数算（`nineKeyStandardKeyCount` 不算 0）。这样九键双拼右侧大回车的字母层和数字层用同一基准（拼音九键的宽度基准），切层时 123 不横移；测试里九键双拼的右侧大回车也是拿拼音九键的标准底行来比。
- 顶部数字行的高度门槛：Android 是窗口低于 480 dp 不画，iOS 与鸿蒙是手机横屏不画。
- 账号同步：Android 与鸿蒙走 `TouchHostKeys`；iOS 不同步这四项，与 `touch_quanpin_shift` 相同。
- 「按键布局」的九键组判定：Android 按 `QUANPIN_NINE_KEY`、`NINE_KEY_SHUANGPIN`、`STROKE` 判断；鸿蒙的 `touchKeyLayoutGroups` 把 shuangpin 也算进九键组。只有双拼的版本里鸿蒙列 9 键组、Android 不列；现有版本表里没有这种版本，实际不受影响，本次没改。

## Alternatives considered

- 一个枚举覆盖全部键面（如 `touch_bottom_row_style`）：只有一个设置项，但多数取值只对部分键面有意义，宿主得各自定义「这个值在九键上画成什么」，三端很容易画出不一样的结果；按键面类型分开存，每个取值都有唯一的画法。
- 保留 Android 本机设置、iOS 与鸿蒙各加本机设置：改动最小，但同一账号的几台设备要各设一遍，三端的取值也会漂移；底行习惯是跨设备的，照 `touch_quanpin_shift` 的先例做成共享偏好。
- 不认识的样式让整份文档读失败（默认派生的 `Deserialize`）：能更早暴露写错的值，但较新版本加了样式之后，旧版本会读不出整份偏好；按 `CandidateTranslationScope` 的做法宽松读取。

## Consequences

- 收益：三端可以用同一组取值画底行和数字行，选择随账号同步；默认文档不变，老用户和旧版本不受影响。
- 代价：多了四个只对手机触屏有意义的共享字段，桌面宿主要在门禁里逐项说明不读的理由；Android 两个本机旧设置需要宿主做一次性迁移，迁移前两套值可能并存。
- 服务端账号字段表需要收录八个新键（Android 与鸿蒙各四个）后，这几项才会真正上传。
- 字段表收录这些键的那一刻，每台设备的同步基线里都没有它们。`merge_settings`（`crates/client-services/src/account/cloud_sync/policy.rs`）把「基线里没有的键」算作本机改过，所以之后第一次带着本机改动（`dirty`，任何一项设置变了都算）走上传或合并的设备，会用本机的值盖掉云端的值，默认值也一样。例子：手机 A 已把本机数字行 `true` 迁进 `touch_number_row` 并同步，云端是 `true`；同账号的手机 B 从没开过数字行，但改了别的设置后同步，上传 `false`；A 下一轮本机值与基线相同，跟着云端变成 `false`，A 上原有的数字行被关掉。没有本机改动的设备走下载，不会出现这种覆盖。这是合并规则对所有新进字段表的键的共同行为（`touch_quanpin_shift` 也一样），这次不改共享的合并规则：改成「基线里没有、云端已有就用云端」能挡住这个例子，但会吞掉设备在下载之后才做的真实修改，需要单独决定。顺序要求：(1) Android 宿主必须在第一次导出或应用同步之前完成 `number_row`、`long_space_layout` 两个本机旧键的迁移，迁移和导出同版发布，不能先发只导出不迁移的版本，否则会把默认值当成用户的选择传上去；(2) 服务端字段表要等带迁移的 Android 版本发布后再收录这八个键；(3) 收录之前先决定上面的合并规则要不要改。

## Verification

- `rbuild cargo test -p msime-client-core -p msime-client-services`：新增的默认值与不改写旧文档、存储往返、线上写法与不认识的值、Android 同步写法与跳过，以及导出键清单的两处快照和 Android、鸿蒙的整份往返用例。
- `python3 scripts/test-preferences-field-parity.py` 与 `python3 platforms/macos/tests/settings/preference_coverage.py .` 通过。
- 三端第二轮合并后，在合并树上跑过 `bash platforms/harmony/tests/run.sh`（0 失败）、`python3 scripts/test-preferences-field-parity.py`、`python3 scripts/test-mobile-return-yield-parity.py`（三端回车让位一致）与 `git diff --check origin/develop...HEAD`，都通过。各端的构建、模拟器结果和没验证的部分见「各端实现」。
- 三端第三轮（右侧大回车数字层统一、中列宽度规则）合并后，在合并树上重跑上面四项与三个笔记校验脚本（`verify-agent-note-tree.ts`、`verify-agent-note-format.ts`、`verify-archived-agent-notes.ts`），都通过。
- 三端第四轮（设置页入口按版本显示与说明三端统一、iOS 中列扣键距、鸿蒙笔画右列按两行对齐）合并后，在合并树上重跑 `bash platforms/harmony/tests/run.sh`、`python3 scripts/test-preferences-field-parity.py`、`python3 scripts/test-mobile-return-yield-parity.py`、`git diff --check origin/develop...HEAD` 与三个笔记校验脚本，都通过。

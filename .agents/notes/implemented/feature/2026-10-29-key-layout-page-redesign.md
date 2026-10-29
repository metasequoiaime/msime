# Agent Note: 「按键布局」设置页改成「一次看一种键盘、整块预览、胶囊选样式」

Status: implemented

## Problem

[触屏按键布局样式](2026-10-27-touch-key-layout-styles.md)上线的设置子页，维护者在 iPhone 上看过之后的评价是「这辈子没想过会有这么丑的交互方式」。问题有三个：

- 每个选项下只画一条底行缩略图，26 键四条、14/18 键三条、9 键四条，十几条几乎一样，只差一两个键，看不出整个键盘长什么样，也看不出差在哪。
- 三组全堆在一页，用户只用其中一种键盘，却要翻两屏找自己那组。
- 选了样式带来的副作用（「长空格：符号键移到工具栏」）藏在组末灰色小字里，选之前不会注意到。

## Decision

三端（iOS、Android、共享设置页即鸿蒙手机）同一套结构，存储（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style`、`touch_number_row`）、键盘排法、入口和入口副标题都不变：

1. **键盘类型分段**：`26 键 | 14/18 键 | 9 键`，只列本版本提供的类型，只有一种时不画。默认停在用户正在用的中文键盘类型（读的是键盘页「中文键盘」那一行用的同一批偏好），判断不了时停在第一个。切分段不写偏好。
2. **整块键盘预览**：数字行（开着且是 26 或 14/18 键时）、字母行和所选样式的底行都画出来；9 键画符号栏、九宫格和右列，右侧大回车画成回车竖跨两行。键序和份额取自各端键盘或热力图已有的表，不另编一套。各端第三排左键照自己键盘空闲时的样子画：Android 26 键是 ⇧，14 键是「符」；iOS 14 键是「分词」、五笔 14 键是「符」；共享页沿用热力图的表，26 键是 ⇧。
3. **高亮与标准不同的键**：强调色描边，按键的标识选，不比文字。26 键逗号句号在空格两侧高亮 `，` `。`，长空格高亮空格，表情键高亮表情键；14/18 键长空格高亮空格，表情键高亮表情键；9 键底栏带标点高亮 `，` `。`，表情键高亮表情键，右侧大回车高亮回车；标准不高亮。
4. **样式胶囊**：预览下一排，放不下时横向滚动，选中的强调色实底。点一下立刻写偏好、预览立刻变。
5. **只说当前样式的副作用**：26 键长空格「符号键移到工具栏。」，表情键（26 与 14/18）「长按 123 打开符号。」，9 键右侧大回车「回车竖占两行，符号键在左下角。」，其他样式不画这一行。取代原来的三条组末说明。
6. **顶部数字行开关**放在最下面单独一组，9 键分段时照样显示（它作用于 26 与 14/18 键），预览里 9 键不画数字行。

各端的位置：iOS `KeyboardKeyLayoutSettingsView`，数据在 `TouchKeyLayoutStylePreference.swift`（`TouchKeyLayoutKind`、各样式的 `note(_:)`、`highlightedSlots(_:)`）；Android `KeyLayoutStylesPage` + `KeyLayoutPreview`，纯逻辑在 `KeyLayoutStylesPolicy`（`BottomRowPreview` 删除）；共享页 `touch-key-layout-settings.tsx`，数据在 `touch-key-layout-styles.ts`（`*StyleNotes`、`*StyleHighlights`，`nineKeyTallReturnPreview` 删除，改用热力图的 `nineTallReturnRows`）。

## Alternatives considered

- **保留三组列表，只把缩略图换成整块键盘**：改动小。不采用：十一块整键盘排成一列比现在更长，「找自己那组」的问题更重。
- **每种样式一张卡片、横向轮播整块键盘**：直观。不采用：一次只看得到一种，样式之间没法对照，胶囊加单块预览一样能来回切着比，而且所有选项名字一直都在屏幕上。
- **按差异自动算高亮**（和标准底行逐键比）：不用维护表。不采用：逗号句号样式里「中」「回车」的左邻都变了，按位置比会把没动的键也框进去；显式表更准，三端各自用测试钉住。

## Consequences

样式名「逗号句号在空格两侧」较长，26 键的四个胶囊在 390pt 宽的手机上放不下，要横向滑一下才看得到最后一个。

## Verification

- 共享页：`apps/desktop` 下 `npx vitest run tests/settings` 476 个文件 3061 项通过（按键布局 13 项，覆盖默认分段、只列提供的分段、胶囊写偏好、各样式高亮、说明显隐、数字行开关、9 键无数字行、设置搜索、底行份额与鸿蒙键盘一致）；`tsc --noEmit`、`vp fmt --check`、`vp lint` 通过；无头浏览器 390×844 截图看过 26 键浅色、9 键深色、14/18 键。
- Android：`check-host.sh` 通过（306 项 JVM 冒烟，含新的 `KeyLayoutStylesPolicySmoke`）；`build-editor.sh` 通过；Gradle `compileFullDebugJavaWithJavac` 通过。没有在模拟器或真机上看过。
- iOS：Mac Studio 上 `MSIMEApp` 与 `MSIMEClientTests` 编译通过；测试产物拉回本机模拟器跑 `KeyLayoutStyleTests` 33 项、`TypingStatisticsTests` 22 项通过；本机 iPhone 17 Pro 模拟器上浅色、深色逐个分段点过并截图。最后两处纯界面调整（配色、切分段时胶囊滚回开头）只编译和看过，没有重跑测试。
- 鸿蒙手机、iOS 真机都没有看过。

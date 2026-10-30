# Agent Note: 「键盘」页和皮肤页的键盘预览跟随按键布局

Status: implemented

## Problem

2026-10-29 在 iPhone XR 上看到：用户的键盘是标准底行 `123 | 符 | 空格 | ， | 中 | 换行`，「键盘」页的实时预览却画 `123 | ， | 空格 | 中/英 | 换行`。[按键布局样式](../feature/2026-10-27-touch-key-layout-styles.md)上线后，只有「按键布局」子页的预览按样式画，其余整块键盘预览的底行还是写死的旧排法，三端都是：

- iOS `KeyboardSkinPreview` 写死 `123`、`，`、`中/英`、`换行`，用在「键盘」页的实时预览和皮肤相关页面；
- Android `home/KeyboardPreview` 的 `NINE_KEY_ROWS` / `FULL_ROWS` 写死，用在 AI 皮肤、皮肤、社区皮肤页；
- 共享设置页（鸿蒙手机）「屏幕键盘」页和皮肤、社区页用 `harmonyTouchKeyboardRows` 的固定底行；鸿蒙上九键双拼和笔画还会落到 26 键。

## Decision

这些预览的字母层底行和顶部数字行改按用户当前的共享偏好画（`touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style` 含右侧大回车、`touch_twenty_six_symbol_first`、`touch_number_row`，iOS 还有隐藏中/英键、Android 还有本机「交换语言键与数字键」），键序和份额取自 [按键布局页改版](../feature/2026-10-29-key-layout-page-redesign.md) 整理出的同一张表（iOS `TouchKeyLayoutKind` / `TouchBottomRowLayout`，Android `KeyLayoutStylesPolicy` / `KeyHeatmapLayout`，共享 `harmonyStyledRows`），不另写一套。预览画用户正在用的那一类键盘；九键一族（拼音九键、九键双拼、笔画）都画九宫格外框，字母格用拼音九键的 ABC。预览不做高亮。

社区皮肤只带配色、不带作者的键位，所以按读卡人自己的偏好画：他想看的是装到自己键盘上的样子。拿不到偏好时画标准样式。

画哪一类由用户当前的方案和双拼、五笔布局决定，三端用同一条规则：iOS `KeyboardPreviewFace`（取 `TouchKeyLayoutKind.of`），Android `KeyLayoutStylesPolicy.previewKind`，共享页 `touchKeyLayoutKind`（`harmonyTouchRowsForPreferences` 把 `"nine"` 一类都画成九宫格）。读不了样式的方案（手写、注音、日文、韩文等）画 26 键。iOS 最初合入时「键盘」页用 `isMandarinNineKey` 判九键，漏了笔画，笔画用户看到的是 26 键标准底行；皮肤页的缩略图写死 `nineKey: false`，九键和 14/18 键用户看到的都是 26 键。跟进时两处都改成按 `KeyboardPreviewFace` 画，笔画因此画九宫格外框并读九键样式（含右侧大回车），与键盘的 `nineKeyFrame` 一致。

iOS「键盘工具栏」设置页的预览画中文 26 键，底行按样式画，工具栏也跟着样式画：26 键选了「长空格」时，符 排在品牌键后面、其他按钮前面，与键盘的 `symbolsShortcut` 同一位置；显示方式为「隐藏」或可选按钮全关时，这一行仍画品牌、符 和收起，而不是留空的顶栏，与键盘的 `toolbarRowHidden`、`longSpaceToolbarRowKept` 相同，也与 Android「键盘工具栏」预览（`KeyboardOptionsPage.toolbarPreview`，只看 26 键样式）同一规则。表情键样式的底行也没有 符，但它的入口是长按 123，工具栏不变。规则在 SharedUI 的 `KeyboardToolbarPreviewRow.items`（返回 nil 表示不画工具栏内容），`KeyboardPreviewToolbarItem` 随之从 App 挪进 SharedUI，键盘测试才能直接测它；设置页的 `keyLayout` 与预览底行读同一份 App Group 镜像（`TouchKeyLayoutStyle.mirrored`），回到页面时重读。键盘本身的 符 按钮在[按键布局样式](../feature/2026-10-27-touch-key-layout-styles.md)那次已经做了，点按打开与 符 键相同的符号面板，这里不动。

## Alternatives considered

- **只修 iOS「键盘」页**：这是看到的那一处。不采用：三端的皮肤页预览是同一个毛病，修一处另两处照旧画错。
- **工具栏设置页的预览固定画标准底行**：底行有 符，工具栏不用加按钮，改动最小。不采用：长空格用户在这页看到的底行和工具栏都与自己的键盘不一样，这正是这篇要修的毛病。
- **社区皮肤按标准样式画**：每张卡一样，好比较。不采用：读卡人选皮肤时关心的是自己键盘的样子，标准样式对用长空格、九键的人反而陌生。

## Consequences

Android 皮肤页预览的 26 键第二排铺满整行、字母大写，九键右列的 ！ 画成功能键色，与真实键盘一致；共享表里 ！ 的键色同时影响「按键布局」页的预览。iOS 工具栏设置页的预览在长空格样式下底行去掉了 符，工具栏上补出 符，隐藏时也画品牌、符 和收起；这页的「工具栏内容已隐藏」提示随之只在这一行真的不画时出现，长空格用户选「隐藏」时看不到这句提示，预览本身说明了留下的是什么。预览规则与键盘是两份代码，靠 `KeyLayoutStyleTests` 的对照用例钉在一起。

## Verification

- iOS：Studio 上 xcodegen 生成工程后 `MSIMEApp` 和 `MSIMEClientTests` 编译通过；新增 3 个 `KeyLayoutStyleTests` 用例只编译、没跑（Studio 没有可用的模拟器）；`scripts/test-ios-key-layout-settings.py` 等契约脚本通过。
- Android：`check-host.sh` 311 项冒烟通过（新增 `KeyboardPreviewLayoutSmoke`），`build-editor.sh` 和 Gradle `compileFullDebugJavaWithJavac` 通过。
- 共享页：`npx vitest run tests/settings tests/community tests/keyboard` 593 个文件 3719 项通过（新增 `harmony-keyboard-preview-layout.test.tsx`，含九键双拼、笔画画九宫格），`tsc`、`vp fmt --check`、`vp lint` 通过。
- iOS 跟进（笔画画九宫格、皮肤页按当前键盘类型画）：新增 `KeyLayoutStyleTests.testPreviewFaceFollowsTheCurrentKeyboardKind` 和 `testPreviewFaceReadsTheSharedDocument`，结果见跟进 PR 的验证一节和它的 iOS Simulator CI。
- iOS 工具栏设置页补 符：`TouchToolbarPreferenceTests` 新增两项钉住预览规则（只有 26 键长空格带 符、排在最前；隐藏或全关时长空格仍画 符，其他样式不画），`KeyLayoutStyleTests.testToolbarPreviewMatchesTheKeyboardToolbarForEveryTwentySixKeyStyle` 对每种 26 键样式、两种显示方式，拿真键盘空闲时工具栏的可见按钮逐个比预览。结果见 PR 的验证一节和它的 iOS Simulator CI。
- 三端都没在模拟器或真机上看过修好后的预览。

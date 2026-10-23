# Agent Note: Android 候选栏为旧字体缺失的粗等号提供绘制兜底

Status: implemented

## Problem

#8038 在 Android 11 上报告 `🟰` 候选不可见，且 Via 地址栏不显示、Firefox 和编辑区可显示。Android 9 专用 AVD 的真实 `Paint.hasGlyph` 对 U+1F7F0 返回假，`=`、`＝` 和既有表情返回真。该字符属于 [Unicode 14](https://www.unicode.org/emoji/charts-14.0/emoji-released.html)，旧系统字体缺少它，不能只调整深色背景来修复。

## Decision

候选主文字中的 U+1F7F0 使用随当前文字颜色绘制的两条粗横线兜底，尺寸跟随候选字号。支持该字形的字体继续正常绘制。预编辑、候选文字、候选身份和上屏内容均保持原码位；候选条、普通展开网格和九键展开网格共用 `candidateLabel` 的处理路径。

[符号目录与 emoji 候选](../../implemented/feature/2026-10-08-android-symbol-catalog-and-emoji-candidates.md)部分重叠：该篇继续拥有面板的字体过滤和候选不按字形过滤的约束；本篇只补一个可用基本图形精确表示的候选字形，不隐藏或重排候选。其他缺字形 emoji 不在此次修复范围。

## Alternatives considered

- **隐藏缺字形的候选**：与现有表情面板行为一致，还能避免选入浏览器不支持的字符；但候选身份、翻页、高亮和数字键选词由共享引擎管理，仅在宿主删行会制造映射分歧，不能借一个绘制修复重写这套契约。
- **把上屏字符改成 `=` 或 `＝`**：旧应用也能显示；但改变了用户选中的 Unicode 字符，复制到支持原字符的应用后也无法恢复，不能静默替换。
- **随包嵌入完整 emoji 字体或启用下载字体**：能覆盖更多新表情；但完整字体增加包体，下载字体还要处理离线和无字体提供方的设备，本问题只需两条横线，先用原生图形恢复其可见性。

## Verification

`HeavyEqualsCandidateDeviceSmoke` 从安装产品加载真实 `candidateLabel` 和 `KeyboardPressButton`，对浅色、深色及高亮颜色绘制位图：旧字体缺字时必须有两条分离的横带，支持字形时沿用字体绘制且不为空。另检查原始代理对、变体选择符、数字前缀、释义 span 范围、其他候选不变及绘制不污染 `Paint`。

Android 9（API 28）上 `system missing glyph=true`，Android 15（API 35）上为 `false`，两条分支的安装产品回归都通过。API 28 的完整 core 装包验收通过，包含系统中文输入与删除、过期候选回调和剪贴板保存生命周期。API 35 只运行本次候选绘制回归，没有重跑全部键盘套件。宿主检查的 231 项 JVM 冒烟及 111 个设备源码编译通过。

本地 quick、`pnpm run verify-notes` 和 `git diff --check` 通过。设备复测还检查安装 APK 与本次构建的 SHA-256 一致，避免同一专用 AVD 上安装包被替换后误测其他版本。

## Consequences

旧系统候选中的粗等号有可见绘制并跟随文字颜色，没有新字体、网络下载、候选过滤或上屏字符替换。已有符号与表情面板的缺字过滤规则不变，[手势候选局部图像](2026-10-19-android-hand-emoji-candidate-glyph.md)扩展四个新手势的显示；其余缺字形 emoji 仍需完整能力映射方案。

Via 的地址栏不使用本应用的候选标签，兜底无法改变该浏览器的字体。PR 使用 `Refs #8038` 保留外部编辑器的验收缺口，不宣称整个 issue 已解决。此处“原始字符保留”由标签原码位回归验证，不等同于在 Via 实机完成上屏验收。

[Unicode 18 下标补齐](2026-10-23-android-unicode18-subscripts.md)补入 w/y/z 的真实码位，并仅在缺字形时缩小当前字母绘制，长按选项与候选共用局部标签处理。

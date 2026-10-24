#pragma once

namespace msime::candidate
{
// 候选窗自绘部分的尺寸与透明度，都是 100% 大小时的值，乘 candidate_scale_percent 换算。macOS（CandidateChrome.h、CandidateTypography.h 和 InputController.mm 的预编辑光标）和 Linux 的 X11 原生画布（platforms/linux/src/candidates/NativeCanvas.h）都从这里取，改一处两边一起变。

// 释义没有自己的颜色时取正文色的这个透明度。
constexpr double kCandidateTranslationOpacity = 0.62;
// 读音比它跟着的释义再淡一些，相对释义色。
constexpr double kCandidatePronunciationOpacity = 0.72;
// 译文默认使用 12pt，不随候选字号变化；支持独立译文字号的宿主可用用户偏好覆盖它。
constexpr double kCandidateTranslationPointSize = 12.0;
// 首行：左边 logo 和拼音，右边「1 / 3」、‹ › 和展开键。
constexpr double kCandidateHeaderHeight = 26.0;
// 横排卡片超过半屏时离可见区域两边各留的距离。
constexpr double kCandidateScreenMargin = 16.0;
// 「1 / 3」用 13pt 等宽数字，次要色（dc.html L1324）。
constexpr double kCandidatePageIndicatorPointSize = 13.0;
constexpr double kCandidatePageArrowWidth = 28.0;
constexpr double kCandidatePageIndicatorGap = 4.0;
// 没有上一页或下一页的箭头再淡到这个比例。
constexpr double kCandidateDisabledArrowAlpha = 0.4;
// 展开、收起键的 chevron 符号字号。
constexpr double kCandidateExpandSymbolPointSize = 11.0;
// A row keeps 6 points above and below its text, and a gloss under the text 2 points on each side of its lines.
constexpr double kCandidateRowPadding = 12.0;
constexpr double kCandidateGlossPadding = 4.0;
constexpr double kCandidateTextRight = 8.0;
// 序号字号是候选字号的 0.8 倍，和候选文字隔 1.5pt（Windows 的标签字体同样是 80%）。
constexpr double kCandidateNumberScale = 0.8;
constexpr double kCandidateNumberGap = 1.5;
// 首行打头的 logo 边长和它与拼音之间的间距。
constexpr double kCandidateLogoSide = 16.0;
constexpr double kCandidateLogoGap = 6.0;
// 拼音行至少这么高；文字更高时在文字高度上加 kCandidatePreeditPadding。
constexpr double kCandidatePreeditMinHeight = 22.0;
constexpr double kCandidatePreeditPadding = 6.0;
// 拼音里的插入光标：宽度和两侧留白，与 Windows TextBlock 的预编辑光标间距一致。
constexpr double kCandidatePreeditCaretWidth = 1.25;
constexpr double kCandidatePreeditCaretSideAir = 0.85;
constexpr double kCandidatePreeditCaretEndAir = 1.5;
constexpr double kCandidatePreeditCaretGap = kCandidatePreeditCaretWidth + 2 * kCandidatePreeditCaretSideAir;
// 有翻页时卡片至少这么宽，放得下首行的翻页控件。
constexpr double kCandidatePagingMinimumWidth = 76.0;
// 卡片宽度以可见区域的这个比例封顶（横排一行放得下时例外）。
constexpr double kCandidateWidthCapRatio = 0.5;
// 固定了排位的候选未选中时正文画这个颜色（Windows 的 fixed-position span）。
constexpr unsigned kCandidateFixedPositionRgb = 0x379AD3;

// 行内序号前的留白：没有选中条时 8pt，有选中条时再让出 6pt。
constexpr double CandidateTextLeft(bool showSelectedBar, double scale = 1.0)
{
    return (8.0 + (showSelectedBar ? 6.0 : 0.0)) * scale;
}
} // namespace msime::candidate

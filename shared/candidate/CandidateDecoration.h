#pragma once

#include <optional>

namespace msime::candidate
{
// 皮肤顶带装饰图摆在哪儿。macOS（platforms/macos/src/candidate/CandidateSkin.h 以 using 引入）和 Linux 的 X11 原生画布共用；Windows 的 candidate_decoration_left 是同一条规则。

// Where the decoration sits along the card's top edge.
enum class DecorationAlign
{
    left,
    center,
    right,
};

// 有待机动效的装饰图在窗口顶边和左右两边至少留出的点数。各动效的最大位移都不超过它（见 CandidateMotionRules.h），候选窗裁掉越界部分时装饰图不会被切掉。
constexpr double kDecorationMotionMargin = 4.0;

// The x of a decoration `width` wide over a card `cardWidth` wide: `pad` in from the edge it is aligned to, or centred, never left of the card. The same rule as the Windows host's candidate_decoration_left.
inline double DecorationLeft(DecorationAlign align, double cardWidth, double pad, double width)
{
    double left = cardWidth - pad - width;
    switch (align)
    {
    case DecorationAlign::left:
        left = pad;
        break;
    case DecorationAlign::center:
        left = (cardWidth - width) / 2.0;
        break;
    case DecorationAlign::right:
        break;
    }
    return left > 0.0 ? left : 0.0;
}

// Where the decoration is drawn, in top-down coordinates: x from the card's left edge, top from the window's top edge.
struct DecorationRect
{
    double x = 0.0;
    double top = 0.0;
    double width = 0.0;
    double height = 0.0;
};

// The decoration as every host draws it: the window is `band` taller than the card and that band is transparent; the image is `width` wide at its own aspect ratio, its bottom `pad` below the card's top edge so it sits over the edge, and it is drawn after the card. An image too tall for the band and the overlap is scaled down whole rather than squashed. None for an empty band, width or image.
// `margin` 是待机动效要的余量（kDecorationMotionMargin，没有动效时为 0）：动起来的装饰图离窗口顶边和左右两边至少这么远。
inline std::optional<DecorationRect> DecorationPlacement(DecorationAlign align, double cardWidth, double pad, double band,
                                                         double width, double imageWidth, double imageHeight,
                                                         double margin = 0.0)
{
    if (!(band > 0.0) || !(width > 0.0) || !(imageWidth > 0.0) || !(imageHeight > 0.0))
        return std::nullopt;
    const double room = band + pad;
    const bool moving = margin > 0.0;
    const double tallest = moving ? room - margin : room;
    const double widest = cardWidth - 2.0 * margin;
    if (!(tallest > 0.0) || (moving && !(widest > 0.0)))
        return std::nullopt;
    double height = width * imageHeight / imageWidth;
    if (height > tallest)
    {
        width *= tallest / height;
        height = tallest;
    }
    if (moving && width > widest)
    {
        height *= widest / width;
        width = widest;
    }
    double left = DecorationLeft(align, cardWidth, pad, width);
    if (moving && left < margin)
        left = margin;
    if (moving && left + width > cardWidth - margin)
        left = cardWidth - margin - width;
    return DecorationRect{left, room - height, width, height};
}
} // namespace msime::candidate

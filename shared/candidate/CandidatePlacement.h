#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <limits>
#include <optional>
#include <vector>

namespace msime::candidate
{
// 候选窗摆在光标旁边的规则，macOS（platforms/macos/src/candidate/CandidatePlacement.h 包一层 NSRect）和 Linux 的 X11 原生画布共用。坐标系照 AppKit：y 朝上，矩形的 y 是下边。Linux 的 X11 坐标 y 朝下，调用方先用 CandidateFlipY 翻过来，算完再翻回去，两边因此按同一条规则翻转和夹边。
struct CandidateRect
{
    double x = 0.0;
    double y = 0.0;
    double width = 0.0;
    double height = 0.0;
    double minX() const { return x; }
    double minY() const { return y; }
    double maxX() const { return x + width; }
    double maxY() const { return y + height; }
};

struct CandidatePoint
{
    double x = 0.0;
    double y = 0.0;
};

// 光标间隙：卡片和光标之间留 4 个单位。
constexpr double kCandidateCaretGap = 4.0;

// y 朝下的矩形换成 y 朝上（反过来也一样）：以 0 为轴镜像，宽高不变。
constexpr CandidateRect CandidateFlipY(CandidateRect rect)
{
    return {rect.x, -(rect.y + rect.height), rect.width, rect.height};
}

inline bool CandidateValidCaret(const CandidateRect &caret)
{
    return std::isfinite(caret.x) && std::isfinite(caret.y) && std::isfinite(caret.width) &&
           std::isfinite(caret.height) && caret.height > 0;
}

// The tallest vertical candidate panel shown since the panel last became visible, matching Windows' g_max_vertical_container_height_dip. A panel that was hidden in between starts over, as Windows' ResetCandidatePlacementMemory does when its candidate window hides; a horizontal panel carries the memory without adding to it.
constexpr double TallestCandidateHeight(double remembered, double current, bool vertical, bool panelWasVisible)
{
    const double base = panelWasVisible ? std::max(remembered, 0.0) : 0.0;
    return vertical ? std::max(base, current) : base;
}

// 横排候选窗自上次显示以来最宽的一次。横排卡片的宽度按这一页候选的长短算，打字时候选每个键都在变，不记的话卡片随每个键忽长忽短（实测一次输入里在 557pt 和 792pt 之间来回跳，同一个键补一轮候选还会再变一次）。记住之后，候选窗一直开着的这段时间里卡片只变宽不变窄；候选窗隐藏过（上屏、取消、失焦）就从头记，与 TallestCandidateHeight 对竖排高度的处理相同。竖排不记，返回 0。
constexpr double WidestCandidateWidth(double remembered, double current, bool vertical, bool panelWasVisible)
{
    if (vertical) return 0.0;
    const double base = panelWasVisible ? std::max(remembered, 0.0) : 0.0;
    return std::max(base, current);
}

// Decides the side of the caret with the taller of the current height and flipDecisionHeight, but places the panel with its current height. A vertical list that grows as candidates arrive therefore does not jump from below the caret to above it, and a shorter later page that stays above the caret sits against the line instead of leaving the tallest page's gap.
// cardTopInset 是窗口顶上留给皮肤装饰的透明带，size 和 flipDecisionHeight 都含这条带。放在光标下方时贴着光标的是卡片顶边，不是窗口顶边：透明带朝上伸，盖在光标所在的这一行和上面几行上，装饰图不透明的地方会挡住那里的字。放不下、翻到光标上方时窗口底边就是卡片底边，透明带在最上面，和以前一样贴着光标上方。翻不翻只看卡片：卡片在光标下方放得下就放下方，透明带朝上伸不占光标下方的地方。最后整个窗口（连同透明带）仍夹进可见区域，光标贴近屏幕上边、透明带伸出上边时宁可把窗口往下压、卡片和光标之间留出一段，也不让装饰图被屏幕边缘裁掉；留出的这段至多是透明带伸出的部分，比以前整条透明带的空隙小。cardTopInset 为 0 时结果和以前完全一样。
// canvas 的左、下外留白不参与卡片对齐和翻转判断；窗口仍整体夹进可见区域，保留原有装饰裁切策略。返回窗口左下角。
inline CandidatePoint CandidateOrigin(CandidateRect caret, double width, double height, CandidateRect bounds,
                                      double flipDecisionHeight, double cardTopInset = 0, double cardLeftInset = 0,
                                      double cardBottomInset = 0)
{
    const double left = std::min(std::max(cardLeftInset, 0.0), width);
    const double x = std::min(std::max(caret.minX() - left, bounds.minX()), std::max(bounds.minX(), bounds.maxX() - width));
    const double band = std::min(std::max(cardTopInset, 0.0), height);
    const double bottom = std::min(std::max(cardBottomInset, 0.0), height - band);
    const double decisionHeight = std::max(height, flipDecisionHeight) - band - bottom;
    double y = caret.minY() - (height - band) - kCandidateCaretGap;
    if (caret.minY() - decisionHeight - kCandidateCaretGap < bounds.minY()) y = caret.maxY() + kCandidateCaretGap - bottom;
    y = std::min(std::max(y, bounds.minY()), std::max(bounds.minY(), bounds.maxY() - height));
    return {x, y};
}

// 候选窗放在 frames 里的哪一块屏幕上：先取含住锚点的第一块（含左边和下边、不含右边和上边，两块屏幕共用的边因此归右边、上边那块）；一块都不含时取离锚点最近的一块，距离相同取排在前面的。frames 为空时没有结果。
inline std::optional<std::size_t> CandidateScreenIndex(CandidatePoint anchor, const std::vector<CandidateRect> &frames)
{
    std::optional<std::size_t> nearest;
    double nearestDistance = std::numeric_limits<double>::infinity();
    for (std::size_t index = 0; index < frames.size(); ++index)
    {
        const CandidateRect &frame = frames[index];
        if (anchor.x >= frame.minX() && anchor.x < frame.maxX() && anchor.y >= frame.minY() && anchor.y < frame.maxY())
            return index;
        const double dx = std::max(std::max(frame.minX() - anchor.x, 0.0), anchor.x - frame.maxX());
        const double dy = std::max(std::max(frame.minY() - anchor.y, 0.0), anchor.y - frame.maxY());
        const double distance = dx * dx + dy * dy;
        if (distance < nearestDistance)
        {
            nearest = index;
            nearestDistance = distance;
        }
    }
    return nearest;
}
} // namespace msime::candidate

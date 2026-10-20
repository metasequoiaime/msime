#pragma once

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <functional>
#include <optional>
#include <vector>

namespace msime::mac
{
// Candidate item geometry ported from the Windows presenter: candidate_item_layout and candidate_page_layout in platforms/windows/src/candidate/CandidateCardSize.h, which are themselves ports of the shipped CandidateList::MeasureItem and CandidateList::Measure. Widths and heights arrive already measured in points, so this header only composes them and stays free of AppKit. Sizing, drawing and hit testing all read the one result: the candidate buttons take their frames from the rows and draw their runs into the boxes, so a wrapped line is always inside the area that answers the click.

// Single-line widths of one candidate's three runs: the candidate text with its badge, the annotation (the 辅助码) at the candidate font, and the gloss at the gloss font. All three zero hides the row.
struct CandidateItemWidths
{
    double text = 0.0;
    double annotation = 0.0;
    double translation = 0.0;
    // Height of the gloss drawn without wrapping. A gloss carries one line per target language, so this can be more than one translationLine; zero means a single translationLine.
    double translationHeight = 0.0;
};

enum class CandidateRun
{
    text,
    annotation,
    translation
};

struct CandidateLayoutMetrics
{
    // Height of a row whose text fits on one line.
    double candidateRow = 0.0;
    // Everything in a row that is not content: selection number, bar and the paddings on both sides. A row's content width is its width minus this.
    double chrome = 0.0;
    // The annotation follows the text 4 points later at the candidate font; the gloss is 0.65 of the candidate font away. A run moved below the first line takes at least one line of its own font.
    double annotationGap = 4.0;
    double annotationLine = 0.0;
    double translationGap = 0.0;
    double translationLine = 0.0;
    double itemGap = 0.0;
    // 普通横排列距包含文字后的固定 6 DIP；收紧时和竖排一样只用 itemGap。
    double columnGap = 0.0;
    double itemMinWidth = 0.0;
    // 横排每行剩余宽度只均分给可见候选，空行不能变成可点击的空格。
    bool justifyRows = false;
};

// A run's box relative to the row's content column: x from where the candidate text starts, y from the row top. Zero width means the run is absent. An inline run shares the first line and is centred in it; one below the first line is top aligned and may wrap.
struct CandidateRunBox
{
    double x = 0.0;
    double y = 0.0;
    double width = 0.0;
    double height = 0.0;
    bool below = false;
};

struct CandidateItemLayout
{
    double textWidth = 0.0;
    // Height of the text's box from the row top: one candidateRow, or the measured wrapped height when the text is wider than the column. The text is centred in it, and runs placed below the first line start under it.
    double textHeight = 0.0;
    // Whether the text is wider than the column and so is drawn wrapped.
    bool textWrapped = false;
    CandidateRunBox annotation;
    CandidateRunBox translation;
    // At least one candidateRow; grows by every run placed below the first line.
    double height = 0.0;
};

// Height of a run once wrapped to `width` points. Returning a non-finite or too small value keeps the run's single line.
using CandidateRunMeasure = std::function<double(CandidateRun run, double width)>;

// Port of CandidateList::MeasureItem. Text wider than the column wraps inside it and the row takes the measured height, at least one candidateRow. Short runs stay on the text's line; a run that does not fit moves under the text and wraps to the column. A horizontal list always puts the gloss under the text, and a gloss never goes back up once the annotation has moved down.
inline CandidateItemLayout LayoutCandidateItem(const CandidateItemWidths &item, double contentWidth,
                                               const CandidateLayoutMetrics &metrics, bool horizontal,
                                               const CandidateRunMeasure &wrapped)
{
    contentWidth = std::isfinite(contentWidth) ? std::max(contentWidth, 1.0) : 1.0;
    CandidateItemLayout layout;
    layout.textWidth = std::min(std::max(item.text, 0.0), contentWidth);
    // Height of a run of `natural` single-line width once it sits in `width`: `line` unless it has to wrap.
    auto runHeight = [&](CandidateRun run, double natural, double width, double line) {
        double height = line;
        if (natural > width)
            height = wrapped ? wrapped(run, width) : std::ceil(natural / width) * line;
        // A failed or nonsense measurement still reserves the single line.
        return std::isfinite(height) ? std::max(height, line) : line;
    };
    layout.textWrapped = item.text > contentWidth;
    layout.textHeight =
        layout.textWrapped
            ? std::max(runHeight(CandidateRun::text, item.text, contentWidth, metrics.annotationLine),
                       metrics.candidateRow)
            : metrics.candidateRow;
    layout.height = layout.textHeight;
    double lineEnd = layout.textWidth;
    auto below = [&](CandidateRun run, double natural, double width, double line) {
        const double height = runHeight(run, natural, width, line);
        CandidateRunBox box{0.0, layout.height, width, height, true};
        layout.height += height;
        return box;
    };
    if (item.annotation > 0.0)
    {
        const double width = std::min(item.annotation, contentWidth);
        if (lineEnd + metrics.annotationGap + width <= contentWidth)
        {
            layout.annotation = {lineEnd + metrics.annotationGap, 0.0, width, metrics.candidateRow, false};
            lineEnd = layout.annotation.x + width;
        }
        else
        {
            layout.annotation = below(CandidateRun::annotation, item.annotation, width, metrics.annotationLine);
        }
    }
    if (item.translation > 0.0)
    {
        const double width = std::min(item.translation, contentWidth);
        const double line = std::max(metrics.translationLine,
                                     std::isfinite(item.translationHeight) ? item.translationHeight : 0.0);
        if (!horizontal && !layout.annotation.below && lineEnd + metrics.translationGap + width <= contentWidth)
        {
            // A gloss with one line per target language can stand taller than the text beside it; the row grows to hold it.
            const double height = std::max(metrics.candidateRow, line);
            layout.translation = {lineEnd + metrics.translationGap, 0.0, width, height, false};
            layout.height = std::max(layout.height, height);
        }
        else
        {
            layout.translation = below(CandidateRun::translation, item.translation, width, line);
        }
    }
    return layout;
}

// Width a candidate's row asks for with nothing wrapped. A vertical row keeps the gloss on the text's line; a horizontal one stacks it under the text, so the wider of the two lines decides. Zero for a hidden candidate.
inline double CandidateItemNaturalWidth(const CandidateItemWidths &item, const CandidateLayoutMetrics &metrics,
                                        bool horizontal)
{
    if (item.text <= 0.0 && item.annotation <= 0.0 && item.translation <= 0.0)
        return 0.0;
    const double line = item.text + (item.annotation > 0.0 ? metrics.annotationGap + item.annotation : 0.0);
    const double content =
        horizontal ? std::max(line, item.translation)
                   : line + (item.translation > 0.0 ? metrics.translationGap + item.translation : 0.0);
    return std::max(content + metrics.chrome, metrics.itemMinWidth);
}

// The narrowest a horizontal page's columns can be on one line: each candidate's line (text and annotation) with its chrome, the glosses left to wrap under them. The card may grow past its usual cap to this width so a page of long candidates still stays on one line.
inline double SingleLineMinimumWidth(const std::vector<CandidateItemWidths> &items, const CandidateLayoutMetrics &metrics)
{
    const double columnGap = metrics.columnGap > 0.0 ? metrics.columnGap : metrics.itemGap;
    double total = 0.0;
    for (const auto &item : items)
    {
        CandidateItemWidths line = item;
        line.translation = 0.0;
        const double wide = CandidateItemNaturalWidth(item, metrics, true);
        if (wide > 0.0)
            total += std::min(std::max(CandidateItemNaturalWidth(line, metrics, true), metrics.chrome), wide);
    }
    return total + (items.empty() ? 0.0 : columnGap * static_cast<double>(items.size() - 1));
}

// The columns of a horizontal page on a single line `lineWidth` wide. Columns that fit keep their natural widths. Otherwise each column whose gloss is wider than its candidate line gives up room, never below that line and in proportion to how much it could give, so the glosses wrap under their text instead of the page starting a second line. None when the candidate lines alone do not fit.
inline std::optional<std::vector<double>> SingleLineColumns(const std::vector<CandidateItemWidths> &items, double lineWidth,
                                                            const CandidateLayoutMetrics &metrics)
{
    const double columnGap = metrics.columnGap > 0.0 ? metrics.columnGap : metrics.itemGap;
    std::vector<double> natural, firm;
    natural.reserve(items.size());
    firm.reserve(items.size());
    double naturalTotal = 0.0, firmTotal = 0.0;
    for (const auto &item : items)
    {
        CandidateItemWidths line = item;
        line.translation = 0.0;
        const double wide = CandidateItemNaturalWidth(item, metrics, true);
        // A gloss-only candidate still needs a column; its line is the chrome.
        const double narrow = wide > 0.0 ? std::max(CandidateItemNaturalWidth(line, metrics, true), metrics.chrome) : 0.0;
        natural.push_back(wide);
        firm.push_back(std::min(narrow, wide));
        naturalTotal += wide;
        firmTotal += firm.back();
    }
    const double gaps = items.empty() ? 0.0 : columnGap * static_cast<double>(items.size() - 1);
    lineWidth = std::max(1.0, lineWidth - gaps);
    if (naturalTotal <= lineWidth)
        return natural;
    if (firmTotal > lineWidth || !(naturalTotal > firmTotal))
        return std::nullopt;
    const double keep = (lineWidth - firmTotal) / (naturalTotal - firmTotal);
    for (std::size_t index = 0; index < natural.size(); ++index)
        natural[index] = firm[index] + (natural[index] - firm[index]) * keep;
    return natural;
}

// One laid out row, in the card's content area: x from its left edge, y downwards from its top.
struct CandidateRowLayout
{
    double x = 0.0;
    double y = 0.0;
    double width = 0.0;
    double height = 0.0;
    CandidateItemLayout item;
};

// Height of candidate `index`'s run once wrapped to `width` points.
using CandidatePageMeasure = std::function<double(std::size_t index, CandidateRun run, double width)>;

// Rows for a whole page at the width the rows actually get. A vertical list stacks full-width rows of their own heights. A horizontal list keeps every candidate on one line: SingleLineColumns narrows the glosses to fit, and only when the candidate lines themselves do not fit does it fall back to CandidateList::Measure, where each column is its candidate's natural width and one that would pass the line's end starts a new line (a candidate wider than a whole line is narrowed to it, and its text and runs wrap inside that). Every column on a line takes the line's tallest height, so the selection fills evenly. `minimumHeight` raises every row (vertical) or line (horizontal) to at least that height.
inline std::vector<CandidateRowLayout> LayoutCandidatePage(const std::vector<CandidateItemWidths> &items,
                                                           double lineWidth, const CandidateLayoutMetrics &metrics,
                                                           bool horizontal, const CandidatePageMeasure &wrapped = {},
                                                           double minimumHeight = 0.0)
{
    std::vector<CandidateRowLayout> rows;
    rows.reserve(items.size());
    lineWidth = std::isfinite(lineWidth) ? std::max(lineWidth, 1.0) : 1.0;
    const double floorHeight = std::isfinite(minimumHeight) ? std::max(minimumHeight, 0.0) : 0.0;
    double top = 0.0;
    double tallest = 0.0;
    double x = 0.0;
    std::size_t lineStart = 0;
    auto closeLine = [&](std::size_t end) {
        tallest = std::max(tallest, floorHeight);
        for (std::size_t index = lineStart; index < end; ++index)
            rows[index].height = tallest;
        if (metrics.justifyRows)
        {
            double used = 0.0;
            std::size_t count = 0;
            for (std::size_t index = lineStart; index < end; ++index)
                if (rows[index].width > 0.0)
                {
                    used = std::max(used, rows[index].x + rows[index].width);
                    ++count;
                }
            const double extra = count ? std::max(0.0, lineWidth - used) / static_cast<double>(count) : 0.0;
            std::size_t visible = 0;
            for (std::size_t index = lineStart; index < end; ++index)
                if (rows[index].width > 0.0)
                {
                    rows[index].x += static_cast<double>(visible++) * extra;
                    rows[index].width += extra;
                }
        }
        lineStart = end;
    };
    const auto singleLine = horizontal ? SingleLineColumns(items, lineWidth, metrics) : std::nullopt;
    const double columnGap = metrics.columnGap > 0.0 ? metrics.columnGap : metrics.itemGap;
    for (std::size_t index = 0; index < items.size(); ++index)
    {
        CandidateRowLayout row;
        if (horizontal)
        {
            const double natural = CandidateItemNaturalWidth(items[index], metrics, true);
            const double column = singleLine ? (*singleLine)[index] : natural > 0.0 ? std::min(natural, lineWidth) : 0.0;
            if (!singleLine && x > 0.0 && x + column > lineWidth)
            {
                closeLine(index);
                top += tallest + metrics.itemGap;
                tallest = 0.0;
                x = 0.0;
            }
            row.x = x;
            row.y = top;
            row.width = column;
            x += column;
            if (index + 1 < items.size()) x += columnGap;
        }
        else
        {
            row.x = 0.0;
            row.y = top;
            row.width = lineWidth;
        }
        CandidateRunMeasure measure;
        if (wrapped)
            measure = [&wrapped, index](CandidateRun run, double width) { return wrapped(index, run, width); };
        row.item = LayoutCandidateItem(items[index], row.width - metrics.chrome, metrics, horizontal, measure);
        if (horizontal)
        {
            tallest = std::max(tallest, row.item.height);
        }
        else
        {
            row.height = std::max(row.item.height, floorHeight);
            top += row.height;
            if (index + 1 < items.size()) top += metrics.itemGap;
        }
        rows.push_back(row);
    }
    if (horizontal && !rows.empty())
        closeLine(rows.size());
    return rows;
}

// 原生主题保留 7em 卡片下限；共享 Windows 版式使用固定 160 DIP，fitContent 取消默认下限。
// 内置下限夹在可见屏幕内，包声明与装饰的独立下限始终保留；没有共享版式时不改变旧调用。
constexpr double kCandidateMinWidthEm = 7.0;

inline double CandidateCardMinimumWidth(double fontSize, double skinMinWidth, double screenCap,
                                         std::optional<bool> skinFitContent = std::nullopt, double scale = 1.0)
{
    double builtIn = skinFitContent.has_value() ? (*skinFitContent ? 0.0 : 160.0 * scale)
        : std::isfinite(fontSize) && fontSize > 0.0 ? fontSize * kCandidateMinWidthEm : 0.0;
    if (std::isfinite(screenCap) && screenCap > 0.0)
        builtIn = std::min(builtIn, screenCap);
    const double skin = std::isfinite(skinMinWidth) && skinMinWidth > 0.0 ? skinMinWidth : 0.0;
    return std::max(builtIn, skin);
}

// Total height the rows take: a vertical page is the sum of its rows, a horizontal one ends at its last line's bottom.
inline double CandidatePageHeight(const std::vector<CandidateRowLayout> &rows)
{
    double bottom = 0.0;
    for (const auto &row : rows)
        bottom = std::max(bottom, row.y + row.height);
    return bottom;
}
} // namespace msime::mac

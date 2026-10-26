#pragma once
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <limits>
#include <vector>
#include "../../../../shared/candidate/CandidateExpandedGrid.h"

namespace msime::mac {
struct AdaptiveCandidateRows {
    std::vector<std::size_t> starts{0};
    std::vector<std::vector<double>> widths;
    // 按需分页停在最后一行之后时为真：后面还有候选，只是还没排。
    bool more = false;

    // 已知的页数：排好的行，加上还没排的下一页（如果有）。总数未知，只够判断前后有没有页。
    std::size_t knownPages() const { return widths.size() + (more ? 1 : 0); }

    std::size_t rowFor(std::size_t index) const {
        if (widths.empty()) return 0;
        const auto next = std::upper_bound(starts.begin(), starts.end(), index);
        return next == starts.begin() ? 0 : std::min(widths.size() - 1, std::size_t(next - starts.begin() - 1));
    }
};

inline std::size_t AdaptiveCandidateAtColumn(const AdaptiveCandidateRows &rows, std::size_t row, double x, double gap) {
    double left = 0;
    for (std::size_t slot = 0; slot < rows.widths[row].size(); ++slot) {
        if (x < left + rows.widths[row][slot] + gap / 2) return rows.starts[row] + slot;
        left += rows.widths[row][slot] + gap;
    }
    return rows.starts[row + 1] - 1;
}

inline candidate::ExpandedGridStep StepAdaptiveCandidates(const AdaptiveCandidateRows &rows, std::size_t cursor,
    std::size_t pageRows, candidate::ExpandedGridMove move, double gap) {
    if (rows.widths.empty()) return {0, false};
    const auto row = rows.rowFor(cursor);
    const auto start = rows.starts[row], end = rows.starts[row + 1];
    using candidate::ExpandedGridMove;
    if (move == ExpandedGridMove::Left) return {cursor > start ? cursor - 1 : cursor, false};
    if (move == ExpandedGridMove::Right) return {cursor + 1 < end ? cursor + 1 : cursor, false};
    if (move == ExpandedGridMove::Up && row == 0) return {cursor, true};
    double x = 0;
    for (std::size_t index = start; index < cursor; ++index) x += rows.widths[row][index - start] + gap;
    const auto step = move == ExpandedGridMove::PageUp || move == ExpandedGridMove::PageDown ? std::max(std::size_t{1}, pageRows) : 1;
    const bool previous = move == ExpandedGridMove::Up || move == ExpandedGridMove::PageUp;
    const auto target = previous ? (row > step ? row - step : 0) : std::min(row + step, rows.widths.size() - 1);
    return {AdaptiveCandidateAtColumn(rows, target, x, gap), false};
}

// 单行按自然宽度装满；展开时列跨度向上取整，长词独占多列，超宽词收窄到整行。
// 逐行贪心装填，一行只取决于它之前的候选，所以排完含 `through` 的那一行、且已排够 `minimumRows` 行就可以停，
// `more` 标出后面还有。只给了前缀宽度时，没有 `more` 的最后一行可能还没装满，调用方要么已给出全部宽度，要么补量后重排。
inline AdaptiveCandidateRows LayoutAdaptiveCandidates(const std::vector<double> &naturalWidths,
                                                       double available, double gap, std::size_t columns = 0,
                                                       const std::vector<double> &numberWidths = {},
                                                       double minimumWidth = 0,
                                                       std::size_t through = std::numeric_limits<std::size_t>::max(),
                                                       std::size_t minimumRows = 0) {
    AdaptiveCandidateRows result;
    available = std::max(1.0, available);
    gap = std::max(0.0, gap);
    const double unit = columns ? (available + gap) / columns : 0;
    // 序号栏按行内本地编号的位数留宽：同一行跨到两位数时整行一起加宽，换行后从 1 重新算。
    auto rowWidths = [&](std::size_t start, std::size_t end) {
        const double number = numberWidths.empty() ? 0 : numberWidths[std::min(end - start, numberWidths.size()) - 1];
        std::vector<double> row;
        for (std::size_t index = start; index < end; ++index) {
            double width = std::clamp(std::max(naturalWidths[index] + number, minimumWidth), 1.0, available);
            if (columns) width = std::min(available, std::ceil((width + gap) / unit) * unit - gap);
            row.push_back(width);
        }
        return row;
    };
    std::size_t start = 0;
    for (std::size_t end = 2; end <= naturalWidths.size(); ++end) {
        const auto row = rowWidths(start, end);
        double used = gap * (row.size() - 1);
        for (double width : row) used += width;
        if (used <= available + 0.001) continue;
        result.widths.push_back(rowWidths(start, end - 1));
        start = end - 1;
        result.starts.push_back(start);
        if (start > through && result.widths.size() >= minimumRows) {
            result.more = true;
            return result;
        }
    }
    if (start < naturalWidths.size()) result.widths.push_back(rowWidths(start, naturalWidths.size()));
    result.starts.push_back(naturalWidths.size());
    return result;
}
}

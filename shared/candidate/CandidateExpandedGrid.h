#pragma once
#include <cstddef>

namespace msime::candidate
{
// 展开的候选网格（共享偏好 `expand_candidates`，#7307）里光标怎么走。网格一行是当前每页候选数，行与行之间上下滚动；这里只管下标，不碰 AppKit，所以能单独测。
//
// 规则：←/→ 只在当前行里移动，不换行；↑/↓ 在行之间移动并保持列，目标行比这一列短时停在它的最后一个；第一行再按 ↑ 是收起网格（`close`），光标不动；翻页键一次走 `pageRows` 行（网格可见的行数），到头为止、不收起。

// 一次最多画几行。超出的行靠 ↑/↓、翻页键和滚轮滚进来。
constexpr std::size_t kExpandedGridVisibleRows = 5;

enum class ExpandedGridMove
{
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
};

struct ExpandedGridStep
{
    std::size_t cursor = 0;
    // 第一行按 ↑：收起网格，组字保留。
    bool close = false;
};

constexpr std::size_t ExpandedGridRows(std::size_t count, std::size_t columns)
{
    return columns == 0 ? 0 : (count + columns - 1) / columns;
}

constexpr ExpandedGridStep StepExpandedGrid(std::size_t cursor, std::size_t count, std::size_t columns,
                                            std::size_t pageRows, ExpandedGridMove move)
{
    if (count == 0 || columns == 0)
        return {0, move == ExpandedGridMove::Up};
    if (cursor >= count)
        cursor = count - 1;
    const std::size_t rows = ExpandedGridRows(count, columns);
    const std::size_t row = cursor / columns;
    const std::size_t column = cursor % columns;
    // 目标行里同一列；那一行比它短（最后一行）时停在行尾。
    auto at = [&](std::size_t target) {
        const std::size_t index = target * columns + column;
        return index < count ? index : count - 1;
    };
    const std::size_t step = pageRows == 0 ? 1 : pageRows;
    switch (move)
    {
    case ExpandedGridMove::Left:
        return {column > 0 ? cursor - 1 : cursor, false};
    case ExpandedGridMove::Right:
        return {column + 1 < columns && cursor + 1 < count ? cursor + 1 : cursor, false};
    case ExpandedGridMove::Up:
        if (row == 0)
            return {cursor, true};
        return {at(row - 1), false};
    case ExpandedGridMove::Down:
        return {row + 1 < rows ? at(row + 1) : cursor, false};
    case ExpandedGridMove::PageUp:
        return {at(row > step ? row - step : 0), false};
    case ExpandedGridMove::PageDown:
        return {at(row + step < rows ? row + step : rows - 1), false};
    }
    return {cursor, false};
}

// 可见的第一行：光标所在行在可见范围之外时滚到刚好看见它，其余时候不动；末页允许少画几行，保持打开时的高亮行在顶部。
constexpr std::size_t ExpandedGridFirstRow(std::size_t firstRow, std::size_t cursorRow, std::size_t rows,
                                           std::size_t visibleRows)
{
    if (rows == 0 || visibleRows == 0)
        return 0;
    if (visibleRows > rows)
        visibleRows = rows;
    if (cursorRow >= rows)
        cursorRow = rows - 1;
    if (cursorRow < firstRow)
        firstRow = cursorRow;
    if (cursorRow >= firstRow + visibleRows)
        firstRow = cursorRow + 1 - visibleRows;
    return firstRow < rows ? firstRow : rows - 1;
}
} // namespace msime::candidate

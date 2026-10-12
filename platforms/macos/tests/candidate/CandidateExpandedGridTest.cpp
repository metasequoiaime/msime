#include "candidate/CandidateExpandedGrid.h"

#include <cassert>

using msime::mac::ExpandedGridFirstRow;
using msime::mac::ExpandedGridMove;
using msime::mac::ExpandedGridRows;
using msime::mac::StepExpandedGrid;

int main()
{
    // 23 个候选、每行 5 个：5 行，最后一行只有 3 个（20、21、22）。
    constexpr std::size_t count = 23, columns = 5, pageRows = 2;
    static_assert(ExpandedGridRows(count, columns) == 5);
    static_assert(ExpandedGridRows(0, columns) == 0);

    // ←/→ 只在行内移动，到行首、行尾停住，不换到相邻的行。
    assert(StepExpandedGrid(6, count, columns, pageRows, ExpandedGridMove::Left).cursor == 5);
    assert(StepExpandedGrid(5, count, columns, pageRows, ExpandedGridMove::Left).cursor == 5);
    assert(StepExpandedGrid(8, count, columns, pageRows, ExpandedGridMove::Right).cursor == 9);
    assert(StepExpandedGrid(9, count, columns, pageRows, ExpandedGridMove::Right).cursor == 9);
    // 最后一行的行尾是最后一个候选，不是这一行本该有的第五列。
    assert(StepExpandedGrid(22, count, columns, pageRows, ExpandedGridMove::Right).cursor == 22);

    // ↑/↓ 在行之间移动并保持列；目标行短时停在它的最后一个。
    assert(StepExpandedGrid(7, count, columns, pageRows, ExpandedGridMove::Down).cursor == 12);
    assert(StepExpandedGrid(19, count, columns, pageRows, ExpandedGridMove::Down).cursor == 22);
    assert(StepExpandedGrid(22, count, columns, pageRows, ExpandedGridMove::Down).cursor == 22);
    assert(!StepExpandedGrid(22, count, columns, pageRows, ExpandedGridMove::Down).close);
    assert(StepExpandedGrid(12, count, columns, pageRows, ExpandedGridMove::Up).cursor == 7);
    assert(!StepExpandedGrid(12, count, columns, pageRows, ExpandedGridMove::Up).close);

    // 第一行按 ↑ 收起网格，光标不动。
    const auto closing = StepExpandedGrid(3, count, columns, pageRows, ExpandedGridMove::Up);
    assert(closing.close && closing.cursor == 3);

    // 翻页键一次走 pageRows 行，到头为止，不收起。
    assert(StepExpandedGrid(2, count, columns, pageRows, ExpandedGridMove::PageDown).cursor == 12);
    assert(StepExpandedGrid(14, count, columns, pageRows, ExpandedGridMove::PageDown).cursor == 22);
    assert(StepExpandedGrid(12, count, columns, pageRows, ExpandedGridMove::PageUp).cursor == 2);
    assert(StepExpandedGrid(7, count, columns, pageRows, ExpandedGridMove::PageUp).cursor == 2);
    assert(!StepExpandedGrid(2, count, columns, pageRows, ExpandedGridMove::PageUp).close);

    // 越界的光标先夹到最后一个；空网格只有 ↑ 收起。
    assert(StepExpandedGrid(99, count, columns, pageRows, ExpandedGridMove::Left).cursor == 21);
    assert(StepExpandedGrid(0, 0, columns, pageRows, ExpandedGridMove::Up).close);
    assert(!StepExpandedGrid(0, 0, columns, pageRows, ExpandedGridMove::Down).close);

    // 可见窗口：光标在里面时不滚，出了上沿或下沿滚到刚好看见，末尾不留空行。
    static_assert(ExpandedGridFirstRow(0, 2, 8, 5) == 0);
    static_assert(ExpandedGridFirstRow(0, 5, 8, 5) == 1);
    static_assert(ExpandedGridFirstRow(3, 1, 8, 5) == 1);
    static_assert(ExpandedGridFirstRow(6, 7, 8, 5) == 3);
    static_assert(ExpandedGridFirstRow(4, 0, 3, 5) == 0);
    static_assert(ExpandedGridFirstRow(0, 0, 0, 5) == 0);
    return 0;
}

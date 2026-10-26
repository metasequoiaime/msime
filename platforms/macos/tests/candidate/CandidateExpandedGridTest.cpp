#include "../../../../shared/candidate/CandidateExpandedGrid.h"
#include "../../src/candidate/AdaptiveCandidateLayout.h"

#include <cassert>

using msime::candidate::ExpandedGridFirstRow;
using msime::candidate::ExpandedGridMove;
using msime::candidate::ExpandedGridRows;
using msime::candidate::StepExpandedGrid;

int main()
{
    const auto collapsed = msime::mac::LayoutAdaptiveCandidates({40, 40, 40, 200, 500, 20}, 300, 5);
    assert((collapsed.starts == std::vector<std::size_t>{0, 3, 4, 5, 6}));
    assert(collapsed.widths[2][0] == 300);
    const auto many = msime::mac::LayoutAdaptiveCandidates(std::vector<double>(30, 10), 400, 0);
    assert(many.widths.size() == 1 && many.widths[0].size() == 30);
    std::vector<double> numbers(300, 15);
    std::fill_n(numbers.begin(), 9, 5);
    std::fill(numbers.begin() + 9, numbers.begin() + 99, 10);
    const auto numbered = msime::mac::LayoutAdaptiveCandidates(std::vector<double>(300, 30), 120, 0, 0, numbers);
    assert(numbered.starts[1] == 3 && numbered.widths[0][0] == 35 && numbered.widths[1][0] == 35);
    const auto twoDigits = msime::mac::LayoutAdaptiveCandidates(std::vector<double>(30, 10), 200, 0, 0, numbers);
    assert(twoDigits.starts[1] == 10 && twoDigits.widths[0][0] == 20);
    const auto numberBoundary = msime::mac::LayoutAdaptiveCandidates(std::vector<double>(30, 10), 195, 0, 0, numbers);
    assert(numberBoundary.starts[1] == 9 && numberBoundary.widths[0][0] == 15);
    const auto minimum = msime::mac::LayoutAdaptiveCandidates({10, 40}, 100, 0, 0, numbers, 30);
    assert(minimum.widths[0][0] == 30 && minimum.widths[0][1] == 45);
    const auto expanded = msime::mac::LayoutAdaptiveCandidates({40, 90, 40, 40, 40, 200, 40, 40}, 300, 0, 6);
    assert((expanded.starts == std::vector<std::size_t>{0, 5, 8}));
    assert(expanded.widths[0][0] == 50 && expanded.widths[0][1] == 100);
    assert(expanded.rowFor(4) == 0 && expanded.rowFor(5) == 1);
    assert(msime::mac::StepAdaptiveCandidates(expanded, 2, 5, ExpandedGridMove::Down, 0).cursor == 5);
    assert(msime::mac::StepAdaptiveCandidates(expanded, 6, 5, ExpandedGridMove::Up, 0).cursor == 3);
    assert(msime::mac::StepAdaptiveCandidates(expanded, 0, 5, ExpandedGridMove::Up, 0).close);
    assert(msime::mac::LayoutAdaptiveCandidates({}, 0, 0).widths.empty());
    // 按需分页：停在含 `through` 的那一行之后，已排的行与全量排版逐行一致。
    const std::vector<double> lazyWidths{40, 40, 40, 200, 500, 20, 30, 30};
    const auto full = msime::mac::LayoutAdaptiveCandidates(lazyWidths, 300, 5);
    assert(!full.more && full.knownPages() == full.widths.size());
    const auto firstRow = msime::mac::LayoutAdaptiveCandidates(lazyWidths, 300, 5, 0, {}, 0, 0);
    assert((firstRow.starts == std::vector<std::size_t>{0, 3}) && firstRow.more && firstRow.knownPages() == 2);
    assert(firstRow.widths[0] == full.widths[0] && firstRow.rowFor(2) == 0);
    const auto third = msime::mac::LayoutAdaptiveCandidates(lazyWidths, 300, 5, 0, {}, 0, 4);
    assert((third.starts == std::vector<std::size_t>{0, 3, 4, 5}) && third.more && third.rowFor(4) == 2);
    assert(third.widths[2] == full.widths[2]);
    // 含 `through` 的是最后一行时没有下一页。
    const auto last = msime::mac::LayoutAdaptiveCandidates(lazyWidths, 300, 5, 0, {}, 0, 6);
    assert(!last.more && last.starts == full.starts);
    // 至少排够 `minimumRows` 行才停：展开网格要排满视口，不只是光标那一行。
    const auto window = msime::mac::LayoutAdaptiveCandidates(lazyWidths, 300, 5, 0, {}, 0, 0, 3);
    assert((window.starts == std::vector<std::size_t>{0, 3, 4, 5}) && window.more && window.widths[2] == full.widths[2]);
    const auto beyond = msime::mac::LayoutAdaptiveCandidates(lazyWidths, 300, 5, 0, {}, 0, 0, 10);
    assert(!beyond.more && beyond.starts == full.starts);
    // 等宽列的展开网格同样按需：前几行与全量一致。
    const auto gridFull = msime::mac::LayoutAdaptiveCandidates(std::vector<double>(40, 45), 295, 5, 6);
    const auto gridLazy = msime::mac::LayoutAdaptiveCandidates(std::vector<double>(40, 45), 295, 5, 6, {}, 0, 0, 2);
    assert(gridLazy.more && gridLazy.widths.size() == 2 &&
           std::equal(gridLazy.starts.begin(), gridLazy.starts.end(), gridFull.starts.begin()));
    // 只给前缀时停不下来：最后一行未满，也没有 `more`，由调用方补量。
    const auto prefix = msime::mac::LayoutAdaptiveCandidates({40, 40}, 300, 5, 0, {}, 0, 0);
    assert(!prefix.more && (prefix.starts == std::vector<std::size_t>{0, 2}));
    const auto exact = msime::mac::LayoutAdaptiveCandidates({45, 45, 45, 45, 45, 45, 45}, 295, 5, 6);
    assert(exact.starts[1] == 6);
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

    // 可见窗口：光标在里面时不滚，出了上沿或下沿滚到刚好看见，末页不向前补行。
    static_assert(ExpandedGridFirstRow(0, 2, 8, 5) == 0);
    static_assert(ExpandedGridFirstRow(0, 5, 8, 5) == 1);
    static_assert(ExpandedGridFirstRow(3, 1, 8, 5) == 1);
    static_assert(ExpandedGridFirstRow(6, 7, 8, 5) == 6);
    static_assert(ExpandedGridFirstRow(4, 0, 3, 5) == 0);
    static_assert(ExpandedGridFirstRow(0, 0, 0, 5) == 0);
    static_assert(ExpandedGridFirstRow(4, 4, 5, 5) == 4);
    return 0;
}

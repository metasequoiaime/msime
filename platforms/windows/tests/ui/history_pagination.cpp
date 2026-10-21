#include "../../settings/HistoryPagination.h"
#include <cstdio>
#include <stdexcept>
#include <string>

using namespace msime::settings;
void require_at(bool value, int line) {
  if (!value)
    throw std::runtime_error("History pagination test failed at line " + std::to_string(line));
}
#define require(...) require_at((__VA_ARGS__), __LINE__)

int main() {
  try {
    // 没有记录：一页、不翻页，状态仍是第 1 / 1 页。
    auto empty = history_page(0, 0);
    require(empty.offset == 0 && empty.count == 0 && empty.last_offset == 0);
    require(!empty.paged() && !empty.has_previous() && !empty.has_next());
    require(history_page_status(empty) == L"第 1 / 1 页 · 共 0 条");

    // 正好一页不画翻页，多一条才画。
    require(!history_page(20, 0).paged());
    require(history_page(20, 0).count == 20);
    require(history_page(21, 0).paged());

    // 45 条分成 20／20／5，与共享页的验收相同。
    auto first = history_page(45, 0);
    require(first.count == 20 && first.last_offset == 40);
    require(!first.has_previous() && first.has_next());
    require(first.next_offset() == 20);
    require(history_page_status(first) == L"第 1 / 3 页 · 共 45 条");
    auto second = history_page(45, first.next_offset());
    require(second.offset == 20 && second.count == 20);
    require(second.has_previous() && second.has_next());
    require(second.previous_offset() == 0 && second.next_offset() == 40);
    auto last = history_page(45, second.next_offset());
    require(last.offset == 40 && last.count == 5);
    require(last.has_previous() && !last.has_next());
    require(last.next_offset() == 40);
    require(history_page_status(last) == L"第 3 / 3 页 · 共 45 条");

    // 删掉末页最后一条：41 条时第 3 页还在；剩 40 条时起点退到最后一页（第 2 页）。
    require(history_page(41, 40).offset == 40 && history_page(41, 40).count == 1);
    auto shrunk = history_page(40, 40);
    require(shrunk.offset == 20 && shrunk.count == 20);
    require(history_page_status(shrunk) == L"第 2 / 2 页 · 共 40 条");
    // 普通删除保留当前页。
    require(history_page(44, 20).offset == 20);
    // 清空后回到第一页。
    require(history_page(0, 40).offset == 0);

    // 不在页首的起点（不会出现，但不越界）。
    auto odd = history_page(45, 33);
    require(odd.offset == 33 && odd.count == 12 && odd.previous_offset() == 13);

    // 每页条数可以换，0 当作 1。
    require(history_page(3, 5, 0).offset == 2 && history_page(3, 5, 0).count == 1);
    require(history_page(1000, 999).offset == 980 && history_page(1000, 999).count == 20);
  } catch (const std::exception &error) {
    std::fputs(error.what(), stderr);
    std::fputs("\n", stderr);
    return 1;
  }
  return 0;
}

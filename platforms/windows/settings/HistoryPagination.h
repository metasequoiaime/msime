#pragma once
#include <algorithm>
#include <cstddef>
#include <string>

// 「剪贴板」页「历史记录」标签的分页，照共享设置 UI 的 `clipboard-history-section.tsx`：每页 20 条，`lastOffset` 是最后一页的起点，记下的起点超过它时退到最后一页（删掉末页最后一条之后），状态写「第 N / M 页 · 共 X 条」，只有超过一页时才画翻页。翻页只改显示，不重读历史。不依赖 Windows 头文件，主机上的单元测试能直接查。
namespace msime::settings {

inline constexpr std::size_t clipboard_history_page_size = 20;

struct HistoryPage {
  // 这一页第一条在整个列表里的下标，已收进有效范围。
  std::size_t offset = 0;
  // 这一页画几条。
  std::size_t count = 0;
  // 最后一页的起点（共享页的 `lastOffset`）。
  std::size_t last_offset = 0;
  std::size_t total = 0;
  std::size_t page_size = clipboard_history_page_size;

  bool has_previous() const { return offset > 0; }
  bool has_next() const { return offset < last_offset; }
  // 超过一页才画翻页，与共享页的 `entries.length > HISTORY_PAGE_SIZE` 相同。
  bool paged() const { return total > page_size; }
  std::size_t previous_offset() const { return offset > page_size ? offset - page_size : 0; }
  std::size_t next_offset() const { return std::min(offset + page_size, last_offset); }
};

// `total` 条记录里从 `requested_offset` 起的那一页；`page_size` 为 0 时当作 1。
constexpr HistoryPage history_page(std::size_t total, std::size_t requested_offset,
                                   std::size_t page_size = clipboard_history_page_size) {
  HistoryPage page;
  page.page_size = page_size == 0 ? 1 : page_size;
  page.total = total;
  const std::size_t pages = (total + page.page_size - 1) / page.page_size;
  page.last_offset = (pages == 0 ? 0 : pages - 1) * page.page_size;
  page.offset = std::min(requested_offset, page.last_offset);
  page.count = std::min(page.page_size, total - std::min(total, page.offset));
  return page;
}

// 翻页器中间的状态，逐字照共享页：`第 N / M 页 · 共 X 条`。
inline std::wstring history_page_status(HistoryPage const &page) {
  return L"第 " + std::to_wstring(page.offset / page.page_size + 1) + L" / " +
         std::to_wstring(page.last_offset / page.page_size + 1) + L" 页 · 共 " +
         std::to_wstring(page.total) + L" 条";
}

} // namespace msime::settings

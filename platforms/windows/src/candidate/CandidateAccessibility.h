#pragma once
#include "AccessibleElements.h"
#include "CandidateCardSize.h"
#include "CandidatePresentation.h"
#include <algorithm>
#include <optional>
#include <string>
#include <vector>

namespace msime::windows {
// 候选窗交给读屏的元素，名称与 macOS 候选窗的 accessibilityLabel 一致（InputController.mm）：每行「序号  候选」、页码「第 N 页，共 M 页」、翻页箭头「上一页候选」「下一页候选」、预编辑「候选窗预编辑」、logo 是产品名。

// 元素 id：候选行是序号（1 到 9），其余是固定的编号，换页、换快照都不变。
inline constexpr int candidate_accessible_logo = 100;
inline constexpr int candidate_accessible_preedit = 101;
inline constexpr int candidate_accessible_page = 102;
inline constexpr int candidate_accessible_previous = 103;
inline constexpr int candidate_accessible_next = 104;
inline constexpr const char *candidate_accessible_name = "候选窗";

// 卡片坐标到客户区像素：卡片左上角在画布里的位置（阴影边距和装饰图让出的高度），乘上布局缩放，与悬停提示的区域、点击换算同一套。
struct CandidateAccessibleFrame {
  double card_left = 0.0, card_top = 0.0, card_width = 0.0, scale = 1.0;
};

// 一行候选读出来的样子：候选文字（带纠错星号和徽标），跟着注释（五笔的辅助码、引擎的注释）和韩文汉字的훈음，与 macOS 的 CandidateDisplayWithWubiHint 一样把它们连在候选后面。
inline std::string candidate_accessible_display(const PresentationCandidate &candidate) {
  auto text = candidate_primary_text(candidate);
  for (const auto *run : {&candidate.annotation, &candidate.gloss})
    if (!run->empty())
      text += " " + *run;
  return text;
}

// 按刚画好的页算出元素树。`clickable` 和 `pageable` 是窗口有没有点选、翻页的回调；`pointer_input` 为 false 的列表（只能用键盘选）和鼠标一样不能执行。`product` 是这个版本的产品名（UTF-8）。
inline AccessibleTree candidate_accessible_tree(const CandidatePresentation &value,
                                                const std::vector<CandidateRowLayout> &rows,
                                                const std::optional<CandidatePagerLayout> &pager,
                                                const CandidateCardMetrics &metrics,
                                                bool show_preedit,
                                                const CandidateAccessibleFrame &frame,
                                                bool clickable, bool pageable,
                                                const std::string &product) {
  AccessibleTree tree{AccessibleContainer::List, candidate_accessible_name, {}};
  const auto client = [&frame](const CandidateRowBounds &box) {
    return AccessibleBounds{(frame.card_left + box.left) * frame.scale,
                            (frame.card_top + box.top) * frame.scale,
                            (frame.card_left + box.right) * frame.scale,
                            (frame.card_top + box.bottom) * frame.scale};
  };
  if (metrics.logo_visible) {
    AccessibleElement logo;
    logo.id = candidate_accessible_logo;
    logo.role = AccessibleRole::Image;
    logo.automation_id = "candidate-logo";
    logo.name = product;
    logo.bounds = client(candidate_logo_bounds(metrics));
    tree.elements.push_back(std::move(logo));
  }
  // 与 macOS 一样，只有拼音非空时才有预编辑这一项；拼音本身是它的值。
  if (show_preedit && !value.preedit.empty()) {
    AccessibleElement preedit;
    preedit.id = candidate_accessible_preedit;
    preedit.role = AccessibleRole::Text;
    preedit.automation_id = "candidate-preedit";
    preedit.name = "候选窗预编辑";
    preedit.value = value.preedit;
    const double right = pager ? pager->left - metrics.pager_gap
                               : frame.card_width - metrics.pad_x / 2.0;
    preedit.bounds = client({candidate_preedit_left(metrics), metrics.pad_y, right,
                             metrics.pad_y + metrics.preedit_row});
    tree.elements.push_back(std::move(preedit));
  }
  if (pager && value.page < value.page_count) {
    AccessibleElement page;
    page.id = candidate_accessible_page;
    page.role = AccessibleRole::Text;
    page.automation_id = "candidate-page-indicator";
    page.name = "第 " + std::to_string(value.page + 1) + " 页，共 " +
                std::to_string(value.page_count) + " 页";
    page.bounds = client(pager->indicator);
    tree.elements.push_back(std::move(page));
    // 上一页在第一页上不可用，和画出来变暗、点了不翻页一致；下一页总是可用，因为引擎按需取候选，页数会随翻页增长。
    const bool live = pageable && value.pointer_input;
    for (const bool previous : {true, false}) {
      AccessibleElement arrow;
      arrow.id = previous ? candidate_accessible_previous : candidate_accessible_next;
      arrow.role = AccessibleRole::Button;
      arrow.automation_id = previous ? "candidate-page-previous" : "candidate-page-next";
      arrow.name = previous ? "上一页候选" : "下一页候选";
      arrow.enabled = !previous || value.page > 0;
      arrow.invokable = live && arrow.enabled;
      arrow.bounds = client(previous ? pager->previous : pager->next);
      tree.elements.push_back(std::move(arrow));
    }
  }
  const size_t count = (std::min)(rows.size(), value.candidates.size());
  for (size_t slot = 0; slot < count; ++slot) {
    const auto &candidate = value.candidates[slot];
    AccessibleElement row;
    row.id = static_cast<int>(slot + 1);
    row.role = AccessibleRole::ListItem;
    row.automation_id = "candidate-" + std::to_string(slot + 1);
    row.name = std::to_string(slot + 1) + "  " + candidate_accessible_display(candidate);
    // 补充说明与悬停提示同一段文字：候选全文，有释义时下一行是释义。
    row.help = candidate_tooltip_text(candidate);
    row.invokable = clickable && value.pointer_input;
    row.bounds = client(rows[slot].bounds);
    tree.elements.push_back(std::move(row));
  }
  return tree;
}
} // namespace msime::windows

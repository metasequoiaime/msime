// 候选窗交给读屏（UI Automation）的元素树。名称与 macOS 候选窗的 accessibilityLabel 一致：每行「序号  候选」、页码「第 N 页，共 M 页」、翻页箭头「上一页候选」「下一页候选」、预编辑「候选窗预编辑」、logo 是产品名。
#include "CandidateAccessibility.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Candidate accessibility check failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)

PresentationCandidate candidate(size_t index, const std::string &text) {
  PresentationCandidate value{};
  value.session = 1;
  value.generation = 2;
  value.index = index;
  value.text = text;
  value.highlighted = index == 0;
  return value;
}
const AccessibleElement &element(const AccessibleTree &tree, int id) {
  const auto *found = accessible_element(tree, id);
  if (!found)
    throw std::runtime_error("Missing candidate element " + std::to_string(id));
  return *found;
}
} // namespace

int main() {
  try {
    CandidatePresentation value{};
    value.visible = true;
    value.preedit = "ni hao";
    value.page = 1;
    value.page_count = 3;
    value.candidates = {candidate(0, "你好"), candidate(1, "拟好")};
    // 五笔的辅助码和引擎注释跟在候选后面，和 macOS 的 CandidateDisplayWithWubiHint 一样；纠错的候选带星号。
    value.candidates[1].annotation = "rv";
    value.candidates[1].corrected = true;
    value.candidates[0].translation = "hello";
    const auto metrics = candidate_card_metrics(16.0, 16.0, true, true, true);
    const auto pager = candidate_pager_layout(240.0, 30.0, metrics);
    require(pager);
    std::vector<CandidateRowLayout> rows(2);
    rows[0].bounds = {6.0, 40.0, 234.0, 70.0};
    rows[1].bounds = {6.0, 72.0, 234.0, 100.0};
    // 卡片在画布里偏移 (8, 20)，布局缩放 1.5。
    const CandidateAccessibleFrame frame{8.0, 20.0, 240.0, 1.5};
    const auto tree = candidate_accessible_tree(value, rows, pager, metrics, true, frame, true,
                                                true, "水杉输入法");
    require(tree.container == AccessibleContainer::List && tree.name == "候选窗");
    // 自上而下、自左而右：logo、预编辑、页码、两个箭头，然后是候选行。
    require(tree.elements.size() == 7);
    require(tree.elements[0].id == candidate_accessible_logo);
    require(tree.elements[1].id == candidate_accessible_preedit);
    require(tree.elements[2].id == candidate_accessible_page);
    require(tree.elements[3].id == candidate_accessible_previous);
    require(tree.elements[4].id == candidate_accessible_next);
    require(tree.elements[5].id == 1 && tree.elements[6].id == 2);

    const auto &logo = element(tree, candidate_accessible_logo);
    require(logo.role == AccessibleRole::Image && logo.name == "水杉输入法");
    require(logo.automation_id == "candidate-logo");
    const auto mark = candidate_logo_bounds(metrics);
    require(logo.bounds.left == (8.0 + mark.left) * 1.5 && logo.bounds.top == (20.0 + mark.top) * 1.5);

    // 预编辑的名字固定，正在输入的拼音是它的值；它从 logo 之后开始，到页码之前为止。
    const auto &preedit = element(tree, candidate_accessible_preedit);
    require(preedit.role == AccessibleRole::Text && preedit.name == "候选窗预编辑");
    require(preedit.value == std::optional<std::string>("ni hao"));
    require(preedit.bounds.left == (8.0 + candidate_preedit_left(metrics)) * 1.5);
    require(preedit.bounds.right == (8.0 + pager->left - metrics.pager_gap) * 1.5);

    const auto &page = element(tree, candidate_accessible_page);
    require(page.name == "第 2 页，共 3 页" && page.automation_id == "candidate-page-indicator");
    require(page.bounds.left == (8.0 + pager->indicator.left) * 1.5);

    const auto &previous = element(tree, candidate_accessible_previous);
    const auto &next = element(tree, candidate_accessible_next);
    require(previous.role == AccessibleRole::Button && previous.name == "上一页候选");
    require(next.role == AccessibleRole::Button && next.name == "下一页候选");
    require(previous.enabled && previous.invokable && next.enabled && next.invokable);
    require(next.bounds.right == (8.0 + pager->next.right) * 1.5);

    // 候选行的名字是「序号  候选」，补充说明是悬停提示那段文字（候选全文和释义）。
    const auto &first = element(tree, 1);
    require(first.role == AccessibleRole::ListItem && first.name == "1  你好");
    require(first.help == candidate_tooltip_text(value.candidates[0]));
    require(first.help == "你好\nhello");
    require(first.invokable && first.automation_id == "candidate-1");
    require(first.bounds.top == (20.0 + 40.0) * 1.5 && first.bounds.right == (8.0 + 234.0) * 1.5);
    require(element(tree, 2).name == "2  拟好* rv");

    // 第一页上的上一页箭头和画出来的一样不可用；下一页总是可用，因为页数会随翻页增长。
    value.page = 0;
    const auto first_page = candidate_accessible_tree(value, rows, pager, metrics, true, frame,
                                                      true, true, "水杉输入法");
    require(!element(first_page, candidate_accessible_previous).enabled);
    require(!element(first_page, candidate_accessible_previous).invokable);
    require(element(first_page, candidate_accessible_next).invokable);
    require(element(first_page, candidate_accessible_page).name == "第 1 页，共 3 页");

    // 只能用键盘选的列表：鼠标点不了，读屏也执行不了；名字照样读得到。
    value.pointer_input = false;
    const auto keyboard_only = candidate_accessible_tree(value, rows, pager, metrics, true, frame,
                                                         true, true, "水杉输入法");
    require(!element(keyboard_only, 1).invokable);
    require(!element(keyboard_only, candidate_accessible_next).invokable);
    require(element(keyboard_only, 1).name == "1  你好");
    value.pointer_input = true;

    // 没有翻页回调时箭头读得到、执行不了；没有点选回调时候选行执行不了。
    const auto inert = candidate_accessible_tree(value, rows, pager, metrics, true, frame, false,
                                                 false, "水杉输入法");
    require(!element(inert, candidate_accessible_next).invokable);
    require(!element(inert, 1).invokable);

    // logo 关掉、拼音隐藏、没有翻页时只剩候选行。
    const auto bare_metrics = candidate_card_metrics(16.0, 16.0, false, false, false);
    const auto bare = candidate_accessible_tree(value, rows, std::nullopt, bare_metrics, false,
                                                frame, true, true, "水杉输入法");
    require(bare.elements.size() == 2 && bare.elements[0].id == 1);
    // 拼音为空时没有预编辑这一项，和 macOS 只在拼音非空时加预编辑一致。
    value.preedit.clear();
    const auto empty_preedit = candidate_accessible_tree(value, rows, pager, metrics, true, frame,
                                                         true, true, "水杉输入法");
    require(!accessible_element(empty_preedit, candidate_accessible_preedit));
    // 行比候选少（正在重建）时只报告两边都有的那几行。
    value.candidates.push_back(candidate(2, "泥号"));
    const auto short_rows = candidate_accessible_tree(value, rows, pager, metrics, true, frame,
                                                      true, true, "水杉输入法");
    require(!accessible_element(short_rows, 3));
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

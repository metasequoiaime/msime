// 皮肤的卷轴展开（expand_panel），照 msime-windows c0a9d371 的 skins/README.md「卷轴展开」与 candidate_presenter.cpp、candidate_skin_scene.cpp：按钮那一窄列、等宽网格、一屏的高度、滚动条、点击、滚轮、批次和收起、回到顶部的规则。数值是合成的。
#include "CandidateExpandPanel.h"
#include <cmath>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Candidate expand panel check failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
bool near(double value, double expected) { return std::fabs(value - expected) < 0.001; }
CandidateSkinMetrics expand_skin(int rows = 4) {
  CandidateSkinLayout layout;
  layout.expand_panel = true;
  layout.expand_rows = rows;
  return candidate_skin_metrics(layout, true, 16.0);
}
std::vector<CandidateItemWidths> page(size_t count, double text = 32.0) {
  return std::vector<CandidateItemWidths>(count, CandidateItemWidths{text, 0.0, 0.0});
}
bool same_bounds(const CandidateRowBounds &a, const CandidateRowBounds &b) {
  return near(a.left, b.left) && near(a.top, b.top) && near(a.right, b.right) &&
         near(a.bottom, b.bottom);
}
} // namespace

int main() {
  // 不写 expand_panel 的皮肤（以及竖排）和以前一样：没有按钮那一列，翻页照旧。
  {
    const auto plain = candidate_card_metrics(16.0, 16.0, true, true, true, {});
    CandidateSkinLayout off;
    off.expand_panel = false;
    const auto disabled =
        candidate_card_metrics(16.0, 16.0, true, true, true, candidate_skin_metrics(off, true, 16.0));
    CandidateSkinLayout on;
    on.expand_panel = true;
    const auto vertical =
        candidate_card_metrics(16.0, 16.0, true, true, true, candidate_skin_metrics(on, false, 16.0));
    for (const auto *metrics : {&disabled, &vertical}) {
      require(!metrics->expand_panel && near(metrics->slack_x, plain.slack_x) &&
              near(metrics->preedit_row, plain.preedit_row) &&
              near(candidate_list_arrow_column(*metrics), 0.0));
    }
  }

  // 按钮那一窄列：边长 round(1.1 * 16) = 18，左右外边距 2 和 1，竖直居中在第一行上；首行不为翻页留高度，page_arrows 被忽略。
  {
    CandidateSkinLayout layout;
    layout.expand_panel = true;
    layout.page_arrows = true;
    const auto skin = candidate_skin_metrics(layout, true, 16.0);
    require(skin.expand_panel && !skin.page_arrows && skin.expand_rows == 4);
    const auto metrics = candidate_card_metrics(16.0, 16.0, false, false, true, skin);
    require(metrics.expand_panel && !metrics.list_arrows);
    const auto &panel = *metrics.expand_panel;
    require(near(panel.button, 18.0) && near(panel.glyph, 10.0) && near(panel.column, 21.0) &&
            near(panel.cell_min, 42.0) && panel.rows == 4 && near(metrics.slack_x, 0.0));
    require(near(panel.margin_top, (metrics.candidate_row - 18.0) * 0.5));
    require(near(candidate_list_arrow_column(metrics), 21.0));
    // 拼音隐藏、不画 logo 时首行不占高度，即使这一页有翻页可画。
    require(near(metrics.preedit_row, 0.0));
    // expand_rows 越界时夹在 2-8。
    CandidateSkinLayout many = layout;
    many.expand_rows = 12;
    require(candidate_skin_metrics(many, true, 16.0).expand_rows == 8);
  }

  const auto metrics = candidate_card_metrics(16.0, 16.0, true, false, false, expand_skin());
  const auto &panel = *metrics.expand_panel;
  const auto items = page(5);
  const double width = 400.0;

  // 收起：首行和普通的一页逐位相同，按钮贴在右上角，没有网格，一屏就是首行。
  {
    const auto layout = candidate_expand_layout(items, width, metrics, {}, 5);
    const auto rows = candidate_page_layout(items, width, metrics, true);
    require(layout.rows.size() == rows.size());
    for (size_t index = 0; index < rows.size(); ++index)
      require(same_bounds(layout.rows[index].bounds, rows[index].bounds));
    require(layout.cells.empty() && !layout.scrollbar && near(layout.max_offset(), 0.0));
    require(near(layout.top, metrics.pad_y + metrics.preedit_row));
    require(near(layout.right, width - metrics.pad_x / 2.0 - 21.0));
    require(near(layout.toggle.left, layout.right + 2.0) &&
            near(layout.toggle.right, width - metrics.pad_x / 2.0 - 1.0) &&
            near(layout.toggle.top, layout.top + panel.margin_top));
    require(near(layout.viewport, metrics.candidate_row) && near(layout.first_extent, layout.viewport));
    // 收起时的卡片和普通卡片一样大，只是多了按钮那一列。
    CandidateCardInput input;
    input.items = items;
    input.horizontal = true;
    input.skin = expand_skin();
    const auto collapsed = candidate_expand_card_size(input, {}, 5);
    const auto plain = candidate_card_size(input);
    require(near(collapsed.width, plain.width) && near(collapsed.height, plain.height));
  }

  // 展开：首行下面是等宽网格，列数是内容宽度能放下几格最小宽度（不超过每页候选数），宽的候选占几格，一行放不下就换行。
  {
    const double gap = metrics.item_gap, row = metrics.candidate_row;
    const std::vector<double> grid{20.0, 20.0, 100.0, 20.0, 20.0, 20.0};
    const auto layout = candidate_expand_layout(items, width, metrics, grid, 5);
    const double content = layout.right - layout.left;
    const auto columns = static_cast<size_t>(std::floor((content + gap) / (42.0 + gap)));
    require(layout.columns == (std::min)(columns, size_t{5}));
    const double cell = (content - gap * 4.0) / 5.0;
    require(layout.cells.size() == grid.size());
    require(near(layout.cells[0].left, layout.left) && near(layout.cells[0].right - layout.cells[0].left, cell));
    require(near(layout.cells[1].left, layout.cells[0].right + gap));
    // 第三个候选比一格宽，占两格。
    require(near(layout.cells[2].right - layout.cells[2].left, cell * 2.0 + gap));
    // 第一行四个候选占满五格，第五个换到下一行。
    require(near(layout.cells[3].top, layout.cells[0].top) &&
            near(layout.cells[3].right, layout.left + content) &&
            near(layout.cells[4].top, layout.cells[0].top + row + gap) &&
            near(layout.cells[4].left, layout.left) &&
            near(layout.cells[5].top, layout.cells[4].top));
    // 网格紧跟在首行下面，隔一段行距；内容只有两行网格，一屏放得下，没有滚动条。
    require(near(layout.cells[0].top, layout.top + layout.first_extent + gap));
    require(!layout.scrollbar && near(layout.content, layout.first_extent + gap + row * 2.0 + gap));
    require(near(layout.viewport, layout.content));
  }

  // 一屏是首行加 expand_rows 行网格；内容更高时出现滚动条，让出 10 DIP 后按变窄的宽度重排一次。
  const std::vector<double> long_grid(60, 20.0);
  const auto scrolling = candidate_expand_layout(items, width, metrics, long_grid, 5);
  {
    const double gap = metrics.item_gap, row = metrics.candidate_row;
    require(scrolling.scrollbar && near(scrolling.content_right, scrolling.right - 10.0));
    require(near(scrolling.viewport, scrolling.first_extent + gap + row * 4.0 + gap * 3.0));
    for (const auto &cell : scrolling.cells)
      require(cell.right <= scrolling.content_right + 0.5);
    require(scrolling.content > scrolling.viewport &&
            near(scrolling.max_offset(), scrolling.content - scrolling.viewport));
    // 两行的皮肤一屏只有两行网格。
    const auto two = candidate_card_metrics(16.0, 16.0, true, false, false, expand_skin(2));
    const auto short_screen = candidate_expand_layout(items, width, two, long_grid, 5);
    require(near(short_screen.viewport, short_screen.first_extent + gap + row * 2.0 + gap));
    // 卡片高度的上限压低一屏，但不低于首行；后续候选还在路上时沿用上一帧的高度，不先缩回去。
    const auto capped = candidate_expand_layout(items, width, metrics, long_grid, 5, 60.0);
    require(near(capped.viewport, 60.0));
    const auto tiny = candidate_expand_layout(items, width, metrics, long_grid, 5, 1.0);
    require(near(tiny.viewport, tiny.first_extent));
    const auto pending = candidate_expand_layout(items, width, metrics, {}, 5, 0.0, {}, 0.0,
                                                 scrolling.viewport);
    require(pending.cells.empty() && near(pending.viewport, scrolling.viewport));
  }

  // 展开且有一整行网格时，网格按卡片能有的最大宽度去排（msime-windows 按 maxW 量网格），卡片就是那么宽；高度是首行之上的部分加一屏。
  {
    CandidateCardInput input;
    input.items = items;
    input.horizontal = true;
    input.skin = expand_skin();
    input.max_width = 700.0;
    input.max_height = 600.0;
    const auto size = candidate_expand_card_size(input, long_grid, 5);
    require(near(size.width, 700.0));
    const auto layout = candidate_expand_layout(items, size.width, metrics, long_grid, 5,
                                                600.0 - metrics.pad_y - metrics.slack_y -
                                                    metrics.preedit_row);
    require(near(size.height,
                 metrics.pad_y + metrics.slack_y + metrics.preedit_row + layout.viewport));
    // 网格只有两个短候选时卡片不必那么宽：取首行和网格里更宽的那一个。
    const auto few = candidate_expand_card_size(input, {20.0, 20.0}, 5);
    require(few.width < 700.0 && few.width >= candidate_card_size(input).width - 0.001);
  }

  // 点击：按钮固定不动；首行和网格加上滚动偏移，只在看得见的那一段里算；滚动条在滚动区右侧那一条。
  {
    const auto &layout = scrolling;
    const auto toggle = candidate_expand_hit(layout, 30.0, layout.toggle.left + 1.0,
                                             layout.toggle.top + 1.0);
    require(toggle.part == CandidateExpandPart::toggle);
    const auto &first = layout.rows[0].bounds;
    require(candidate_expand_hit(layout, 0.0, first.left + 1.0, first.top + 1.0).part ==
            CandidateExpandPart::row);
    // 滚上去以后首行在滚动区上面，点不到。
    require(candidate_expand_hit(layout, layout.first_extent + 4.0, first.left + 1.0,
                                 first.top + 1.0)
                .part != CandidateExpandPart::row);
    const auto &cell = layout.cells[7];
    const double offset = cell.top - layout.top;
    const auto hit = candidate_expand_hit(layout, offset, cell.left + 1.0, layout.top + 1.0);
    require(hit.part == CandidateExpandPart::cell && hit.index == 7);
    require(candidate_expand_hit(layout, 0.0, layout.right - 2.0, layout.top + 5.0).part ==
            CandidateExpandPart::scrollbar);
    require(candidate_expand_hit(layout, 0.0, layout.left + 1.0, layout.bottom() + 1.0).part ==
            CandidateExpandPart::none);
    require(candidate_expand_hit(layout, 0.0, -1.0, layout.top + 1.0).part ==
            CandidateExpandPart::none);
  }

  // 滚动条：滑块按看得见的比例，不短于 20 DIP，贴右缘内缩 2 DIP；滚到底时滑块到底。拖动时滑块上缘跟着指针。
  {
    const auto &layout = scrolling;
    const auto top = candidate_expand_thumb(layout, 0.0);
    require(top && near(top->right, layout.right - 2.0) && near(top->right - top->left, 4.0) &&
            near(top->top, layout.top + 2.0));
    const double length = layout.viewport - 4.0;
    require(near(top->bottom - top->top,
                 (std::max)(length * layout.viewport / layout.content, 20.0)));
    const auto bottom = candidate_expand_thumb(layout, layout.max_offset());
    require(bottom && near(bottom->bottom, layout.bottom() - 2.0));
    require(near(candidate_expand_drag(layout, top->top, 0.0), 0.0));
    require(near(candidate_expand_drag(layout, layout.bottom() + 50.0, 0.0), layout.max_offset()));
    const double height = top->bottom - top->top;
    const double middle = layout.top + 2.0 + (length - height) * 0.5;
    require(near(candidate_expand_drag(layout, middle, 0.0), layout.max_offset() * 0.5));
    // 没有滚动条时没有滑块。
    require(!candidate_expand_thumb(candidate_expand_layout(items, width, metrics, {}, 5), 0.0));
  }

  // 状态：打开从顶部开始；换了候选（网格第一项变了）回到顶部、只排第一批；只往后追加时保持滚动位置。
  {
    CandidateExpandState state;
    require(!candidate_expand_sync(state, 5, "a", 10) && state.revealed == 0);
    candidate_expand_open(state);
    require(state.expanded && near(state.offset, 0.0));
    // 列表还没到时什么都不动。
    require(!candidate_expand_sync(state, 5, "a", 0) && state.revealed == 0);
    require(candidate_expand_sync(state, 5, "a", 700) && state.revealed == candidate_expand_batch);
    state.offset = 80.0;
    require(!candidate_expand_sync(state, 5, "a", 800) && near(state.offset, 80.0) &&
            state.revealed == candidate_expand_batch);
    // 翻了页或接着打字：第一项换了。
    require(candidate_expand_sync(state, 10, "b", 50) && near(state.offset, 0.0) &&
            state.revealed == 50);
    state.offset = 10.0;
    require(candidate_expand_sync(state, 10, "c", 50) && near(state.offset, 0.0));
    candidate_expand_collapse(state);
    require(!state.expanded && !state.identified && state.revealed == 0);
  }

  // 滚轮：一格滚一行，高精度滚轮按比例，到顶或到底停住；收起时不滚。
  {
    CandidateExpandState state;
    const double step = metrics.candidate_row + metrics.item_gap;
    require(!candidate_expand_scroll(state, -120, 120, step, 100.0));
    candidate_expand_open(state);
    require(!candidate_expand_scroll(state, 120, 120, step, 100.0) && near(state.offset, 0.0));
    require(candidate_expand_scroll(state, -120, 120, step, 100.0) && near(state.offset, step));
    require(candidate_expand_scroll(state, -40, 120, step, 100.0) &&
            near(state.offset, step + step / 3.0));
    require(candidate_expand_scroll(state, -1200, 120, step, 100.0) && near(state.offset, 100.0));
    require(!candidate_expand_scroll(state, -120, 120, step, 100.0));
    // 内容变短时偏移跟着收回来。
    require(candidate_expand_clamp(state, 40.0) && near(state.offset, 40.0));
    require(!candidate_expand_clamp(state, 40.0));
  }

  // 批次：滚到离底部一行半以内、还有没排出来的候选时再排一批，每批 300 个；内容不够一屏时立刻再排。
  {
    CandidateExpandState state;
    candidate_expand_open(state);
    require(candidate_expand_sync(state, 5, "a", 700) && state.revealed == 300);
    const double step = 24.0;
    require(!candidate_expand_wants_more(state, 1000.0, step, 700));
    state.offset = 1000.0 - step * 1.5;
    require(candidate_expand_wants_more(state, 1000.0, step, 700));
    require(candidate_expand_reveal(state, 700) && state.revealed == 600);
    require(candidate_expand_reveal(state, 700) && state.revealed == 700);
    require(!candidate_expand_reveal(state, 700) &&
            !candidate_expand_wants_more(state, 1000.0, step, 700));
    CandidateExpandState short_list;
    candidate_expand_open(short_list);
    candidate_expand_sync(short_list, 5, "a", 900);
    require(candidate_expand_wants_more(short_list, 0.0, step, 900));
  }

  // 收起的判断：Server 那边已经收起（候选窗关闭后先清掉，候选窗的轮询晚一步）、换了客户端或这一页不能展开时收起；没展开时不动。
  static_assert(!candidate_expand_stale(true, true, true, true));
  static_assert(candidate_expand_stale(true, false, true, true));
  static_assert(candidate_expand_stale(true, true, false, true));
  static_assert(candidate_expand_stale(true, true, true, false));
  static_assert(!candidate_expand_stale(false, false, false, false));

  // 沿用上一帧的高度只在这一代的列表还没到时：到了而后面一个候选也没有时按实际内容排，不留空白。
  static_assert(candidate_expand_held_viewport(true, false, 120.0) == 120.0);
  static_assert(candidate_expand_held_viewport(true, true, 120.0) == 0.0);
  static_assert(candidate_expand_held_viewport(false, false, 120.0) == 0.0);

  // 文字宽度缓存长过上限时只留眼下这一批，上限不小于这一批的两倍；没到上限时不动。
  {
    std::unordered_map<std::string, double> cache;
    std::vector<std::string> texts;
    for (size_t index = 0; index < 12; ++index) {
      texts.push_back("w" + std::to_string(index));
      cache.emplace(texts.back(), static_cast<double>(index));
    }
    std::vector<const std::string *> keep{&texts[2], &texts[3]};
    candidate_expand_trim_widths(cache, keep, 12);
    require(cache.size() == 12);
    candidate_expand_trim_widths(cache, keep, 10);
    require(cache.size() == 2 && near(cache.at("w3"), 3.0));
    // 这一批比上限大：上限按这一批的两倍算，不会每一帧都清掉。
    std::vector<const std::string *> big;
    for (const auto &text : texts)
      big.push_back(&text);
    for (const auto &text : texts)
      cache.emplace(text, 1.0);
    candidate_expand_trim_widths(cache, big, 4);
    require(cache.size() == 12);
  }

  // 滚轮交给谁：展开着时滚动、不看「鼠标滚轮翻页」开关；收起时开关打开才翻页，关着时交回系统。
  static_assert(candidate_wheel_route(true, false) == CandidateWheelRoute::scroll);
  static_assert(candidate_wheel_route(true, true) == CandidateWheelRoute::scroll);
  static_assert(candidate_wheel_route(false, true) == CandidateWheelRoute::page);
  static_assert(candidate_wheel_route(false, false) == CandidateWheelRoute::pass);

  // 网格从当前页最后一个候选之后开始。
  {
    const std::vector<ExpandedCandidate> list{{0, "a"}, {1, "b"}, {2, "c"}, {3, "d"}, {7, "e"}};
    require(candidate_expand_start(list, 2) == 3);
    require(candidate_expand_start(list, 4) == 4);
    require(candidate_expand_start(list, 7) == 5);
  }
  return 0;
}

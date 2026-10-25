#pragma once
#include "CandidateCardSize.h"
#include <string>
#include <unordered_map>

namespace msime::windows {
// 皮肤 `[candidate_window] expand_panel = true` 的卷轴展开，照 msime-windows c0a9d371 的 skins/README.md「卷轴展开」、candidate_presenter.cpp 与 candidate_skin_scene.cpp：横排候选行右端是「∨」按钮，点开后首行仍是当前页（带序号、数字键照常选），下面是后续候选的等宽网格，一屏是首行加 `expand_rows` 行网格，右侧一条细滚动条；滚轮让首行与网格一起滚，到顶或到底停住；点网格里的候选直接上屏；展开与收起只认按钮，按钮固定在右上角、不随内容滚动。这里只有几何、滚动、点击和批次的规则，不碰 Win32 与 Direct2D，坐标都是卡片坐标的 DIP。
//
// 后续候选来自 msime_client_all_candidates（一次复制这一代的全部候选），网格一次只排出一批：msime-windows 一次带出 300 个（event_listener_candidates.cpp 的 kExpandedCandidateLimit），滚到离底部一行半以内时再加一批。
inline constexpr size_t candidate_expand_batch = 300;

// 完整候选列表里的一项：在这一代里的下标（选它时交给 msime_client_select_any_candidate）和显示的文字（已按繁体输出换好）。
struct ExpandedCandidate {
  size_t index = 0;
  std::string text;
  unsigned source = 0;
};

// 网格里排在当前页之后的第一项在 items 里的位置：items 按下标递增，当前页最后一个候选的下标是 last。
inline size_t candidate_expand_start(const std::vector<ExpandedCandidate> &items, size_t last) {
  size_t position = 0;
  while (position < items.size() && items[position].index <= last)
    ++position;
  return position;
}

// 一帧的展开区：首行与网格所在的滚动区（不含按钮那一列），以及固定在右上角的按钮。rows 和 cells 是滚动偏移为 0 时的位置，画和点击时减去偏移。
struct CandidateExpandLayout {
  double left = 0.0, right = 0.0, top = 0.0;
  // 内容的右缘：出现滚动条时让出滚动条那一条。
  double content_right = 0.0;
  // 看得见的高度、内容的总高度、首行那一段的高度。
  double viewport = 0.0, content = 0.0, first_extent = 0.0;
  bool scrollbar = false;
  size_t columns = 0;
  std::vector<CandidateRowLayout> rows;
  std::vector<CandidateRowBounds> cells;
  CandidateRowBounds toggle{};
  // 滚动条的轨道宽和滑块的尺寸，从 CandidateCardMetrics::ExpandPanel 抄过来，点击和拖动用。
  double track = 0.0, thumb_width = 0.0, thumb_inset = 0.0, thumb_right_inset = 0.0,
         thumb_min = 0.0;
  double max_offset() const { return (std::max)(content - viewport, 0.0); }
  double bottom() const { return top + viewport; }
};

// 一格网格候选的自然宽度：和首行一样留出序号那一栏（网格没有序号，文字和首行的候选文字对齐），文字之后的余量与首行每列自带的相同（msime-windows 网格沿用首行的 CandidateList 外观，只是 label 为空）。
inline double candidate_expand_cell_width(double text, const CandidateCardMetrics &metrics) {
  return metrics.number_and_bar + text + metrics.item_padding_right +
         (std::max)(metrics.column_gap - metrics.item_gap, 0.0);
}

// 按卡片宽度排出展开区。grid 是这一批后续候选的文字宽度（候选字号），收起或还没拿到后续候选时为空，这时只有首行和按钮。max_viewport 大于 0 时限制看得见的高度（卡片不超过工作区的一半），minimum_viewport 让展开着、后续候选还在路上的那一帧保持上一帧的高度，卡片不先缩回一行再长回来。网格按 msime-windows CandidateList::Measure 的等宽网格排：内容宽度能放下几格最小宽度就分几列（不超过每页候选数），宽的候选占几格，一行放不下就换行；一屏是首行加 rows 行网格；内容比一屏高时右侧让出滚动条，按让出后的宽度重排一次。
inline CandidateExpandLayout
candidate_expand_layout(const std::vector<CandidateItemWidths> &items, double width,
                        const CandidateCardMetrics &metrics, const std::vector<double> &grid,
                        size_t page_size, double max_viewport = 0.0,
                        const CandidateWrapMeasure &wrapped = {},
                        double minimum_row_height = 0.0, double minimum_viewport = 0.0) {
  if (!metrics.expand_panel || !std::isfinite(width) || width <= 0.0)
    throw std::invalid_argument("Invalid candidate expand panel");
  const auto &panel = *metrics.expand_panel;
  CandidateExpandLayout layout;
  layout.left = metrics.pad_x / 2.0;
  layout.right = (std::max)(width - metrics.pad_x / 2.0 - panel.column, layout.left + 1.0);
  layout.top = metrics.pad_y + metrics.preedit_row;
  const double toggle_left = layout.right + panel.margin_left;
  layout.toggle = {toggle_left, layout.top + panel.margin_top, toggle_left + panel.button,
                   layout.top + panel.margin_top + panel.button};
  layout.track = panel.scrollbar;
  layout.thumb_width = panel.thumb_width;
  layout.thumb_inset = panel.thumb_inset;
  layout.thumb_right_inset = panel.thumb_right_inset;
  layout.thumb_min = panel.thumb_min;
  const double gap = metrics.item_gap, row = metrics.candidate_row;
  const auto arrange = [&](double content_width) {
    content_width = (std::max)(content_width, 1.0);
    // 首行就是普通的一页，排在 content_width 宽的一行里：candidate_page_layout 从 pad_x / 2 起排，并让出右侧那一窄列。
    layout.rows = candidate_page_layout(items, content_width + metrics.pad_x + panel.column,
                                        metrics, true, wrapped, minimum_row_height);
    double bottom = layout.top;
    bool visible = false;
    for (const auto &item : layout.rows)
      if (item.bounds.right > item.bounds.left) {
        bottom = (std::max)(bottom, item.bounds.bottom);
        visible = true;
      }
    // 空列表也留一行，与 candidate_card_size 一样。
    layout.first_extent = visible ? bottom - layout.top : (std::max)(row, minimum_row_height);
    layout.cells.clear();
    layout.columns = 0;
    double grid_height = 0.0;
    if (!grid.empty()) {
      const double cell_min = (std::max)(panel.cell_min, 1.0);
      const auto columns = static_cast<size_t>((std::clamp)(
          std::floor((content_width + gap) / (cell_min + gap)), 1.0,
          static_cast<double>((std::max)(page_size, size_t{1}))));
      const double cell = (std::max)(
          (content_width - gap * static_cast<double>(columns - 1)) / static_cast<double>(columns),
          1.0);
      const double grid_top = layout.top + layout.first_extent + gap;
      double x = 0.0, y = 0.0;
      layout.cells.reserve(grid.size());
      for (const double text : grid) {
        const double natural =
            candidate_expand_cell_width(std::isfinite(text) ? (std::max)(text, 0.0) : 0.0, metrics);
        const auto span = static_cast<size_t>((std::clamp)(
            std::ceil((natural + gap) / (cell + gap) - 0.001), 1.0, static_cast<double>(columns)));
        const double cell_width = (std::min)(
            cell * static_cast<double>(span) + gap * static_cast<double>(span - 1), content_width);
        // 等宽的格子是除出来的，末格可能因为浮点误差多出一点点，不能因此换行。
        if (x > 0.0 && x + cell_width > content_width + 0.5) {
          y += row + gap;
          x = 0.0;
        }
        layout.cells.push_back({layout.left + x, grid_top + y, layout.left + x + cell_width,
                                grid_top + y + row});
        x += cell_width + gap;
      }
      layout.columns = columns;
      grid_height = y + row;
    }
    layout.content = layout.first_extent + (layout.cells.empty() ? 0.0 : gap + grid_height);
    const double rows = static_cast<double>(panel.rows);
    const double cap = layout.first_extent + gap + rows * row + (rows - 1.0) * gap;
    double viewport = (std::min)(layout.content, cap);
    if (minimum_viewport > viewport && std::isfinite(minimum_viewport))
      viewport = (std::min)(minimum_viewport, cap);
    if (max_viewport > 0.0 && std::isfinite(max_viewport))
      viewport = (std::min)(viewport, (std::max)(max_viewport, layout.first_extent));
    layout.viewport = (std::max)(viewport, layout.first_extent);
  };
  arrange(layout.right - layout.left);
  if (layout.content > layout.viewport + 0.5) {
    layout.scrollbar = true;
    arrange(layout.right - layout.left - panel.scrollbar);
  }
  layout.content_right = layout.right - (layout.scrollbar ? panel.scrollbar : 0.0);
  return layout;
}

// 展开区的卡片尺寸。收起时与 candidate_card_size 相同（按钮那一窄列已经算在里面）。展开且有网格时，网格按卡片能有的最大宽度（工作区的一半）去排，和 msime-windows 按 maxW 量网格一样，所以有一整行网格时卡片就是那么宽；高度是首行之上的部分加一屏的展开区，不矮于按钮。
inline CandidateCardSize candidate_expand_card_size(const CandidateCardInput &input,
                                                    const std::vector<double> &grid,
                                                    size_t page_size,
                                                    double minimum_viewport = 0.0) {
  const auto collapsed = candidate_card_size(input);
  const auto shape = candidate_card_metrics(
      input.font_size, input.preedit_font_size, input.preedit_visible, input.logo_visible,
      input.page_width > 0.0, input.skin,
      std::isfinite(input.number_width) ? input.number_width : 0.0, input.line_height);
  if (!shape.expand_panel || !input.horizontal)
    return collapsed;
  const double reserved = candidate_reserved_row_height(
      shape, input.reserved_secondary_lines, input.reserved_secondary_height);
  double width = collapsed.width;
  const double width_cap = input.max_width > 1.0 ? (std::max)(input.max_width, width) : width;
  if (!grid.empty()) {
    const auto wide = candidate_expand_layout(input.items, width_cap, shape, grid, page_size, 0.0,
                                              input.wrapped, reserved);
    double extent = wide.left;
    for (const auto &cell : wide.cells)
      extent = (std::max)(extent, cell.right);
    for (const auto &row : wide.rows)
      extent = (std::max)(extent, row.bounds.right);
    const double natural = extent - wide.left + (wide.scrollbar ? wide.track : 0.0) +
                           shape.pad_x + candidate_list_arrow_column(shape);
    width = (std::min)((std::max)(width, natural), width_cap);
  }
  const double header = shape.pad_y + shape.slack_y + shape.preedit_row;
  const double max_viewport = input.max_height > 1.0 ? input.max_height - header : 0.0;
  const auto layout = candidate_expand_layout(input.items, width, shape, grid, page_size,
                                              max_viewport, input.wrapped, reserved,
                                              minimum_viewport);
  double height = header + (std::max)(layout.viewport, layout.toggle.bottom - layout.top);
  if (input.max_height > 1.0)
    height = (std::min)(height, input.max_height);
  return {width, (std::max)(height, 1.0)};
}

// 展开区里一点落在哪儿。按钮固定不动；首行和网格要加上滚动偏移，而且只在看得见的那一段里算，滚出去的部分点不到。
enum class CandidateExpandPart { none, toggle, row, cell, scrollbar };
struct CandidateExpandHit {
  CandidateExpandPart part = CandidateExpandPart::none;
  size_t index = 0;
};
inline bool candidate_expand_inside(const CandidateRowBounds &box, double x, double y) {
  return x >= box.left && x < box.right && y >= box.top && y < box.bottom;
}
inline CandidateExpandHit candidate_expand_hit(const CandidateExpandLayout &layout, double offset,
                                               double x, double y) {
  if (!std::isfinite(x) || !std::isfinite(y))
    return {};
  if (candidate_expand_inside(layout.toggle, x, y))
    return {CandidateExpandPart::toggle, 0};
  if (x < layout.left || x >= layout.right || y < layout.top || y >= layout.bottom())
    return {};
  if (layout.scrollbar && x >= layout.right - layout.track)
    return {CandidateExpandPart::scrollbar, 0};
  const double content_y = y + offset;
  for (size_t index = 0; index < layout.rows.size(); ++index)
    if (candidate_expand_inside(layout.rows[index].bounds, x, content_y))
      return {CandidateExpandPart::row, index};
  for (size_t index = 0; index < layout.cells.size(); ++index)
    if (candidate_expand_inside(layout.cells[index], x, content_y))
      return {CandidateExpandPart::cell, index};
  return {};
}

// 滚动条的滑块（ScrollViewer::GetScrollbarThumbRect）：贴着轨道右缘内缩 2 DIP，长度按看得见的比例，不短于 20 DIP；没有滚动条时为空。
inline std::optional<CandidateRowBounds> candidate_expand_thumb(const CandidateExpandLayout &layout,
                                                               double offset) {
  if (!layout.scrollbar || !(layout.viewport > 0.0))
    return std::nullopt;
  const double length = (std::max)(layout.viewport - layout.thumb_inset * 2.0, 0.0);
  const double ratio = (std::clamp)(layout.viewport / (std::max)(layout.content, 1.0), 0.0, 1.0);
  const double height = (std::min)((std::max)(length * ratio, layout.thumb_min), length);
  const double travel = (std::max)(length - height, 0.0);
  const double max = layout.max_offset();
  const double position = max > 0.0 ? (std::clamp)(offset, 0.0, max) / max : 0.0;
  const double right = layout.right - layout.thumb_right_inset;
  const double top = layout.top + layout.thumb_inset + travel * position;
  return CandidateRowBounds{right - layout.thumb_width, top, right, top + height};
}
// 拖动滑块（或在轨道上按下后接着拖）时的滚动偏移：滑块的上缘跟着指针，grab 是按下时指针在滑块里的高度。在轨道上滑块之外按下时，调用方先让滑块中心跳到那里（grab 取滑块高度的一半）。
inline double candidate_expand_drag(const CandidateExpandLayout &layout, double y, double grab) {
  const auto thumb = candidate_expand_thumb(layout, 0.0);
  if (!thumb)
    return 0.0;
  const double height = thumb->bottom - thumb->top;
  const double track_top = layout.top + layout.thumb_inset;
  const double travel =
      (std::max)(layout.viewport - layout.thumb_inset * 2.0 - height, 1.0);
  const double top = (std::clamp)(y - grab, track_top, track_top + travel);
  return (top - track_top) / travel * layout.max_offset();
}

// 展开状态：开没开、滚到哪儿、网格排出了几个候选，以及网格第一项是谁（换了就回到顶部）。
struct CandidateExpandState {
  bool expanded = false;
  double offset = 0.0;
  size_t revealed = 0;
  bool identified = false;
  size_t first_index = 0;
  std::string first_text;
};
// 收起：一切归零。候选窗关闭（上屏或取消）、换了排列或皮肤、点了「∧」都走这里。
inline void candidate_expand_collapse(CandidateExpandState &state) { state = {}; }
// 点「∨」：展开，从顶部开始。后续候选随后才到。
inline void candidate_expand_open(CandidateExpandState &state) {
  candidate_expand_collapse(state);
  state.expanded = true;
}
// 每一帧对照这一帧的后续候选：网格第一项（下标和文字）变了就是换了候选（新的一页或接着打字），回到顶部、只排第一批；只是往后追加了候选（同一代补上了后面的）就保持滚动位置和已排出的数目（msime-windows candidate_presenter.cpp 的 sameContent）。available 是这一帧可排的后续候选数，为 0（还没拿到）时什么都不动。返回是否回到了顶部。
inline bool candidate_expand_sync(CandidateExpandState &state, size_t first_index,
                                  const std::string &first_text, size_t available) {
  if (!state.expanded || available == 0)
    return false;
  const bool same = state.identified && state.first_index == first_index &&
                    state.first_text == first_text;
  if (!same) {
    state.offset = 0.0;
    state.revealed = (std::min)(candidate_expand_batch, available);
    state.identified = true;
    state.first_index = first_index;
    state.first_text = first_text;
    return true;
  }
  state.revealed = (std::min)((std::max)(state.revealed, (std::min)(candidate_expand_batch, available)),
                              available);
  return false;
}
// 偏移夹在 [0, max_offset] 里，内容变短时滚动位置跟着收回来。返回是否变了。
inline bool candidate_expand_clamp(CandidateExpandState &state, double max_offset) {
  const double clamped = (std::clamp)(state.offset, 0.0, (std::max)(max_offset, 0.0));
  const bool changed = clamped != state.offset;
  state.offset = clamped;
  return changed;
}
// 滚轮：一格滚一行（行高加行距），高精度滚轮按比例滚，到顶或到底停住（CandidatePresenter::ScrollExpanded）。返回是否滚动了。
inline bool candidate_expand_scroll(CandidateExpandState &state, int delta, int notch, double step,
                                    double max_offset) {
  if (!state.expanded || notch <= 0 || !(step > 0.0))
    return false;
  const double before = state.offset;
  state.offset -= static_cast<double>(delta) / static_cast<double>(notch) * step;
  candidate_expand_clamp(state, max_offset);
  return state.offset != before;
}
// 滚到离底部一行半以内、而且这一代还有没排出来的候选时，再排一批（RequestMoreExpandedIfNearEnd）。内容不够一屏时偏移的上限是 0，也会立刻再排。
inline bool candidate_expand_wants_more(const CandidateExpandState &state, double max_offset,
                                        double step, size_t available) {
  return state.expanded && state.revealed < available &&
         (std::max)(max_offset, 0.0) - state.offset <= step * 1.5;
}
inline bool candidate_expand_reveal(CandidateExpandState &state, size_t available) {
  const size_t next = (std::min)(state.revealed + candidate_expand_batch, available);
  const bool changed = next != state.revealed;
  state.revealed = next;
  return changed;
}

// 候选窗该不该收起：展开着，而 Server 那边已经不为这个租约开着（候选窗关闭时它先清掉，比候选窗的轮询早；有退出动画时 hide_now 还没跑），或者焦点换了客户端，或者这一页不能展开（竖排、释义页、只能用键盘选的列表、换了皮肤）。
constexpr bool candidate_expand_stale(bool expanded, bool server_open, bool same_lease,
                                      bool available) {
  return expanded && (!server_open || !same_lease || !available);
}
// 展开着、这一代的后续候选还没到时，沿用上一帧展开区的高度，卡片不先缩回一行再长回来；列表已经到了（哪怕这一代后面一个候选也没有）就按实际的内容排，返回 0。
constexpr double candidate_expand_held_viewport(bool expanded, bool list_arrived,
                                                double previous) {
  return expanded && !list_arrived ? previous : 0.0;
}
// 网格文字宽度的缓存超过上限时只留眼下这一批（keep）的，上限不小于这一批的两倍：整个清空的话，一旦排过的不同文字超过上限，每一帧都要把这一批重新量一遍。
inline void candidate_expand_trim_widths(std::unordered_map<std::string, double> &cache,
                                         const std::vector<const std::string *> &keep,
                                         size_t limit = 4096) {
  if (cache.size() <= (std::max)(limit, keep.size() * 2))
    return;
  std::unordered_map<std::string, double> kept;
  kept.reserve(keep.size());
  for (const auto *text : keep)
    if (const auto found = cache.find(*text); found != cache.end())
      kept.emplace(found->first, found->second);
  cache.swap(kept);
}

// 滚轮交给谁：展开着时滚动展开区，不翻页，也不看「鼠标滚轮翻页」开关；收起时和别的皮肤一样，开关打开才翻页，关着时交回 DefWindowProc。
enum class CandidateWheelRoute { pass, page, scroll };
constexpr CandidateWheelRoute candidate_wheel_route(bool expanded, bool mouse_wheel) {
  return expanded ? CandidateWheelRoute::scroll
                  : mouse_wheel ? CandidateWheelRoute::page : CandidateWheelRoute::pass;
}
} // namespace msime::windows

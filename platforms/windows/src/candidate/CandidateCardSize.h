#pragma once
#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <optional>
#include <stdexcept>
#include <vector>

namespace msime::windows {
// Card geometry ported from the shipped Windows presenter. Widths arrive
// already measured in device independent pixels; this header only composes
// them, so it stays free of DirectWrite and is exercised without a renderer.
// Measured single-line widths of one candidate's three runs, each at its own font size: the text (with its badge) and the annotation at the candidate size, the translation at CandidateCardMetrics::translation_font. All three zero hides the row.
struct CandidateItemWidths {
  double text = 0.0, annotation = 0.0, translation = 0.0;
};
enum class CandidateRun { text, annotation, translation };
// Height of candidate `index`'s run once wrapped to `width` DIPs. The text run is the candidate text with its badge, at the candidate size. The window answers with DirectWrite; without one the layout estimates from the single-line width.
using CandidateWrapMeasure =
    std::function<double(size_t index, CandidateRun run, double width)>;
struct CandidateCardInput {
  double preedit_width = 0.0;
  // One entry per candidate.
  std::vector<CandidateItemWidths> items;
  CandidateWrapMeasure wrapped;
  bool horizontal = false;
  bool preedit_visible = true;
  double font_size = 16.0;
  double preedit_font_size = 16.0;
  // Work area caps. At most one pixel means the axis stays uncapped.
  double max_width = 0.0;
  double max_height = 0.0;
  // Minimum card width asked for by an external skin package, in DIPs. Zero keeps the width derived from the font size. A mascot skin needs it: the artwork is drawn against a card of a particular width, and a narrow card makes the decoration overhang.
  double skin_min_width = 0.0;
  // Measured width of the page indicator ("1 / 3") at CandidateCardMetrics::pager_font. Zero draws no pager; it shares the preedit row, so it is only drawn when that row is.
  double page_width = 0.0;
};
struct CandidateCardSize {
  double width, height;
};
// Screen pixels, including negative monitor origins.
struct CandidateBounds {
  int x, y, width, height;
};
// One source for the card's spacing, so sizing, drawing and hit testing cannot drift apart. Rows are laid out from the top padding downwards; the Fluent card pads its rows by 6 on every side.
struct CandidateCardMetrics {
  double pad_x = 12.0, pad_y = 6.0, slack_x = 14.0, slack_y = 6.0;
  double number_and_bar = 0.0, preedit_row = 0.0, candidate_row = 0.0,
         min_width = 0.0;
  // Annotation and translation runs, as the shipped presenter spaces them: the annotation follows the text 4 DIP later at the same size, the translation is 0.78 of the size and 0.65 of it away. A run moved below the first line takes at least one line of its own font.
  double annotation_gap = 4.0, annotation_line = 0.0, translation_font = 0.0,
         translation_gap = 0.0, translation_line = 0.0;
  // Space every horizontal column keeps after its content. It belongs to the column, so neighbouring columns touch and a click between two candidates still lands on one of them.
  double column_gap = 8.0;
  // Vertical space between two stacked rows, and between two lines of a horizontal page: the shipped presenter's CandidateList itemGap. It belongs to no row, so a click in it selects nothing, as in the shipped card.
  double item_gap = 2.0;
  // The pager at the right of the preedit row: the page indicator and the previous and next arrows, in the secondary colour at 13 DIP, 12 DIP after the preedit and 12 DIP before the arrows. Each arrow is a square box of pager_arrow for its glyph and its click.
  double pager_font = 13.0, pager_gap = 12.0, pager_arrow = 16.0;
};
inline CandidateCardMetrics candidate_card_metrics(double font_size,
                                                   double preedit_font_size,
                                                   bool preedit_visible) {
  if (!std::isfinite(font_size) || font_size < 12.0 || font_size > 32.0 ||
      !std::isfinite(preedit_font_size) || preedit_font_size < 12.0 ||
      preedit_font_size > 32.0)
    throw std::invalid_argument("Invalid candidate card font size");
  CandidateCardMetrics metrics;
  // Selection number column plus the separator bar before the candidate text.
  metrics.number_and_bar = font_size * 0.8 + font_size * 0.2 + 8.0;
  // kCandidateMinWidthDip in the shipped presenter (card->SetMinWidth), independent of the font size.
  metrics.min_width = 160.0;
  metrics.preedit_row = preedit_visible ? preedit_font_size * 1.4 + 6.0 : 0.0;
  // The shipped presenter's CandidateList itemHeight: the minimum height of one candidate row.
  metrics.candidate_row = font_size * 1.35 + 2.0;
  metrics.annotation_line = font_size * 1.25;
  metrics.translation_font = font_size * 0.78;
  metrics.translation_gap = font_size * 0.65;
  metrics.translation_line = metrics.translation_font * 1.25;
  return metrics;
}
// Row rectangle in card coordinates for rows of one line each; the caller supplies the card width it actually got. candidate_page_layout starts a vertical page from these and grows the rows that carry wrapped runs; the first row's top and minimum height are the same either way. The horizontal branch is an even split of the width, which candidate_page_layout no longer uses: a horizontal page gets columns of each candidate's natural width instead.
struct CandidateRowBounds {
  double left, top, right, bottom;
};
inline CandidateRowBounds candidate_row_bounds(size_t index, size_t count,
                                               double width,
                                               const CandidateCardMetrics &metrics,
                                               bool horizontal) {
  if (index >= count || count > 9 || !std::isfinite(width) || width <= 0.0)
    throw std::invalid_argument("Invalid candidate row");
  const double top = metrics.pad_y + metrics.preedit_row;
  const double stride = metrics.candidate_row + metrics.item_gap;
  if (!horizontal)
    return {metrics.pad_x / 2.0, top + stride * static_cast<double>(index),
            width - metrics.pad_x / 2.0,
            top + stride * static_cast<double>(index) + metrics.candidate_row};
  const double column =
      (width - metrics.pad_x) / static_cast<double>(count);
  return {metrics.pad_x / 2.0 + column * static_cast<double>(index), top,
          metrics.pad_x / 2.0 + column * static_cast<double>(index + 1),
          top + metrics.candidate_row};
}
// Width of the pager for an indicator of `indicator_width`: the indicator, the gap and the two arrows. Zero without an indicator.
inline double candidate_pager_width(double indicator_width,
                                    const CandidateCardMetrics &metrics) {
  if (!(indicator_width > 0.0) || !std::isfinite(indicator_width))
    return 0.0;
  return indicator_width + metrics.pager_gap + metrics.pager_arrow * 2.0;
}
// Where the pager sits in the preedit row of a card `width` wide, in card coordinates: right aligned to the rows' right edge, the arrows last. `left` is the pager's left edge, which the preedit is clipped before, less pager_gap.
struct CandidatePagerLayout {
  double left = 0.0;
  CandidateRowBounds indicator{}, previous{}, next{};
};
inline std::optional<CandidatePagerLayout>
candidate_pager_layout(double width, double indicator_width,
                       const CandidateCardMetrics &metrics) {
  const double pager = candidate_pager_width(indicator_width, metrics);
  if (!(pager > 0.0) || metrics.preedit_row <= 0.0 || !std::isfinite(width) ||
      width <= 0.0)
    return std::nullopt;
  const double top = metrics.pad_y, bottom = metrics.pad_y + metrics.preedit_row;
  const double right = width - metrics.pad_x / 2.0;
  CandidatePagerLayout layout;
  layout.next = {right - metrics.pager_arrow, top, right, bottom};
  layout.previous = {layout.next.left - metrics.pager_arrow, top,
                     layout.next.left, bottom};
  layout.indicator = {layout.previous.left - metrics.pager_gap - indicator_width,
                      top, layout.previous.left - metrics.pager_gap, bottom};
  layout.left = layout.indicator.left;
  return layout;
}
// Which arrow of a drawn pager a card point falls on: true for the previous page, false for the next. The previous arrow is inert on the first page. The next one is always live, because the Engine hands candidates over lazily and the page count grows as the user pages.
inline std::optional<bool>
candidate_pager_hit(double x, double y,
                    const std::optional<CandidatePagerLayout> &pager,
                    bool first_page) {
  if (!pager || !std::isfinite(x) || !std::isfinite(y))
    return std::nullopt;
  auto inside = [x, y](const CandidateRowBounds &box) {
    return x >= box.left && x < box.right && y >= box.top && y < box.bottom;
  };
  if (inside(pager->previous) && !first_page)
    return true;
  if (inside(pager->next))
    return false;
  return std::nullopt;
}
// The Fluent selection pill on the selected row: 3 DIP wide and centred on the row's left edge, 40% of a one-line row tall (never less than twice its width) and centred in the row, so a one-line row has it from 30% to 70% of its height. A row that grows because its text or annotation wrapped keeps the same pill in its middle instead of a stretched one; a row shorter than the pill starts it at the row top. The corner radius is half the width.
struct CandidateSelectionBar {
  double left, right, top, bottom;
};
inline constexpr double candidate_selection_bar_width = 3.0;
inline CandidateSelectionBar candidate_selection_bar(double row_left,
                                                     double row_top,
                                                     double row_bottom,
                                                     double line_height) {
  const double width = candidate_selection_bar_width;
  const double height = (std::max)(line_height * 0.4, width * 2.0);
  const double top =
      row_top + (std::max)((row_bottom - row_top - height) * 0.5, 0.0);
  return {row_left - width * 0.5, row_left + width * 0.5, top, top + height};
}
// A run's box relative to the row's text column: x from where the candidate text starts, y from the row top. Zero width means the run is absent. An inline run shares the first line and is centred in it; one below the first line is top aligned and may wrap.
struct CandidateRunBox {
  double x = 0.0, y = 0.0, width = 0.0, height = 0.0;
  bool below = false;
};
struct CandidateItemLayout {
  double text_width = 0.0;
  // Height of the text's box from the row top: one candidate_row, or the measured wrapped height when the text is wider than the column. The text is centred in it, and runs placed below the first line start under it.
  double text_height = 0.0;
  // Whether the text is wider than the column and so is drawn wrapped.
  bool text_wrapped = false;
  CandidateRunBox annotation, translation;
  // At least one candidate_row; grows by every run placed below the first line.
  double height = 0.0;
};
// Port of the shipped presenter's per-item geometry (CandidateList::MeasureItem). Text wider than the column wraps inside it and the row takes the measured height, at least one candidate_row. Short runs stay on the text's line; a run that does not fit moves under the text and wraps to the column. A horizontal list always puts the translation under the text, and a translation never goes back up once the annotation has moved down.
inline CandidateItemLayout
candidate_item_layout(const CandidateItemWidths &item, double content_width,
                      const CandidateCardMetrics &metrics, bool horizontal,
                      const std::function<double(CandidateRun, double)> &wrapped) {
  content_width = (std::max)(content_width, 1.0);
  CandidateItemLayout layout;
  layout.text_width = (std::min)(item.text, content_width);
  // Height of a run of `natural` single-line width once it sits in `width`: one `line` unless it has to wrap.
  auto run_height = [&](CandidateRun run, double natural, double width,
                        double line) {
    double height = line;
    if (natural > width)
      height = wrapped ? wrapped(run, width)
                       : std::ceil(natural / width) * line;
    // A failed or nonsense measurement still reserves the single line.
    return std::isfinite(height) ? (std::max)(height, line) : line;
  };
  // Without a measure a wrapped text line is estimated at the candidate size's line height, like an annotation line.
  layout.text_wrapped = item.text > content_width;
  layout.text_height =
      layout.text_wrapped
          ? (std::max)(run_height(CandidateRun::text, item.text,
                                  content_width, metrics.annotation_line),
                       metrics.candidate_row)
          : metrics.candidate_row;
  layout.height = layout.text_height;
  double line_end = layout.text_width;
  auto below = [&](CandidateRun run, double natural, double width, double line) {
    const double height = run_height(run, natural, width, line);
    CandidateRunBox box{0.0, layout.height, width, height, true};
    layout.height += height;
    return box;
  };
  if (item.annotation > 0.0) {
    const double width = (std::min)(item.annotation, content_width);
    if (line_end + metrics.annotation_gap + width <= content_width) {
      layout.annotation = {line_end + metrics.annotation_gap, 0.0, width,
                           metrics.candidate_row, false};
      line_end = layout.annotation.x + width;
    } else {
      layout.annotation = below(CandidateRun::annotation, item.annotation,
                                width, metrics.annotation_line);
    }
  }
  if (item.translation > 0.0) {
    const double width = (std::min)(item.translation, content_width);
    if (!horizontal && !layout.annotation.below &&
        line_end + metrics.translation_gap + width <= content_width)
      layout.translation = {line_end + metrics.translation_gap, 0.0, width,
                            metrics.candidate_row, false};
    else
      layout.translation = below(CandidateRun::translation, item.translation,
                                 width, metrics.translation_line);
  }
  return layout;
}
// Width a candidate's row asks for with nothing wrapped: its selection number and bar plus the content. A vertical row keeps the translation on the text's line; a horizontal one stacks it under the text, so the wider of the two lines decides. Zero for a hidden candidate, whose three runs all measured zero.
inline double candidate_item_natural_width(const CandidateItemWidths &item,
                                           const CandidateCardMetrics &metrics,
                                           bool horizontal) {
  if (item.text <= 0.0 && item.annotation <= 0.0 && item.translation <= 0.0)
    return 0.0;
  const double line =
      item.text +
      (item.annotation > 0.0 ? metrics.annotation_gap + item.annotation : 0.0);
  const double content =
      horizontal ? (std::max)(line, item.translation)
                 : line + (item.translation > 0.0
                               ? metrics.translation_gap + item.translation
                               : 0.0);
  return content + metrics.number_and_bar;
}
// One laid out row: its rectangle in card coordinates and the runs inside it.
struct CandidateRowLayout {
  CandidateRowBounds bounds;
  CandidateItemLayout item;
};
// Rows for a whole page at the card width actually drawn. A vertical list stacks rows of their own heights. A horizontal list is the shipped presenter's CandidateList::Measure: each column is its candidate's natural width, columns run left to right, and one that would pass the card's inner edge starts a new line; only a candidate wider than a whole line is narrowed to it, and its text and runs wrap inside that. Every column on a line takes the line's tallest height, so the selection fills evenly. Sizing, painting and hit testing all read this one result.
inline std::vector<CandidateRowLayout>
candidate_page_layout(const std::vector<CandidateItemWidths> &items,
                      double width, const CandidateCardMetrics &metrics,
                      bool horizontal, const CandidateWrapMeasure &wrapped = {}) {
  if (items.size() > 9 || !std::isfinite(width) || width <= 0.0)
    throw std::invalid_argument("Invalid candidate page");
  std::vector<CandidateRowLayout> rows;
  rows.reserve(items.size());
  double top = metrics.pad_y + metrics.preedit_row, tallest = 0.0;
  // Horizontal cursor: x within the line, and where the current line's columns begin.
  const double line_width = (std::max)(width - metrics.pad_x, 1.0);
  double x = 0.0;
  size_t line_start = 0;
  auto close_line = [&](size_t end) {
    for (size_t index = line_start; index < end; ++index)
      rows[index].bounds.bottom = rows[index].bounds.top + tallest;
    line_start = end;
  };
  for (size_t index = 0; index < items.size(); ++index) {
    auto bounds =
        candidate_row_bounds(index, items.size(), width, metrics, horizontal);
    if (horizontal) {
      const double natural =
          candidate_item_natural_width(items[index], metrics, true);
      const double column =
          natural > 0.0 ? (std::min)(natural + metrics.column_gap, line_width)
                        : 0.0;
      if (x > 0.0 && x + column > line_width) {
        close_line(index);
        top += tallest + metrics.item_gap;
        tallest = 0.0;
        x = 0.0;
      }
      bounds = {metrics.pad_x / 2.0 + x, top, metrics.pad_x / 2.0 + x + column,
                top + metrics.candidate_row};
      x += column;
    }
    std::function<double(CandidateRun, double)> measure;
    if (wrapped)
      measure = [&wrapped, index](CandidateRun run, double run_width) {
        return wrapped(index, run, run_width);
      };
    auto item = candidate_item_layout(
        items[index], bounds.right - bounds.left - metrics.number_and_bar,
        metrics, horizontal, measure);
    if (!horizontal) {
      bounds.top = top;
      bounds.bottom = top + item.height;
      top = bounds.bottom + metrics.item_gap;
    }
    tallest = (std::max)(tallest, item.height);
    rows.push_back({bounds, item});
  }
  if (horizontal)
    close_line(rows.size());
  return rows;
}
// Hit testing runs on the same rows the renderer drew.
inline std::optional<size_t>
candidate_card_hit(double x, double y, double width, double height,
                   const std::vector<CandidateRowLayout> &rows) {
  if (rows.empty() || rows.size() > 9 || !std::isfinite(x) ||
      !std::isfinite(y) || !std::isfinite(width) || !std::isfinite(height) ||
      width <= 0.0 || height <= 0.0 || x < 0.0 || y < 0.0 || x >= width ||
      y >= height)
    return std::nullopt;
  for (size_t index = 0; index < rows.size(); ++index) {
    const auto &row = rows[index].bounds;
    if (x >= row.left && x < row.right && y >= row.top && y < row.bottom)
      return index;
  }
  return std::nullopt;
}
inline CandidateCardSize candidate_card_size(const CandidateCardInput &input) {
  auto measured = [](double value) {
    return std::isfinite(value) && value >= 0.0;
  };
  if (input.items.size() > 9 || !measured(input.preedit_width) ||
      !measured(input.page_width) || !measured(input.max_width) ||
      !measured(input.max_height))
    throw std::invalid_argument("Invalid candidate card measurement");
  for (const auto &item : input.items)
    if (!measured(item.text) || !measured(item.annotation) ||
        !measured(item.translation))
      throw std::invalid_argument("Invalid candidate card measurement");
  const auto shape = candidate_card_metrics(input.font_size,
                                            input.preedit_font_size,
                                            input.preedit_visible);
  const double pad_x = shape.pad_x, pad_y = shape.pad_y,
               slack_x = shape.slack_x, slack_y = shape.slack_y;
  // The skin's floor only ever raises the built-in one, never lowers it: a
  // package must not be able to shrink the card below what the text needs.
  const double min_width =
      (std::max)(shape.min_width, std::isfinite(input.skin_min_width) &&
                                          input.skin_min_width > 0.0
                                      ? input.skin_min_width
                                      : 0.0);
  const double preedit_row = shape.preedit_row;
  const double candidate_row = shape.candidate_row;

  double width = 0.0;
  double height = pad_y + slack_y;
  if (input.preedit_visible) {
    const double pager = candidate_pager_width(input.page_width, shape);
    width = (std::max)(width, input.preedit_width + 6.0 +
                                  (pager > 0.0 ? shape.pager_gap + pager : 0.0));
    height += preedit_row;
  }
  auto visible = [](const CandidateItemWidths &item) {
    return item.text > 0.0 || item.annotation > 0.0 || item.translation > 0.0;
  };
  // A horizontal card asks for every column side by side, the same columns candidate_page_layout places.
  double row_width_sum = 0.0, row_width_max = 0.0;
  for (const auto &item : input.items) {
    if (!visible(item))
      continue;
    const double row =
        candidate_item_natural_width(item, shape, input.horizontal);
    row_width_sum += row + shape.column_gap;
    row_width_max = (std::max)(row_width_max, row);
  }
  width = (std::max)(input.horizontal ? row_width_sum : row_width_max, width);
  width = (std::max)(width + pad_x + slack_x, min_width);
  auto clamp = [](double value, double cap) {
    value = (std::max)(value, 1.0);
    return cap > 1.0 ? (std::min)(value, cap) : value;
  };
  width = clamp(width, input.max_width);
  // Heights come from the width the card will actually get: a capped card wraps the runs that no longer fit, and grows by exactly what the painter will draw.
  const auto rows = candidate_page_layout(input.items, width, shape,
                                          input.horizontal, input.wrapped);
  // A horizontal page ends at its last line's bottom, however many lines the capped width broke it into.
  // A vertical page is its visible rows plus one item_gap between each two of them.
  double rows_height = 0.0;
  bool stacked = false;
  for (size_t index = 0; index < rows.size(); ++index) {
    if (!visible(input.items[index]))
      continue;
    const auto &row = rows[index];
    rows_height =
        input.horizontal
            ? (std::max)(rows_height, row.bounds.bottom - pad_y - preedit_row)
            : rows_height + (stacked ? shape.item_gap : 0.0) + row.item.height;
    stacked = true;
  }
  // An empty list still reserves one row so the card cannot collapse.
  height += (std::max)(rows_height, candidate_row);
  return {width, clamp(height, input.max_height)};
}
// Where the card sits relative to the caret. All values are physical pixels
// except scale, which is dpi/96; the design offsets below are DIPs so the gaps
// stay visually stable at 150% and 200%.
struct CandidatePlacementInput {
  // The line's bottom-left, as TSF reports the text extent.
  int anchor_x = 0;
  int anchor_y = 0;
  int width = 0;
  int height = 0;
  // The height the flip decision is made with. For a vertical list this is the
  // tallest the list has been this composition, not its current height, so a
  // list that grows as the user types does not jump below-to-above mid-word.
  // The card is still *placed* with its current height.
  int decision_height = 0;
  int work_left = 0, work_top = 0, work_right = 0, work_bottom = 0;
  double scale = 1.0;
};
// The height a vertical list's flip decision starts from before any list has been measured this composition: the shipped presenter seeds its memory with DEFAULT_WINDOW_HEIGHT_DIP (CANDIDATE_WINDOW_HEIGHT, 232 DIP) and resets it to that on hide, so a short first page already flips above the caret when a full one would not fit below. The decision then follows the tallest list seen, capped at the work area height.
inline constexpr double candidate_vertical_decision_seed_dip = 232.0;
inline int64_t candidate_vertical_decision_height(int64_t tallest, double scale,
                                                  int64_t available_height) {
  const double factor = std::isfinite(scale) && scale > 0.0 ? scale : 1.0;
  const auto seed = static_cast<int64_t>(
      std::ceil(candidate_vertical_decision_seed_dip * factor));
  return (std::min)((std::max)(tallest, seed), available_height);
}
struct CandidatePlacement {
  int x = 0;
  int y = 0;
  bool above = false;
};
// Position the card against the caret, flipping above the input line when a
// full list would not fit below it. Clamping alone is not enough: near the
// bottom of a screen it slides the card up over the very text being composed.
inline CandidatePlacement
candidate_card_placement(const CandidatePlacementInput &input) {
  const double scale = input.scale > 0.0 ? input.scale : 1.0;
  const int caret_gap = static_cast<int>(std::lround(3.0 * scale));
  const int edge_pad = static_cast<int>(std::lround(2.0 * scale));
  CandidatePlacement placement{input.anchor_x, input.anchor_y + caret_gap,
                               false};
  if (placement.x + input.width > input.work_right - edge_pad)
    placement.x = input.work_right - input.width - edge_pad;
  if (placement.x < input.work_left + edge_pad)
    placement.x = input.work_left + edge_pad;
  if (placement.y < input.work_top + edge_pad)
    placement.y = input.work_top + edge_pad;
  const int decision = (std::max)(input.decision_height, input.height);
  if (placement.y + decision > input.work_bottom) {
    // anchor_y is the line's bottom, so stepping back one line height puts the
    // card's lower edge at the line's top. A fixed gap instead of a line height
    // leaves a hole under short cards.
    const int line_height = static_cast<int>(std::lround(24.0 * scale));
    placement.y = input.anchor_y - input.height - line_height;
    placement.above = true;
    if (placement.y < input.work_top + edge_pad)
      placement.y = input.work_top + edge_pad;
  }
  return placement;
}
} // namespace msime::windows

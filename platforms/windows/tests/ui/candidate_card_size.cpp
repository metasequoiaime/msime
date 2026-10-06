#include "CandidateCardSize.h"
#include <cmath>
#include <limits>
#include <stdexcept>

using namespace msime::windows;
void require(bool value) {
  if (!value)
    throw std::runtime_error("Candidate card size validation failed");
}
bool near(double value, double expected) {
  return std::fabs(value - expected) < 0.001;
}
bool rejected(CandidateCardInput input) {
  try {
    candidate_card_size(input);
  } catch (const std::invalid_argument &) {
    return true;
  }
  return false;
}
int main() {
  // The shipped presenter's row: itemHeight = fontSize * 1.35 + 2 DIP, with itemGap = 2 DIP between two rows (CandidateList::Measure).
  const double row16 = 16.0 * 1.35 + 2.0, gap = 2.0;
  // Vertical list: the card follows the widest row and one row per candidate.
  CandidateCardInput vertical;
  vertical.preedit_width = 40.0;
  vertical.items = {{60.0}, {120.0}, {80.0}};
  const auto stacked = candidate_card_size(vertical);
  require(near(stacked.width, 120.0 + 16.0 + 8.0 + 12.0 + 14.0));
  require(near(stacked.height, 6.0 + 6.0 + (16.0 * 1.4 + 6.0) +
                                   row16 * 3.0 + gap * 2.0));

  // The same candidates on one line widen the card and keep a single row.
  CandidateCardInput horizontal = vertical;
  horizontal.horizontal = true;
  const auto inline_card = candidate_card_size(horizontal);
  require(near(inline_card.width, (60.0 + 120.0 + 80.0) + (16.0 + 8.0) * 3.0 +
                                      8.0 * 3.0 + 12.0 + 14.0));
  require(near(inline_card.height,
               6.0 + 6.0 + (16.0 * 1.4 + 6.0) + row16));
  require(inline_card.width > stacked.width &&
          inline_card.height < stacked.height);

  // Hiding the preedit shrinks its row to the brand mark's without touching the candidate rows: the row stays, so the mark does.
  CandidateCardInput hidden = vertical;
  hidden.preedit_visible = false;
  const auto without_preedit = candidate_card_size(hidden);
  require(near(without_preedit.height,
               stacked.height - (16.0 * 1.4 + 6.0) + 22.0));
  require(near(without_preedit.width, stacked.width));

  // A wide preedit drives the width once it passes the widest candidate, after the brand mark and its gap.
  CandidateCardInput long_preedit = vertical;
  long_preedit.preedit_width = 400.0;
  require(near(candidate_card_size(long_preedit).width,
               16.0 + 6.0 + 400.0 + 6.0 + 12.0 + 14.0));

  // Zero-width entries are measured as hidden and reserve no row.
  CandidateCardInput sparse = vertical;
  sparse.items = {{60.0}, {}, {}};
  require(near(candidate_card_size(sparse).height,
               stacked.height - (row16 + gap) * 2.0));

  // An empty list still reserves one candidate row under the brand mark's row, and the floor applies: the shipped presenter's kCandidateMinWidthDip, the same at every font size.
  CandidateCardInput empty;
  empty.preedit_visible = false;
  const auto collapsed = candidate_card_size(empty);
  require(near(collapsed.width, 160.0));
  require(near(collapsed.height, 6.0 + 6.0 + 22.0 + row16));
  for (double font : {12.0, 24.0, 32.0}) {
    CandidateCardInput sized = empty;
    sized.font_size = font;
    require(near(candidate_card_size(sized).width, 160.0) &&
            near(candidate_card_metrics(font, 16.0, false).min_width, 160.0));
  }

  // Work area caps clamp both axes; a cap of at most one pixel is no cap.
  CandidateCardInput capped = vertical;
  capped.max_width = 100.0;
  capped.max_height = 30.0;
  const auto clamped = candidate_card_size(capped);
  require(near(clamped.width, 100.0) && near(clamped.height, 30.0));
  capped.max_width = 1.0;
  capped.max_height = 0.0;
  require(near(candidate_card_size(capped).width, stacked.width) &&
          near(candidate_card_size(capped).height, stacked.height));

  // Font size scales every constant, including the minimum width.
  CandidateCardInput large = vertical;
  large.font_size = 24.0;
  large.preedit_font_size = 24.0;
  const auto scaled = candidate_card_size(large);
  require(scaled.width > stacked.width && scaled.height > stacked.height);
  require(near(scaled.width, 120.0 + 24.0 + 8.0 + 12.0 + 14.0));

  // Rows come from the same metrics the sizing used, so drawing and hit
  // testing cannot drift apart.
  const auto metrics = candidate_card_metrics(16.0, 16.0, true);
  require(near(metrics.candidate_row, row16) && near(metrics.item_gap, gap) &&
          near(metrics.preedit_row, 16.0 * 1.4 + 6.0) &&
          near(metrics.number_and_bar, 16.0 + 8.0));
  // The top row is always there for the brand mark: without the preedit it is the mark's 22 DIP, and a preedit line never makes it shorter.
  const auto hidden_metrics = candidate_card_metrics(16.0, 16.0, false);
  require(near(hidden_metrics.preedit_row, 22.0) &&
          near(hidden_metrics.header_row, 22.0));
  require(near(candidate_card_metrics(16.0, 12.0, true).preedit_row,
               12.0 * 1.4 + 6.0));
  // The mark is a 16 DIP square at the row's left padding, centred in the row, and the preedit starts 6 DIP after it.
  require(near(metrics.logo_side, 16.0) && near(metrics.logo_gap, 6.0));
  for (const auto &shape : {metrics, hidden_metrics}) {
    const auto logo = candidate_logo_bounds(shape);
    require(near(logo.left, shape.pad_x) &&
            near(logo.right - logo.left, 16.0) &&
            near(logo.bottom - logo.top, 16.0));
    require(near(logo.top - shape.pad_y,
                 shape.pad_y + shape.preedit_row - logo.bottom));
    require(logo.top >= shape.pad_y &&
            logo.bottom <= shape.pad_y + shape.preedit_row);
    require(near(candidate_preedit_left(shape), logo.right + 6.0));
  }
  // The card is wide enough for the mark ahead of the preedit: a card sized for a preedit alone leaves the preedit its measured width after the mark.
  {
    CandidateCardInput marked;
    marked.preedit_width = 300.0;
    marked.items = {{40.0}};
    const auto card = candidate_card_size(marked);
    require(card.width - metrics.pad_x / 2.0 - candidate_preedit_left(metrics) >=
            300.0);
    // Without a preedit or a pager the mark alone still gets its row.
    CandidateCardInput bare;
    bare.preedit_visible = false;
    bare.items = {{40.0}};
    require(near(candidate_card_size(bare).height, 6.0 + 6.0 + 22.0 + row16));
  }
  for (double font : {11.0, 33.0}) {
    bool caught = false;
    try {
      candidate_card_metrics(font, 16.0, true);
    } catch (const std::invalid_argument &) {
      caught = true;
    }
    require(caught);
  }

  const double card_width = stacked.width;
  const auto first = candidate_row_bounds(0, 3, card_width, metrics, false);
  const auto second = candidate_row_bounds(1, 3, card_width, metrics, false);
  require(near(first.top, metrics.pad_y + metrics.preedit_row));
  require(near(first.bottom, first.top + metrics.candidate_row));
  require(near(second.top, first.bottom + gap) && near(second.left, first.left));
  require(near(first.right, card_width - metrics.pad_x / 2.0));
  const auto column = candidate_row_bounds(1, 3, card_width, metrics, true);
  require(near(column.top, first.top) && near(column.bottom, first.bottom));
  require(near(column.right - column.left,
               (card_width - metrics.pad_x) / 3.0));
  for (auto invalid : {std::make_pair(size_t{3}, size_t{3}),
                       std::make_pair(size_t{0}, size_t{10})}) {
    bool caught = false;
    try {
      candidate_row_bounds(invalid.first, invalid.second, card_width, metrics,
                           false);
    } catch (const std::invalid_argument &) {
      caught = true;
    }
    require(caught);
  }

  // Clicks land on the row that was drawn; the preedit band selects nothing.
  const double card_height = stacked.height;
  const auto page =
      candidate_page_layout(vertical.items, card_width, metrics, false);
  auto row_of = [&](double x, double y) {
    return candidate_card_hit(x, y, card_width, card_height, page);
  };
  require(row_of(20.0, first.top + 1.0) == std::optional<size_t>(0));
  require(row_of(20.0, second.top + 1.0) == std::optional<size_t>(1));
  require(!row_of(20.0, metrics.pad_y + 1.0));
  require(!row_of(20.0, card_height - 1.0));
  require(!row_of(-1.0, first.top + 1.0) && !row_of(card_width, first.top + 1.0));
  require(!candidate_card_hit(20.0, first.top + 1.0, card_width, card_height,
                              {}));
  // Without wrapped runs the page is exactly the one-line rows.
  for (size_t index = 0; index < 3; ++index) {
    const auto plain = candidate_row_bounds(index, 3, card_width, metrics, false);
    require(near(page[index].bounds.top, plain.top) &&
            near(page[index].bounds.bottom, plain.bottom) &&
            near(page[index].bounds.left, plain.left) &&
            near(page[index].bounds.right, plain.right));
  }

  // A horizontal page lays each candidate out at its own natural width, as the shipped presenter's CandidateList::Measure does, instead of splitting the card evenly. Columns sit side by side from the left padding and keep the whole text when the card got the width it asked for.
  {
    const auto columns = candidate_page_layout(
        horizontal.items, inline_card.width, metrics, true);
    for (size_t index = 0; index < 3; ++index) {
      const auto &cell = columns[index];
      require(near(cell.bounds.right - cell.bounds.left,
                   horizontal.items[index].text + metrics.number_and_bar +
                       metrics.column_gap));
      require(near(cell.bounds.left, index == 0 ? metrics.pad_x / 2.0
                                                : columns[index - 1].bounds.right));
      require(near(cell.bounds.top, first.top) &&
              near(cell.bounds.bottom, first.bottom));
      require(near(cell.item.text_width, horizontal.items[index].text));
    }
    auto column_of = [&](double x) {
      return candidate_card_hit(x, first.top + 1.0, inline_card.width,
                                inline_card.height, columns);
    };
    require(column_of(columns[1].bounds.left - 1.0) == std::optional<size_t>(0));
    require(column_of(columns[1].bounds.left + 1.0) == std::optional<size_t>(1));
    require(column_of(columns[2].bounds.right - 1.0) == std::optional<size_t>(2));
    require(!column_of(columns[2].bounds.right + 1.0));

    // The case an even split got wrong: a long candidate between two short ones was given a third of the card and clipped, while the short ones sat in empty space.
    CandidateCardInput uneven;
    uneven.horizontal = true;
    uneven.items = {{20.0}, {200.0}, {20.0}};
    const auto uneven_card = candidate_card_size(uneven);
    require(near(uneven_card.width,
                 (20.0 + 200.0 + 20.0) + (24.0 + 8.0) * 3.0 + 12.0 + 14.0));
    require((uneven_card.width - metrics.pad_x) / 3.0 - metrics.number_and_bar <
            200.0);
    const auto uneven_columns =
        candidate_page_layout(uneven.items, uneven_card.width, metrics, true);
    require(near(uneven_columns[1].item.text_width, 200.0) &&
            near(uneven_columns[0].bounds.right - uneven_columns[0].bounds.left,
                 20.0 + 24.0 + 8.0));

    // A card capped by the work area starts a new line with the column that would pass its inner edge, rather than squeezing every column; the card grows by that line and clicks follow it.
    uneven.max_width = 300.0;
    const auto broken = candidate_card_size(uneven);
    require(near(broken.width, 300.0));
    require(near(broken.height, 6.0 + 6.0 + (16.0 * 1.4 + 6.0) +
                                    2.0 * row16 + gap));
    const auto lines =
        candidate_page_layout(uneven.items, broken.width, metrics, true);
    require(near(lines[1].bounds.top, first.top) &&
            near(lines[1].item.text_width, 200.0));
    require(near(lines[2].bounds.left, metrics.pad_x / 2.0) &&
            near(lines[2].bounds.top, first.bottom + gap) &&
            near(lines[2].bounds.bottom,
                 first.bottom + gap + metrics.candidate_row));
    // The gap between the two lines belongs to neither.
    require(!candidate_card_hit(10.0, first.bottom + gap / 2.0, broken.width,
                                broken.height, lines));
    require(candidate_card_hit(10.0, first.bottom + gap + 1.0, broken.width,
                               broken.height, lines) ==
            std::optional<size_t>(2));
    require(candidate_card_hit(10.0, first.top + 1.0, broken.width,
                               broken.height, lines) ==
            std::optional<size_t>(0));

    // Only a candidate wider than a whole line is still narrowed, to that line: the work area cap is the one place a horizontal candidate is clipped.
    CandidateCardInput wide;
    wide.horizontal = true;
    wide.items = {{400.0}};
    wide.max_width = 200.0;
    const auto wide_card = candidate_card_size(wide);
    const auto wide_rows =
        candidate_page_layout(wide.items, wide_card.width, metrics, true);
    require(near(wide_rows[0].bounds.right - wide_rows[0].bounds.left,
                 200.0 - metrics.pad_x) &&
            near(wide_rows[0].item.text_width,
                 200.0 - metrics.pad_x - metrics.number_and_bar));

    // Translations too wide for the capped card give up room, never below their candidate's line, so the page stays on one line and the translations wrap under their text.
    CandidateCardInput glossed;
    glossed.horizontal = true;
    glossed.items = {{40.0, 0.0, 200.0}, {40.0, 0.0, 200.0}, {40.0}};
    glossed.max_width = 300.0;
    const auto glossed_card = candidate_card_size(glossed);
    const auto glossed_rows =
        candidate_page_layout(glossed.items, glossed_card.width, metrics, true);
    const double line_width = glossed_card.width - metrics.pad_x;
    const auto single_line =
        candidate_single_line_columns(glossed.items, line_width, metrics);
    require(single_line.has_value());
    const double firm = 40.0 + metrics.number_and_bar + metrics.column_gap;
    require(near((*single_line)[0], (*single_line)[1]) && (*single_line)[0] > firm &&
            near((*single_line)[2], firm) &&
            near((*single_line)[0] + (*single_line)[1] + (*single_line)[2], line_width));
    for (const auto &row : glossed_rows)
      require(near(row.bounds.top, glossed_rows[0].bounds.top));
    require(glossed_rows[0].item.translation.below &&
            glossed_rows[0].item.translation.width < 200.0);
    // The candidate lines alone past the line leave no single line to keep.
    require(!candidate_single_line_columns({{400.0}, {400.0}}, 300.0, metrics));

    // Candidates whose own lines pass half the work area widen the card past it, up to the single-line cap, rather than start a second line.
    CandidateCardInput long_lines;
    long_lines.horizontal = true;
    long_lines.items = {{200.0, 0.0, 300.0}, {200.0}, {200.0}};
    long_lines.max_width = 300.0;
    long_lines.max_single_line_width = 2000.0;
    const auto long_card = candidate_card_size(long_lines);
    const double firm_width =
        candidate_single_line_minimum_width(long_lines.items, metrics);
    require(near(firm_width, 3.0 * (200.0 + metrics.number_and_bar +
                                    metrics.column_gap)));
    require(long_card.width > 300.0 && long_card.width < 2000.0);
    const auto long_rows = candidate_page_layout(long_lines.items,
                                                 long_card.width, metrics, true);
    for (const auto &row : long_rows)
      require(near(row.bounds.top, long_rows[0].bounds.top));
    // The single-line cap still binds: past it the page breaks as before.
    long_lines.max_single_line_width = 400.0;
    const auto capped = candidate_card_size(long_lines);
    require(near(capped.width, 400.0));
    const auto capped_rows =
        candidate_page_layout(long_lines.items, capped.width, metrics, true);
    require(capped_rows[2].bounds.top > capped_rows[0].bounds.top);
  }

  // Untrusted measurements and font sizes are rejected before any arithmetic.
  CandidateCardInput invalid;
  invalid.items.assign(10, {10.0});
  require(rejected(invalid));
  invalid = vertical;
  invalid.preedit_width = -1.0;
  require(rejected(invalid));
  invalid = vertical;
  invalid.items = {{60.0}, {std::nan("")}};
  require(rejected(invalid));
  invalid = vertical;
  invalid.items = {{60.0, std::nan("")}};
  require(rejected(invalid));
  invalid = vertical;
  invalid.items = {{60.0, 0.0, -1.0}};
  require(rejected(invalid));
  invalid = vertical;
  invalid.max_width = std::numeric_limits<double>::infinity();
  require(rejected(invalid));
  for (double font : {11.0, 33.0, std::nan("")}) {
    invalid = vertical;
    invalid.font_size = font;
    require(rejected(invalid));
    invalid = vertical;
    invalid.preedit_font_size = font;
    require(rejected(invalid));
  }

  // Placement. A 1920x1040 work area, caret two thirds down the screen.
  CandidatePlacementInput place;
  place.work_left = 0;
  place.work_top = 0;
  place.work_right = 1920;
  place.work_bottom = 1040;
  place.anchor_x = 400;
  place.anchor_y = 700;
  place.width = 300;
  place.height = 120;
  place.decision_height = 120;

  // Room below: the card sits under the line, one 3 DIP caret gap down.
  auto below = candidate_card_placement(place);
  require(!below.above);
  require(below.x == 400 && below.y == 703);

  // No room below: it flips above the line rather than being slid up over the
  // text. anchor_y is the line's bottom, so the card's lower edge lands one
  // line height above it - and crucially the card no longer covers anchor_y.
  place.anchor_y = 1000;
  auto above = candidate_card_placement(place);
  require(above.above);
  require(above.y == 1000 - 120 - 24);
  require(above.y + place.height < 1000);

  // The old behaviour was pure clamping, which is exactly what must not happen:
  // clamping would have parked it at work_bottom - height = 920, on top of the
  // line at 1000. Guard against a regression to that.
  require(above.y != 1040 - 120);

  // Horizontal edges are padded by 2 DIP, both sides.
  place.anchor_y = 700;
  place.anchor_x = 1900;
  require(candidate_card_placement(place).x == 1920 - 300 - 2);
  place.anchor_x = -50;
  require(candidate_card_placement(place).x == 2);

  // A caret above the work area is pushed down to the padded top edge.
  place.anchor_x = 400;
  place.anchor_y = -500;
  require(candidate_card_placement(place).y == 2);

  // Hysteresis: a short list that would fit below still flips when the tallest
  // list this composition would not, so a growing list does not jump sides.
  place.anchor_y = 880;
  place.height = 60;
  place.decision_height = 60;
  require(!candidate_card_placement(place).above); // 880+3+60 <= 1040
  place.decision_height = 300;                     // ...but the full list would not
  const auto sticky = candidate_card_placement(place);
  require(sticky.above);
  // Placed with its CURRENT height, not the decision height, so the card hugs
  // the line instead of leaving a 300px hole under a 60px card.
  require(sticky.y == 880 - 60 - 24);

  // decision_height below the real height cannot shrink the decision.
  place.decision_height = 0;
  place.height = 600;
  place.anchor_y = 1000;
  require(candidate_card_placement(place).above);

  // A card taller than the screen still lands inside the work area rather than
  // off the top, even flipped.
  place.height = 2000;
  place.decision_height = 2000;
  require(candidate_card_placement(place).y == 2);

  // Scale moves the DIP offsets with the display: gaps are 3 and 24 DIP.
  place.height = 120;
  place.decision_height = 120;
  place.anchor_y = 700;
  place.anchor_x = 400;
  place.scale = 2.0;
  require(candidate_card_placement(place).y == 700 + 6);
  place.anchor_y = 1000;
  require(candidate_card_placement(place).y == 1000 - 120 - 48);
  // A nonsense scale falls back to 1.0 rather than collapsing the gaps to zero.
  place.scale = 0.0;
  place.anchor_y = 700;
  require(candidate_card_placement(place).y == 703);

  // The vertical flip decision starts from the shipped presenter's 232 DIP seed (DEFAULT_WINDOW_HEIGHT_DIP), so the first short page of a composition already flips when a full one would not fit below; a taller list raises it, and the work area caps it.
  require(near(candidate_vertical_decision_seed_dip, 232.0));
  require(candidate_vertical_decision_height(0, 1.0, 1040) == 232);
  require(candidate_vertical_decision_height(80, 1.0, 1040) == 232);
  require(candidate_vertical_decision_height(300, 1.0, 1040) == 300);
  require(candidate_vertical_decision_height(0, 1.5, 1040) == 348);
  require(candidate_vertical_decision_height(0, 1.25, 1040) == 290);
  require(candidate_vertical_decision_height(0, 0.0, 1040) == 232);
  require(candidate_vertical_decision_height(0, 2.0, 400) == 400);
  {
    CandidatePlacementInput seeded;
    seeded.work_right = 1920;
    seeded.work_bottom = 1040;
    seeded.anchor_x = 400;
    seeded.anchor_y = 900;
    seeded.width = 200;
    seeded.height = 80;
    seeded.decision_height =
        static_cast<int>(candidate_vertical_decision_height(80, 1.0, 1040));
    // 900 + 3 + 80 fits below, but 900 + 3 + 232 does not.
    require(candidate_card_placement(seeded).above);
    seeded.anchor_y = 700;
    require(!candidate_card_placement(seeded).above);
  }

  // An external skin package may ask for a wider card than the font implies:
  // the artwork is drawn against that width, and a narrower card makes the
  // decoration overhang it.
  {
    CandidateCardInput skinned;
    skinned.items = {{40.0}};
    const auto plain = candidate_card_size(skinned);
    skinned.skin_min_width = plain.width + 120.0;
    const auto wide = candidate_card_size(skinned);
    require(near(wide.width, plain.width + 120.0));
    // Height is untouched by a width floor.
    require(near(wide.height, plain.height));

    // The floor only raises: a package must not be able to shrink the card
    // below what its own text needs.
    skinned.skin_min_width = 10.0;
    require(near(candidate_card_size(skinned).width, plain.width));
    skinned.skin_min_width = 0.0;
    require(near(candidate_card_size(skinned).width, plain.width));

    // Nonsense values are ignored rather than propagated into the geometry.
    skinned.skin_min_width = -50.0;
    require(near(candidate_card_size(skinned).width, plain.width));
    skinned.skin_min_width = std::numeric_limits<double>::quiet_NaN();
    require(near(candidate_card_size(skinned).width, plain.width));

    // The work area cap still wins over the skin's floor: a card must not be
    // pushed wider than the screen it has to fit on.
    skinned.skin_min_width = 5000.0;
    skinned.max_width = plain.width + 40.0;
    require(near(candidate_card_size(skinned).width, plain.width + 40.0));
  }

  // Annotation and translation runs, spaced as the shipped presenter spaces them: the annotation 4 DIP after the text, the translation at 0.78 of the size and 0.65 of it away.
  {
    const double row = row16;
    const double base = 6.0 + 6.0 + (16.0 * 1.4 + 6.0);
    const double translation_line = 16.0 * 0.78 * 1.25;
    require(near(metrics.translation_font, 16.0 * 0.78) &&
            near(metrics.translation_gap, 16.0 * 0.65) &&
            near(metrics.translation_line, translation_line) &&
            near(metrics.annotation_gap, 4.0) &&
            near(metrics.annotation_line, 16.0 * 1.25));

    // Vertical: both runs fit on the text's line, so the card only widens.
    CandidateCardInput runs;
    runs.preedit_width = 40.0;
    runs.items = {{60.0, 20.0, 40.0}};
    const auto one_line = candidate_card_size(runs);
    require(near(one_line.width,
                 60.0 + 4.0 + 20.0 + 16.0 * 0.65 + 40.0 + 24.0 + 12.0 + 14.0));
    require(near(one_line.height, base + row));
    const auto inline_rows =
        candidate_page_layout(runs.items, one_line.width, metrics, false);
    const auto &inline_item = inline_rows[0].item;
    require(!inline_item.annotation.below && !inline_item.translation.below);
    require(near(inline_item.annotation.x, 64.0) &&
            near(inline_item.translation.x, 84.0 + 16.0 * 0.65));
    require(near(inline_item.height, row));

    // Horizontal: the translation always goes under the text, and the column is as wide as the wider of the two lines.
    runs.horizontal = true;
    const auto stacked_runs = candidate_card_size(runs);
    // The column asks for 84 + 24 + 8 + 12 + 14 = 142 DIP, under the 160 DIP floor.
    require(near(stacked_runs.width, 160.0));
    require(near(stacked_runs.height, base + row + translation_line));
    const auto under = candidate_page_layout(runs.items, stacked_runs.width,
                                             metrics, true)[0].item;
    require(!under.annotation.below && under.translation.below);
    require(near(under.translation.x, 0.0) && near(under.translation.y, row));

    // A translation run of two lines (a Korean Hanja's 훈음 with its translation under it) goes under the text even in a vertical row where one line would fit beside it, takes both lines, and widens the row only as far as its widest line.
    {
      CandidateCardInput hanja;
      hanja.preedit_width = 40.0;
      hanja.items = {{20.0, 0.0, 120.0, 2}};
      const auto two_lines = candidate_card_size(hanja);
      require(near(two_lines.height, base + row + 2.0 * translation_line));
      require(near(two_lines.width, 120.0 + 24.0 + 12.0 + 14.0));
      require(near(candidate_item_natural_width(hanja.items[0], metrics, false),
                   120.0 + metrics.number_and_bar));
      const auto stacked =
          candidate_page_layout(hanja.items, two_lines.width, metrics, false)[0].item;
      require(stacked.translation.below && near(stacked.translation.x, 0.0) &&
              near(stacked.translation.y, row) &&
              near(stacked.translation.height, 2.0 * translation_line));
      // The measure is asked even though no line is wider than the column, and its height wins.
      size_t asked = 0;
      hanja.wrapped = [&](size_t, CandidateRun run, double) {
        asked += run == CandidateRun::translation;
        return 40.0;
      };
      require(near(candidate_card_size(hanja).height, base + row + 40.0) && asked > 0);
      // One line keeps the ordinary rule and stays beside the text.
      hanja.wrapped = {};
      hanja.items = {{20.0, 0.0, 120.0}};
      const auto beside =
          candidate_page_layout(hanja.items, 400.0, metrics, false)[0].item;
      require(!beside.translation.below && near(beside.height, row));
    }

    // A capped card wraps the translation under the text instead of clipping it, and grows by the wrapped height. Without a measure the lines are estimated from the single-line width.
    CandidateCardInput capped_runs;
    capped_runs.preedit_width = 40.0;
    capped_runs.max_width = 150.0;
    capped_runs.items = {{60.0, 0.0, 300.0}};
    const double content = 150.0 - 12.0 - 24.0;
    const auto estimated = candidate_card_size(capped_runs);
    require(near(estimated.width, 150.0));
    require(near(estimated.height, base + row + 3.0 * translation_line));

    // With a measure, the measured height wins, and it is asked for at the column width the run will be drawn in.
    size_t asked_index = 99;
    CandidateRun asked_run = CandidateRun::annotation;
    double asked_width = 0.0;
    capped_runs.wrapped = [&](size_t index, CandidateRun run, double width) {
      asked_index = index;
      asked_run = run;
      asked_width = width;
      return 20.0;
    };
    require(near(candidate_card_size(capped_runs).height, base + row + 20.0));
    require(asked_index == 0 && asked_run == CandidateRun::translation &&
            near(asked_width, content));
    // A measurement below one line, or no usable measurement, still reserves the line.
    capped_runs.wrapped = [](size_t, CandidateRun, double) { return 2.0; };
    require(near(candidate_card_size(capped_runs).height,
                 base + row + translation_line));
    capped_runs.wrapped = [](size_t, CandidateRun, double) {
      return std::nan("");
    };
    require(near(candidate_card_size(capped_runs).height,
                 base + row + translation_line));

    // An annotation that does not fit moves under the text, and the translation follows it down even though it would fit on the first line.
    capped_runs.wrapped = {};
    capped_runs.items = {{60.0, 200.0, 10.0}};
    const auto moved = candidate_card_size(capped_runs);
    const double moved_item = row + 2.0 * 16.0 * 1.25 + translation_line;
    require(near(moved.height, base + moved_item));
    const auto moved_rows =
        candidate_page_layout(capped_runs.items, 150.0, metrics, false);
    const auto &moved_layout = moved_rows[0].item;
    require(moved_layout.annotation.below && moved_layout.translation.below);
    require(near(moved_layout.annotation.y, row) &&
            near(moved_layout.annotation.width, content) &&
            near(moved_layout.translation.y, row + 2.0 * 16.0 * 1.25) &&
            near(moved_layout.translation.width, 10.0));

    // Rows of a vertical page stack at their own heights, and hit testing follows them: a click in the wrapped part of the first row is the first row, not the second.
    capped_runs.items = {{60.0, 200.0, 10.0}, {60.0}};
    const auto tall = candidate_card_size(capped_runs);
    require(near(tall.height, base + moved_item + gap + row));
    const auto grown =
        candidate_page_layout(capped_runs.items, 150.0, metrics, false);
    const auto first_line = candidate_row_bounds(0, 2, 150.0, metrics, false);
    require(near(grown[0].bounds.top, first_line.top) &&
            near(grown[0].bounds.bottom, first_line.top + moved_item) &&
            near(grown[1].bounds.top, grown[0].bounds.bottom + gap) &&
            near(grown[1].bounds.bottom, grown[1].bounds.top + row));
    require(candidate_card_hit(20.0, first_line.bottom + 1.0, 150.0,
                               tall.height, grown) == std::optional<size_t>(0));
    require(candidate_card_hit(20.0, grown[1].bounds.top + 1.0, 150.0,
                               tall.height, grown) == std::optional<size_t>(1));

    // Columns on one horizontal line all take the line's tallest height, so the selection fills evenly.
    const std::vector<CandidateItemWidths> side_by_side = {{60.0, 0.0, 300.0},
                                                           {60.0}};
    const auto spread =
        candidate_page_layout(side_by_side, 500.0, metrics, true);
    require(spread[0].item.height > row && near(spread[1].item.height, row));
    require(near(spread[1].bounds.left, spread[0].bounds.right) &&
            near(spread[0].bounds.bottom, spread[1].bounds.bottom) &&
            near(spread[0].bounds.bottom - spread[0].bounds.top,
                 spread[0].item.height));

    // Within a capped card the first candidate is narrowed to the line and its runs wrap inside that column, as in a vertical row; the second no longer fits beside it and starts the next line.
    const auto stacked_lines =
        candidate_page_layout(capped_runs.items, 150.0, metrics, true);
    require(near(stacked_lines[0].bounds.right - stacked_lines[0].bounds.left,
                 150.0 - metrics.pad_x));
    require(stacked_lines[0].item.annotation.below &&
            stacked_lines[0].item.translation.below);
    require(near(stacked_lines[1].bounds.top,
                 stacked_lines[0].bounds.bottom + gap) &&
            near(stacked_lines[1].bounds.left, metrics.pad_x / 2.0) &&
            near(stacked_lines[1].bounds.bottom - stacked_lines[1].bounds.top,
                 row));
    capped_runs.horizontal = true;
    require(near(candidate_card_size(capped_runs).height,
                 base + stacked_lines[0].item.height + gap + row));

    // Candidate text wider than the column wraps inside it instead of being clipped, as the shipped presenter's CandidateList::MeasureItem draws it: the row takes the wrapped height, sizing grows the card by it, and the row painted is the row hit testing reads.
    {
      const double text_line = 16.0 * 1.25;
      CandidateCardInput long_text;
      long_text.preedit_width = 40.0;
      long_text.max_width = 150.0;
      long_text.items = {{400.0}};
      // Without a measure the lines are estimated from the single-line width at the candidate size's line height.
      const auto estimated_text = candidate_card_size(long_text);
      require(near(estimated_text.width, 150.0));
      require(near(estimated_text.height,
                   base + std::ceil(400.0 / content) * text_line));
      const auto wrapped_rows =
          candidate_page_layout(long_text.items, 150.0, metrics, false);
      const auto &wrapped_item = wrapped_rows[0].item;
      require(wrapped_item.text_wrapped && near(wrapped_item.text_width, content) &&
              near(wrapped_item.text_height, 4.0 * text_line) &&
              near(wrapped_item.height, wrapped_item.text_height));
      require(near(wrapped_rows[0].bounds.bottom - wrapped_rows[0].bounds.top,
                   wrapped_item.height));

      // The measured height wins, asked for the text run at the column width the text is drawn in; a measurement below one row, or none, still keeps the row.
      size_t text_index = 99;
      CandidateRun text_run = CandidateRun::annotation;
      double text_width = 0.0;
      long_text.wrapped = [&](size_t index, CandidateRun run, double width) {
        text_index = index;
        text_run = run;
        text_width = width;
        return 70.0;
      };
      require(near(candidate_card_size(long_text).height, base + 70.0));
      require(text_index == 0 && text_run == CandidateRun::text &&
              near(text_width, content));
      long_text.wrapped = [](size_t, CandidateRun, double) { return 2.0; };
      require(near(candidate_card_size(long_text).height, base + row));
      long_text.wrapped = [](size_t, CandidateRun, double) {
        return std::numeric_limits<double>::infinity();
      };
      require(near(candidate_card_size(long_text).height, base + row));
      long_text.wrapped = {};

      // Text that fits keeps one line and is not measured wrapped at all.
      bool measured_short = false;
      const auto short_rows = candidate_page_layout(
          {{60.0}}, 150.0, metrics, false,
          [&](size_t, CandidateRun, double) {
            measured_short = true;
            return 70.0;
          });
      require(!measured_short && !short_rows[0].item.text_wrapped &&
              near(short_rows[0].item.text_height, row) &&
              near(short_rows[0].item.height, row));

      // Wrapped text fills the column, so an annotation moves under it and starts where the text's box ends.
      const auto text_then_runs = candidate_page_layout(
          {{400.0, 20.0, 10.0}}, 150.0, metrics, false)[0].item;
      require(text_then_runs.annotation.below &&
              text_then_runs.translation.below);
      require(near(text_then_runs.annotation.y, text_then_runs.text_height) &&
              near(text_then_runs.translation.y,
                   text_then_runs.text_height + text_line) &&
              near(text_then_runs.height,
                   text_then_runs.text_height + text_line + translation_line));

      // A vertical page stacks the grown row, and a click anywhere in the wrapped text is that candidate; the next row starts under it.
      long_text.items = {{400.0}, {60.0}};
      const auto two = candidate_card_size(long_text);
      require(near(two.height, base + 4.0 * text_line + gap + row));
      const auto stacked_text =
          candidate_page_layout(long_text.items, two.width, metrics, false);
      const auto one_line_row = candidate_row_bounds(0, 2, two.width, metrics, false);
      require(near(stacked_text[0].bounds.top, one_line_row.top) &&
              near(stacked_text[0].bounds.bottom,
                   one_line_row.top + 4.0 * text_line) &&
              near(stacked_text[1].bounds.top,
                   stacked_text[0].bounds.bottom + gap));
      // The paint rectangle of every row (its bounds, with the text box inside it) is exactly what hit testing resolves: its corners and centre select that row and nothing else, and the text box never leaves it.
      for (size_t index = 0; index < stacked_text.size(); ++index) {
        const auto &bounds = stacked_text[index].bounds;
        const auto &item = stacked_text[index].item;
        require(bounds.top + item.text_height <= bounds.bottom + 0.001);
        require(bounds.left + metrics.number_and_bar + item.text_width <=
                bounds.right + 0.001);
        for (double x : {bounds.left, (bounds.left + bounds.right) / 2.0,
                         bounds.right - 0.01})
          for (double y : {bounds.top, (bounds.top + bounds.bottom) / 2.0,
                           bounds.bottom - 0.01})
            require(candidate_card_hit(x, y, two.width, two.height,
                                       stacked_text) ==
                    std::optional<size_t>(index));
      }
      require(candidate_card_hit(20.0, one_line_row.bottom + 1.0, two.width,
                                 two.height, stacked_text) ==
              std::optional<size_t>(0));

      // Horizontal: a candidate wider than a whole line is narrowed to the line and its text wraps there; the line takes that height, the next candidate starts a new line under it, and the card grows by both.
      long_text.horizontal = true;
      const auto across = candidate_card_size(long_text);
      require(near(across.width, 150.0));
      require(near(across.height, base + 4.0 * text_line + gap + row));
      const auto columns_text =
          candidate_page_layout(long_text.items, across.width, metrics, true);
      require(columns_text[0].item.text_wrapped &&
              near(columns_text[0].bounds.right - columns_text[0].bounds.left,
                   150.0 - metrics.pad_x) &&
              near(columns_text[0].bounds.bottom - columns_text[0].bounds.top,
                   4.0 * text_line));
      require(!columns_text[1].item.text_wrapped &&
              near(columns_text[1].bounds.top,
                   columns_text[0].bounds.bottom + gap) &&
              near(columns_text[1].bounds.left, metrics.pad_x / 2.0));
      for (size_t index = 0; index < columns_text.size(); ++index) {
        const auto &bounds = columns_text[index].bounds;
        require(candidate_card_hit((bounds.left + bounds.right) / 2.0,
                                   bounds.bottom - 0.01, across.width,
                                   across.height, columns_text) ==
                std::optional<size_t>(index));
      }
    }

    // Pages are bounded like the card.
    bool caught = false;
    try {
      candidate_page_layout(std::vector<CandidateItemWidths>(10), 150.0,
                            metrics, false);
    } catch (const std::invalid_argument &) {
      caught = true;
    }
    require(caught);
  }

  // The Fluent selection pill: 40% of a one-line row tall, so it runs from 30% to 70% of that row, and it stays that height and centred when the row grows instead of stretching with it. Horizontally it is 3 DIP wide and centred on the row's left edge.
  {
    const double row_top = 30.0, row_left = 6.0;
    const auto single = candidate_selection_bar(
        row_left, row_top, row_top + metrics.candidate_row,
        metrics.candidate_row);
    require(near(candidate_selection_bar_width, 3.0) &&
            near(single.left, row_left - 1.5) &&
            near(single.right, row_left + 1.5));
    require(near(single.top, row_top + metrics.candidate_row * 0.3) &&
            near(single.bottom, row_top + metrics.candidate_row * 0.7));
    const double wrapped_bottom = row_top + metrics.candidate_row * 3.0;
    const auto tall = candidate_selection_bar(row_left, row_top, wrapped_bottom,
                                              metrics.candidate_row);
    require(near(tall.bottom - tall.top, single.bottom - single.top));
    require(near((tall.top + tall.bottom) / 2.0,
                 (row_top + wrapped_bottom) / 2.0));
    // The height follows the line height, not the row.
    const auto large_line = candidate_selection_bar(0.0, 0.0, 200.0, 45.2);
    require(near(large_line.bottom - large_line.top, 45.2 * 0.4));
    // Never shorter than twice its width, so it still reads as a pill on a tiny line.
    const auto tiny = candidate_selection_bar(0.0, 0.0, 10.0, 5.0);
    require(near(tiny.bottom - tiny.top, 6.0));
    // A row shorter than the pill starts it at the row top.
    const auto squeezed = candidate_selection_bar(0.0, 10.0, 15.0, 23.6);
    require(near(squeezed.top, 10.0) &&
            near(squeezed.bottom - squeezed.top, 23.6 * 0.4));
  }

  // The pager shares the preedit row: the page indicator, then the previous and next arrows, right aligned to the rows' right edge. `metrics` is the 16 DIP card with its preedit row visible.
  {
    require(near(metrics.pager_font, 13.0) && near(metrics.pager_gap, 12.0) &&
            near(metrics.pager_arrow, 16.0));
    require(near(candidate_pager_width(30.0, metrics), 30.0 + 12.0 + 32.0));
    require(near(candidate_pager_width(0.0, metrics), 0.0) &&
            near(candidate_pager_width(std::nan(""), metrics), 0.0));
    const auto pager = candidate_pager_layout(200.0, 30.0, metrics);
    require(pager.has_value());
    require(near(pager->next.right, 200.0 - 6.0) &&
            near(pager->next.left, 200.0 - 6.0 - 16.0) &&
            near(pager->previous.right, pager->next.left) &&
            near(pager->previous.left, pager->next.left - 16.0) &&
            near(pager->indicator.right, pager->previous.left - 12.0) &&
            near(pager->indicator.left, pager->indicator.right - 30.0) &&
            near(pager->left, pager->indicator.left));
    require(near(pager->next.top, metrics.pad_y) &&
            near(pager->next.bottom, metrics.pad_y + metrics.preedit_row));
    // No indicator or no card draws no pager. A hidden preedit still leaves the row, so the pager stays in it at the row's height.
    require(!candidate_pager_layout(200.0, 0.0, metrics));
    const auto hidden_pager = candidate_pager_layout(
        200.0, 30.0, candidate_card_metrics(16.0, 16.0, false));
    require(hidden_pager && near(hidden_pager->next.bottom, 6.0 + 22.0) &&
            near(hidden_pager->left, pager->left));
    require(!candidate_pager_layout(0.0, 30.0, metrics));

    // The arrows are the click targets. The previous one is inert on the first page; the next one is always live, because the page count grows as the user pages.
    const double middle = metrics.pad_y + metrics.preedit_row / 2.0;
    const double previous_x = (pager->previous.left + pager->previous.right) / 2.0;
    const double next_x = (pager->next.left + pager->next.right) / 2.0;
    require(candidate_pager_hit(previous_x, middle, pager, false) == true);
    require(!candidate_pager_hit(previous_x, middle, pager, true));
    require(candidate_pager_hit(next_x, middle, pager, true) == false);
    require(!candidate_pager_hit(pager->indicator.left + 1.0, middle, pager, false));
    require(!candidate_pager_hit(next_x, metrics.pad_y + metrics.preedit_row + 1.0,
                                 pager, false));
    require(!candidate_pager_hit(next_x, middle, std::nullopt, false));

    // Sizing makes room for the preedit, the gap and the pager, so the preedit is not clipped under the indicator.
    CandidateCardInput paged;
    paged.preedit_width = 120.0;
    paged.page_width = 30.0;
    paged.items = {{40.0}};
    const auto card = candidate_card_size(paged);
    require(near(card.width, 16.0 + 6.0 + 120.0 + 6.0 + 12.0 +
                                 (30.0 + 12.0 + 32.0) + 12.0 + 14.0));
    const auto drawn = candidate_pager_layout(card.width, 30.0, metrics);
    require(drawn && candidate_preedit_left(metrics) + 120.0 <=
                         drawn->left - metrics.pager_gap);
    // With the preedit hidden the pager keeps its room after the mark, so a long indicator cannot run over it.
    paged.preedit_visible = false;
    paged.page_width = 200.0;
    const auto hidden_card = candidate_card_size(paged);
    require(near(hidden_card.width,
                 16.0 + 6.0 + 12.0 + (200.0 + 12.0 + 32.0) + 12.0 + 14.0));
    const auto hidden_drawn = candidate_pager_layout(
        hidden_card.width, 200.0, candidate_card_metrics(16.0, 16.0, false));
    require(hidden_drawn &&
            candidate_logo_bounds(metrics).right <= hidden_drawn->left);
    // An unusable indicator measurement is refused like any other.
    paged.page_width = -1.0;
    require(rejected(paged));
  }
}

#include "../../src/candidate/CandidateMenuLayout.h"

#include <cassert>
#include <iostream>
#include <set>
#include <string>

using namespace msime::windows;

namespace {
// Removing a single character from the dictionary would leave the user unable
// to type it at all, so the shipped menu omits the row rather than offering it.
void delete_is_offered_only_for_words() {
  const auto single = candidate_menu_items(1);
  for (const auto &item : single)
    assert(item.command != CandidateMenuCommand::Remove);
  assert(single.size() == 2);
  for (size_t code_points : {0u, 2u, 3u, 8u}) {
    const auto many = candidate_menu_items(code_points);
    assert(many.size() == 3);
    assert(many.back().command == CandidateMenuCommand::Remove);
  }
}

// 固定排位 opens the submenu and is not itself a command; the rest are.
void only_the_fix_row_carries_a_submenu() {
  const auto items = candidate_menu_items(2);
  size_t submenus = 0;
  for (const auto &item : items) {
    if (item.submenu) {
      ++submenus;
      assert(item.command == CandidateMenuCommand::FixPosition);
    }
    assert(!item.separator);
    assert(!item.label.empty());
  }
  assert(submenus == 1);
}

// Display-only candidates keep the same menu shape, but every dictionary
// action must be inert; the Server remains a second validation boundary.
void display_only_candidates_are_inert() {
  const auto items = candidate_menu_items(2, false);
  for (const auto &item : items)
    assert(!item.available);
  const auto metrics = CandidateMenuMetrics{};
  for (size_t i = 0; i < items.size(); ++i) {
    const auto row = candidate_menu_row(i, items, metrics);
    assert(!candidate_menu_hit(metrics.width / 2.0, row.top + 1.0, items,
                               metrics));
  }
  const auto submenu = candidate_menu_submenu_items(false, 3);
  for (const auto &item : submenu)
    assert(!item.available);
}

// The submenu offers exactly the five positions the protocol accepts, each
// distinct, plus 取消固定 under a separator.
void submenu_offers_five_positions_and_a_clear() {
  const auto items = candidate_menu_submenu_items();
  std::set<unsigned> positions;
  size_t separators = 0;
  size_t clears = 0;
  for (const auto &item : items) {
    if (item.separator) {
      ++separators;
      // A separator is a line; it must carry no label to draw.
      assert(item.label.empty());
      continue;
    }
    assert(!item.label.empty());
    assert(!item.submenu);
    if (item.command == CandidateMenuCommand::FixAtPosition) {
      assert(item.position >= 1 && item.position <= 5);
      positions.insert(item.position);
    } else {
      assert(item.command == CandidateMenuCommand::ClearFixedPosition);
      ++clears;
    }
  }
  assert(positions.size() == 5);
  assert(separators == 1);
  assert(clears == 1);
  // 取消固定 comes last, after the separator.
  assert(!items.back().separator);
  assert(items.back().command == CandidateMenuCommand::ClearFixedPosition);
}

// Rows have to tile the card exactly, or a click lands between two of them.
void rows_tile_the_card_without_gaps() {
  const CandidateMenuMetrics metrics;
  for (const auto &items :
       {candidate_menu_items(1), candidate_menu_items(3),
        candidate_menu_submenu_items()}) {
    const auto size = candidate_menu_size(items, metrics);
    assert(candidate_menu_row(0, items, metrics).top == metrics.padding);
    for (size_t i = 1; i < items.size(); ++i)
      assert(candidate_menu_row(i - 1, items, metrics).bottom ==
             candidate_menu_row(i, items, metrics).top);
    // The last row ends exactly one padding short of the card's bottom.
    assert(candidate_menu_row(items.size() - 1, items, metrics).bottom +
               metrics.padding ==
           size.height);
  }
}

// A separator is a line, not a command. Treating it as a hit would let a click
// land on whatever row happens to follow it - here, 取消固定.
void separators_are_never_hit() {
  const CandidateMenuMetrics metrics;
  const auto items = candidate_menu_submenu_items();
  size_t separator = items.size();
  for (size_t i = 0; i < items.size(); ++i)
    if (items[i].separator)
      separator = i;
  assert(separator < items.size());
  const auto row = candidate_menu_row(separator, items, metrics);
  for (double y : {row.top, (row.top + row.bottom) / 2.0, row.bottom - 0.01})
    assert(!candidate_menu_hit(metrics.width / 2.0, y, items, metrics));
}

// Every drawn row is selectable across its whole height and width, and nothing
// outside the card is.
void hit_testing_matches_the_drawn_rows() {
  const CandidateMenuMetrics metrics;
  const auto items = candidate_menu_items(3);
  const auto size = candidate_menu_size(items, metrics);
  for (size_t i = 0; i < items.size(); ++i) {
    const auto row = candidate_menu_row(i, items, metrics);
    for (double y : {row.top, (row.top + row.bottom) / 2.0, row.bottom - 0.01})
      for (double x : {0.0, metrics.width / 2.0, size.width - 0.01}) {
        const auto hit = candidate_menu_hit(x, y, items, metrics);
        assert(hit && *hit == i);
      }
  }
  // Outside, including the padding above the first row and below the last.
  assert(!candidate_menu_hit(-1.0, 10.0, items, metrics));
  assert(!candidate_menu_hit(size.width, 10.0, items, metrics));
  assert(!candidate_menu_hit(10.0, -1.0, items, metrics));
  assert(!candidate_menu_hit(10.0, size.height, items, metrics));
  assert(!candidate_menu_hit(10.0, metrics.padding / 2.0, items, metrics));
  assert(!candidate_menu_hit(10.0, size.height - metrics.padding / 2.0, items,
                             metrics));
}

// The flyout opens at the pointer, and flips rather than clamping flush: a
// menu jammed against the edge with a row under the cursor would select
// something the moment the button comes up.
void the_flyout_flips_instead_of_clamping() {
  const CandidateMenuMetrics metrics;
  const auto size = candidate_menu_size(candidate_menu_items(3), metrics);
  const auto roomy = candidate_menu_bounds(400, 300, 0, 0, 1920, 1080, 96, size);
  assert(roomy.x == 400 && roomy.y == 300);

  // Against the right and bottom edges it opens up and to the left, so the
  // pointer ends up outside it rather than on its first row.
  const auto corner =
      candidate_menu_bounds(1910, 1070, 0, 0, 1920, 1080, 96, size);
  assert(corner.x + corner.width <= 1920);
  assert(corner.y + corner.height <= 1080);
  assert(corner.x < 1910);
  assert(corner.y < 1070);

  // Never outside the monitor, whatever the pointer claims.
  for (int x : {-5000, 0, 1919, 5000})
    for (int y : {-5000, 0, 1079, 5000}) {
      const auto bounds =
          candidate_menu_bounds(x, y, 0, 0, 1920, 1080, 96, size);
      assert(bounds.x >= 0 && bounds.y >= 0);
      assert(bounds.x + bounds.width <= 1920);
      assert(bounds.y + bounds.height <= 1080);
    }
  // A monitor that is not at the origin.
  const auto second =
      candidate_menu_bounds(2200, 400, 1920, 0, 3840, 1080, 96, size);
  assert(second.x >= 1920 && second.x + second.width <= 3840);
}

// The submenu sits beside its parent row and must not cover it: the pointer
// has to stay on that row to keep the submenu open.
void the_submenu_opens_beside_its_parent() {
  const CandidateMenuMetrics metrics;
  const auto size = candidate_menu_size(candidate_menu_submenu_items(), metrics);
  const auto beside =
      candidate_submenu_bounds(400, 508, 320, 0, 0, 1920, 1080, 96, size);
  assert(beside.x == 508);
  assert(beside.y == 320);

  // No room on the right: it flips to the parent's left, still not overlapping.
  const auto flipped =
      candidate_submenu_bounds(1800, 1908, 320, 0, 0, 1920, 1080, 96, size);
  assert(flipped.x + flipped.width <= 1800);
  assert(flipped.x >= 0);

  // A row near the bottom pulls the submenu up so all of it stays on screen.
  const auto low =
      candidate_submenu_bounds(400, 508, 1070, 0, 0, 1920, 1080, 96, size);
  assert(low.y + low.height <= 1080);
}

// Bad geometry must be refused rather than producing a zero-size or offscreen
// window that the user cannot see or dismiss.
void invalid_placement_is_refused() {
  const CandidateMenuMetrics metrics;
  const auto size = candidate_menu_size(candidate_menu_items(2), metrics);
  for (unsigned dpi : {0u, 47u, 961u}) {
    bool threw = false;
    try {
      candidate_menu_bounds(10, 10, 0, 0, 1920, 1080, dpi, size);
    } catch (const std::invalid_argument &) {
      threw = true;
    }
    assert(threw);
  }
  bool threw = false;
  try {
    candidate_menu_bounds(10, 10, 100, 0, 100, 1080, 96, size);
  } catch (const std::invalid_argument &) {
    threw = true;
  }
  assert(threw);
  // An empty menu has nothing to show and no size to compute.
  threw = false;
  try {
    candidate_menu_size({}, metrics);
  } catch (const std::invalid_argument &) {
    threw = true;
  }
  assert(threw);
  assert(!candidate_menu_hit(0.0, 0.0, {}, metrics));
}

// 「删除」行提示 Ctrl+Shift+Alt+N，N 是候选在本页的位置；第 9 个候选没有这个快捷键，不提示。有提示时菜单在右侧多留一栏，提示那一栏照样可以点。
void delete_row_shows_its_shortcut() {
  const CandidateMenuMetrics metrics;
  const auto plain = candidate_menu_items(2);
  for (const auto &item : plain)
    assert(item.shortcut.empty());
  for (size_t slot = 0; slot < 8; ++slot) {
    const auto items = candidate_menu_items(2, true, slot);
    assert(items.back().command == CandidateMenuCommand::Remove &&
           items.back().shortcut == "Ctrl+Shift+Alt+" + std::to_string(slot + 1));
    assert(items.front().shortcut.empty() && items[1].shortcut.empty());
  }
  assert(candidate_menu_items(2, true, 8).back().shortcut.empty());
  // 单个字没有「删除」行，也就没有提示。
  for (const auto &item : candidate_menu_items(1, true, 0))
    assert(item.shortcut.empty());
  const auto hinted = candidate_menu_items(3, true, 2);
  const auto size = candidate_menu_size(hinted, metrics);
  assert(size.width == metrics.width + metrics.shortcut_column);
  assert(candidate_menu_size(plain, metrics).width == metrics.width);
  const auto remove = candidate_menu_row(2, hinted, metrics);
  assert(candidate_menu_hit(size.width - 2.0, remove.top + 1.0, hinted, metrics) ==
         std::optional<size_t>(2));
}

// Each chosen row maps to the Engine action it asks for; 固定排位 itself only opens the submenu.
void chosen_rows_map_to_engine_actions() {
  assert(candidate_menu_action(CandidateMenuCommand::PinToTop, 0)->action ==
         CandidateAction::Pin);
  assert(candidate_menu_action(CandidateMenuCommand::Remove, 0)->action ==
         CandidateAction::Remove);
  const auto fix = candidate_menu_action(CandidateMenuCommand::FixAtPosition, 4);
  assert(fix && fix->action == CandidateAction::FixPosition && fix->position == 4);
  assert(!candidate_menu_action(CandidateMenuCommand::FixAtPosition, 0));
  assert(!candidate_menu_action(CandidateMenuCommand::FixAtPosition, 6));
  assert(candidate_menu_action(CandidateMenuCommand::ClearFixedPosition, 0)
             ->action == CandidateAction::ClearPosition);
  assert(!candidate_menu_action(CandidateMenuCommand::FixPosition, 0));
}

// The flyout is reused across right clicks, so a choice must reach the candidate of the opening in progress, never the one that first created the menu.
struct TestClick {
  uint64_t session, generation;
  size_t index;
  CandidateAction action = CandidateAction::Select;
  uint8_t position = 0;
};
void every_opening_acts_on_its_own_candidate() {
  CandidateMenuTarget<TestClick> target;
  assert(!target.choose(CandidateMenuCommand::PinToTop, 0));

  target.open({1, 1, 0});
  const auto first = target.choose(CandidateMenuCommand::PinToTop, 0);
  assert(first && first->session == 1 && first->index == 0 &&
         first->action == CandidateAction::Pin);

  // A later right click, in a newer generation and on another row.
  target.open({1, 2, 3});
  const auto second = target.choose(CandidateMenuCommand::Remove, 0);
  assert(second && second->generation == 2 && second->index == 3 &&
         second->action == CandidateAction::Remove);

  // An opening that was never chosen from is replaced by the next one.
  target.open({2, 5, 1});
  target.open({2, 6, 4});
  const auto fixed = target.choose(CandidateMenuCommand::FixAtPosition, 2);
  assert(fixed && fixed->session == 2 && fixed->generation == 6 &&
         fixed->index == 4 && fixed->position == 2);

  // One choice per opening: the menu closes after it.
  assert(!target.choose(CandidateMenuCommand::PinToTop, 0));

  // The submenu row carries no action and leaves the opening in place.
  target.open({3, 1, 2});
  assert(!target.choose(CandidateMenuCommand::FixPosition, 0));
  assert(target.choose(CandidateMenuCommand::ClearFixedPosition, 0)->index == 2);
}
} // namespace

// 候选旁有释义时，菜单最后隔一条线加「加入生词本」：云候选、AI 候选这些词库操作不可用的行也能点，选中它不映射到任何 Engine 动作。
void personal_wordbook_row_follows_the_dictionary_rows() {
  const CandidateMenuMetrics metrics;
  for (bool actionable : {true, false}) {
    const auto items = candidate_menu_items(2, actionable, 0, true);
    assert(items.size() == 5);
    assert(items[3].separator && items[3].command == CandidateMenuCommand::AddToPersonalWordbook);
    assert(items[4].command == CandidateMenuCommand::AddToPersonalWordbook &&
           items[4].label == "加入生词本" && items[4].available && !items[4].submenu);
    const auto row = candidate_menu_row(4, items, metrics);
    assert(candidate_menu_hit(4.0, row.top + 1.0, items, metrics) == std::optional<size_t>(4));
  }
  assert(candidate_menu_items(1, true, std::nullopt, true).size() == 4);
  assert(candidate_menu_items(2).size() == 3);
  assert(!candidate_menu_action(CandidateMenuCommand::AddToPersonalWordbook, 0));
}

// 有能读的英文时，同一组里「朗读」排在「加入生词本」之前；只有它时这一组只有「朗读」。选中它同样不映射到 Engine 动作。
void read_aloud_row_leads_the_last_group() {
  const CandidateMenuMetrics metrics;
  for (bool actionable : {true, false}) {
    const auto items = candidate_menu_items(2, actionable, 0, true, true);
    assert(items.size() == 6);
    assert(items[3].separator);
    assert(items[4].command == CandidateMenuCommand::ReadAloud && items[4].label == "朗读" &&
           items[4].available && !items[4].submenu);
    assert(items[5].command == CandidateMenuCommand::AddToPersonalWordbook);
    const auto row = candidate_menu_row(4, items, metrics);
    assert(candidate_menu_hit(4.0, row.top + 1.0, items, metrics) == std::optional<size_t>(4));
  }
  const auto alone = candidate_menu_items(5, true, std::nullopt, false, true);
  assert(alone.size() == 5);
  assert(alone[3].separator && alone[4].command == CandidateMenuCommand::ReadAloud);
  assert(candidate_menu_items(2, true, 0, true, false).size() == 5);
  assert(!candidate_menu_action(CandidateMenuCommand::ReadAloud, 0));
}

int main() {
  read_aloud_row_leads_the_last_group();
  personal_wordbook_row_follows_the_dictionary_rows();
  chosen_rows_map_to_engine_actions();
  delete_row_shows_its_shortcut();
  every_opening_acts_on_its_own_candidate();
  delete_is_offered_only_for_words();
  only_the_fix_row_carries_a_submenu();
  display_only_candidates_are_inert();
  submenu_offers_five_positions_and_a_clear();
  rows_tile_the_card_without_gaps();
  separators_are_never_hit();
  hit_testing_matches_the_drawn_rows();
  the_flyout_flips_instead_of_clamping();
  the_submenu_opens_beside_its_parent();
  invalid_placement_is_refused();
  std::cout << "Candidate menu: contents, geometry and placement\n";
  return 0;
}

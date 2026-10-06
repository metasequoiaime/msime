#pragma once
#include "CandidateAction.h"
#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace msime::windows {
// The candidate card's right-click menu: contents, geometry and hit testing.
//
// The menu itself used to be a TrackPopupMenuEx, which runs a nested modal
// message loop. The Server's pump is a bounded PeekMessage batch that also
// applies preference changes, syncs Caps Lock and drives the toolbar, so for
// as long as the menu was open none of that ran. The shipped presenter uses a
// non-modal flyout instead, and this header is the part of it that decides
// what the menu offers and where each row sits - testable without a desktop,
// the same split TrayMenuLayout uses.
enum class CandidateMenuCommand {
  PinToTop,
  FixPosition, // Opens the submenu; never itself a chosen command.
  Remove,
  FixAtPosition, // Carries `position`.
  ClearFixedPosition,
};
struct CandidateMenuItem {
  CandidateMenuCommand command;
  std::string label;
  // Rows that open a submenu draw a chevron and cannot be chosen themselves.
  bool submenu = false;
  // A separator is drawn as a line and can never be hit.
  bool separator = false;
  // Dictionary actions are shown but inert for cloud/AI/Japanese candidates.
  bool available = true;
  // 1-5 for FixAtPosition, otherwise 0.
  unsigned position = 0;
};

// The top-level rows for one candidate.
//
// 删除 is offered only for multi-character words. Removing a single character
// from the dictionary would leave the user unable to type it at all, which is
// why the shipped menu hides the row rather than disabling it.
inline std::vector<CandidateMenuItem>
candidate_menu_items(size_t code_points, bool actionable = true) {
  std::vector<CandidateMenuItem> items;
  items.reserve(3);
  items.push_back({CandidateMenuCommand::PinToTop, "置顶", false, false, actionable});
  items.push_back({CandidateMenuCommand::FixPosition, "固定排位", true, false, actionable});
  if (code_points != 1)
    items.push_back({CandidateMenuCommand::Remove, "删除", false, false,
                     actionable});
  return items;
}

// The 固定排位 submenu: the five positions, then 取消固定 below a separator.
inline std::vector<CandidateMenuItem>
candidate_menu_submenu_items(bool actionable = true, int fixed_position = 0) {
  std::vector<CandidateMenuItem> items;
  items.reserve(7);
  for (unsigned position = 1; position <= 5; ++position)
    items.push_back({CandidateMenuCommand::FixAtPosition,
                     "第 " + std::to_string(position) + " 位", false, false,
                     actionable, position});
  items.push_back({CandidateMenuCommand::ClearFixedPosition, "", false, true,
                   actionable && fixed_position > 0});
  items.push_back({CandidateMenuCommand::ClearFixedPosition, "取消固定", false,
                   false, actionable && fixed_position > 0});
  return items;
}

// What a chosen row asks the Engine to do. 固定排位 only opens the submenu, so it maps to nothing; so does a position outside the five slots the protocol accepts.
struct CandidateMenuAction {
  CandidateAction action;
  uint8_t position = 0;
};
inline std::optional<CandidateMenuAction>
candidate_menu_action(CandidateMenuCommand command, unsigned position) {
  switch (command) {
  case CandidateMenuCommand::PinToTop:
    return CandidateMenuAction{CandidateAction::Pin, 0};
  case CandidateMenuCommand::Remove:
    return CandidateMenuAction{CandidateAction::Remove, 0};
  case CandidateMenuCommand::FixAtPosition:
    if (position < 1 || position > 5)
      return std::nullopt;
    return CandidateMenuAction{CandidateAction::FixPosition,
                               static_cast<uint8_t>(position)};
  case CandidateMenuCommand::ClearFixedPosition:
    return CandidateMenuAction{CandidateAction::ClearPosition, 0};
  case CandidateMenuCommand::FixPosition:
    break;
  }
  return std::nullopt;
}

// The candidate the open menu acts on. The flyout is built once and reused for every right click, so its choice callback must not capture the click that first created it: that is how later 置顶 / 删除 / 固定排位 used to reach whichever candidate was right-clicked first, in a session and generation that had long since moved on. Each opening records its own target here and a choice consumes it, as the reference gets by rebuilding its menu on every open (candidate_presenter.cpp:560-563, :698-765). A template only so the rule is testable without the IPC contract that the Server's click type pulls in.
template <class Click> class CandidateMenuTarget final {
public:
  void open(const Click &target) { target_ = target; }
  // The click to send for this choice, or nothing when no menu is open or the row carries no action. One choice per opening: the menu closes after it.
  std::optional<Click> choose(CandidateMenuCommand command, unsigned position) {
    if (!target_)
      return std::nullopt;
    const auto action = candidate_menu_action(command, position);
    if (!action)
      return std::nullopt;
    Click click = *target_;
    target_.reset();
    click.action = action->action;
    click.position = action->position;
    return click;
  }

private:
  std::optional<Click> target_;
};

struct CandidateMenuMetrics {
  double width = 108.0;
  double row_height = 30.0;
  double separator_height = 9.0;
  double padding = 4.0;
  double radius = 8.0;
  double border_width = 1.0;
  double label_inset = 10.0;
  // The chevron drawn on a submenu row, at the trailing edge.
  double chevron_column = 16.0;
};

struct CandidateMenuSize {
  double width, height;
};
inline CandidateMenuSize
candidate_menu_size(const std::vector<CandidateMenuItem> &items,
                    const CandidateMenuMetrics &metrics) {
  if (items.empty() || items.size() > 16 || metrics.width <= 0.0 ||
      metrics.row_height <= 0.0 || metrics.padding < 0.0)
    throw std::invalid_argument("Invalid candidate menu metrics");
  double height = metrics.padding * 2.0;
  for (const auto &item : items)
    height += item.separator ? metrics.separator_height : metrics.row_height;
  return {metrics.width, height};
}

struct CandidateMenuRow {
  double top, bottom;
};
inline CandidateMenuRow
candidate_menu_row(size_t index, const std::vector<CandidateMenuItem> &items,
                   const CandidateMenuMetrics &metrics) {
  if (index >= items.size())
    throw std::invalid_argument("Invalid candidate menu row");
  double top = metrics.padding;
  for (size_t i = 0; i < index; ++i)
    top += items[i].separator ? metrics.separator_height : metrics.row_height;
  return {top, top + (items[index].separator ? metrics.separator_height
                                             : metrics.row_height)};
}

// The row under the pointer, or nothing. A separator is never a hit: it is a
// line, not a command, and treating it as one would let a click land on
// whatever row happened to follow it.
inline std::optional<size_t>
candidate_menu_hit(double x, double y,
                   const std::vector<CandidateMenuItem> &items,
                   const CandidateMenuMetrics &metrics) {
  if (items.empty())
    return std::nullopt;
  const auto size = candidate_menu_size(items, metrics);
  if (x < 0.0 || y < 0.0 || x >= size.width || y >= size.height)
    return std::nullopt;
  for (size_t index = 0; index < items.size(); ++index) {
    const auto row = candidate_menu_row(index, items, metrics);
    if (y >= row.top && y < row.bottom)
      return items[index].separator || !items[index].available ? std::nullopt
                                    : std::optional<size_t>(index);
  }
  return std::nullopt;
}

struct CandidateMenuBounds {
  int x, y, width, height;
};

// Place the flyout at the pointer, kept inside the monitor. It flips rather
// than being clamped flush: a menu jammed against the edge with its first row
// under the cursor selects something the moment the button comes up.
inline CandidateMenuBounds
candidate_menu_bounds(int pointer_x, int pointer_y, int left, int top,
                      int right, int bottom, unsigned dpi,
                      const CandidateMenuSize &size) {
  if (dpi < 48 || dpi > 960 || right <= left || bottom <= top)
    throw std::invalid_argument("Invalid candidate menu placement");
  const double scale = static_cast<double>(dpi) / 96.0;
  const auto width = (std::min)(static_cast<int>(size.width * scale + 0.5),
                                right - left);
  const auto height = (std::min)(static_cast<int>(size.height * scale + 0.5),
                                 bottom - top);
  int x = pointer_x;
  if (x + width > right)
    x = pointer_x - width;
  int y = pointer_y;
  if (y + height > bottom)
    y = pointer_y - height;
  return {(std::clamp)(x, left, right - width),
          (std::clamp)(y, top, bottom - height), width, height};
}

// Place the submenu beside its parent row, flipping to the other side when it
// would not fit. Overlapping the parent would hide the row the pointer has to
// stay on to keep the submenu open.
inline CandidateMenuBounds
candidate_submenu_bounds(int parent_left, int parent_right, int row_top,
                         int left, int top, int right, int bottom,
                         unsigned dpi, const CandidateMenuSize &size) {
  if (dpi < 48 || dpi > 960 || right <= left || bottom <= top ||
      parent_right <= parent_left)
    throw std::invalid_argument("Invalid candidate submenu placement");
  const double scale = static_cast<double>(dpi) / 96.0;
  const auto width = (std::min)(static_cast<int>(size.width * scale + 0.5),
                                right - left);
  const auto height = (std::min)(static_cast<int>(size.height * scale + 0.5),
                                 bottom - top);
  int x = parent_right;
  if (x + width > right)
    x = parent_left - width;
  return {(std::clamp)(x, left, right - width),
          (std::clamp)(row_top, top, bottom - height), width, height};
}
} // namespace msime::windows

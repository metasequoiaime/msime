#pragma once
#include "CandidateMenuLayout.h"
#include "CandidatePalette.h"
#include <functional>
#include <optional>
#include <vector>
// windows.h first: its DrawText macro has to reach the Direct2D declarations,
// and NOMINMAX keeps its min/max macros away from the standard library.
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
// What the user chose from a candidate's right-click menu.
struct CandidateMenuChoice {
  CandidateMenuCommand command;
  // 1-5 for FixAtPosition, otherwise 0.
  unsigned position = 0;
};

// The candidate card's right-click menu, as a non-modal flyout.
//
// This replaces TrackPopupMenuEx, which runs a nested modal message loop. The
// Server's pump is a bounded PeekMessage batch that also applies preference
// changes, syncs Caps Lock and drives the toolbar, so for as long as the menu
// was open none of that ran - the toolbar stopped following the mode, and a
// Caps Lock press went unreported until the menu closed.
//
// Like the tray menu it never takes focus from the application being typed
// into, so it has no focus to lose and cannot rely on WM_KILLFOCUS. It holds
// the mouse capture instead, which is what lets it see the click that should
// dismiss it.
class CandidateFlyoutWindow final {
public:
  // Runs the chosen command. The menu closes either way; a right-click menu
  // that stayed open after a click would sit over the candidates.
  using Chosen = std::function<void(const CandidateMenuChoice &)>;
  explicit CandidateFlyoutWindow(Chosen chosen);
  ~CandidateFlyoutWindow();
  CandidateFlyoutWindow(const CandidateFlyoutWindow &) = delete;
  CandidateFlyoutWindow &operator=(const CandidateFlyoutWindow &) = delete;
  // Share the candidate card's resolved tokens so one theme covers both.
  void set_palette(CandidatePalette palette);
  // Open at the pointer for a candidate of `code_points` characters. The
  // Engine may expose a candidate that is display-only; keep its dictionary
  // rows visible but inert instead of waiting for a rejected IPC action.
  // `slot` 是这个候选在本页的位置，「删除」行据此提示删除它的快捷键（candidate_menu_items）。`personal_wordbook` 为真时菜单最后有「加入生词本」，`read_aloud` 为真时同一组里先是「朗读」。
  bool open(int pointer_x, int pointer_y, size_t code_points,
            bool actions_available, int fixed_position,
            std::optional<size_t> slot = std::nullopt,
            bool personal_wordbook = false, bool read_aloud = false) noexcept;
  bool visible() const noexcept;
  void hide() noexcept;
  HWND handle() const noexcept { return menu_.window; }

private:
  // One drawn list: the menu itself, or its 固定排位 submenu. Both are popup
  // windows of the same class, so the geometry and drawing are written once.
  struct Panel {
    HWND window = nullptr;
    msimeui::DeviceResources device;
    std::vector<CandidateMenuItem> items;
    size_t hovered = static_cast<size_t>(-1);
    unsigned dpi = 0;
  };
  static LRESULT CALLBACK procedure(HWND window, UINT message, WPARAM wparam,
                                    LPARAM lparam) noexcept;
  Panel *panel_for(HWND window) noexcept;
  void create(Panel &panel);
  void show(Panel &panel, const CandidateMenuBounds &bounds,
            std::vector<CandidateMenuItem> items);
  void paint(Panel &panel);
  std::optional<size_t> hit(const Panel &panel, int x, int y) const;
  // Follow the pointer: open the submenu on its row, close it off any other.
  void track(Panel &panel, int x, int y);
  void choose(const Panel &panel, size_t index);
  void open_submenu();
  void close_submenu() noexcept;
  // True when the screen point is inside either panel, which is what decides
  // whether a captured click is a choice or a dismissal.
  bool contains(POINT screen) const noexcept;

  Chosen chosen_;
  Panel menu_;
  Panel submenu_;
  CandidatePalette palette_;
  CandidateMenuMetrics metrics_;
  // Set after any Direct2D failure; the menu then stays closed rather than
  // throwing on every right click.
  bool failed_ = false;
  bool capturing_ = false;
  bool actions_available_ = true;
  int fixed_position_ = 0;
};
} // namespace msime::windows

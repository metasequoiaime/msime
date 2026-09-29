#pragma once
#include "CandidatePalette.h"
#include "TrayMenuLayout.h"
#include <functional>
// windows.h first: its DrawText macro has to reach the Direct2D declarations,
// and NOMINMAX keeps its min/max macros away from the standard library.
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
// The tray menu the shipped language bar opens: a composed card of commands, shown on request and dismissed as soon as it loses the pointer or focus. It owns no input state and never takes focus from the application being typed into, so the mode rows still address the focused TIP.
class TrayMenuWindow final {
public:
  // Runs a chosen command. Returning false leaves the menu open so a failed
  // command does not look like it was accepted.
  using Command = std::function<bool(TrayMenuCommand)>;
  // The live state the rows show: the toolbar switch, the focused TIP's modes and the stored preferences, so each row shows what the Server reports rather than a guess.
  using State = std::function<TrayMenuState()>;
  TrayMenuWindow(TrayMenuCapabilities capabilities, Command command,
                 State state);
  ~TrayMenuWindow();
  TrayMenuWindow(const TrayMenuWindow &) = delete;
  TrayMenuWindow &operator=(const TrayMenuWindow &) = delete;
  // Open under the tray icon, in work area pixels.
  void show(int icon_center_x, int icon_top);
  // show() throws on Direct2D failure. Opening the menu must never take the
  // Server down, so callers use this and treat false as "no menu this time".
  bool open(int icon_center_x, int icon_top) noexcept;
  // The card is WS_EX_NOACTIVATE and never receives WM_KILLFOCUS, so the UI
  // pump polls the pointer to decide when to dismiss it.
  bool pointer_inside() const noexcept;
  void hide();
  void set_palette(CandidatePalette palette);
  bool visible() const;
  bool failed() const { return failed_; }
  HWND handle() const { return window_; }

private:
  // Resolved once per window; null when neither icon font is installed.
  const wchar_t *icon_family_ = nullptr;
  IDWriteTextFormat *icon_text_format_ = nullptr;
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void paint();
  std::optional<size_t> hit(int x, int y) const;
  void choose(size_t index);
  // Rebuild the rows and their geometry from the live state.
  void refresh_items();
  // The product mark for the header, loaded from the Server's resources at the size it is drawn. Null when the module carries no mark, as in the test executables; the header then shows its name alone.
  ID2D1Bitmap *logo_bitmap(int pixels);
  TrayMenuCapabilities capabilities_;
  Command command_;
  State state_;
  // Direct2D's imaging factory is a COM server; this thread owns an apartment.
  struct Apartment {
    Apartment();
    ~Apartment();
    Apartment(const Apartment &) = delete;
    Apartment &operator=(const Apartment &) = delete;
    bool owned = false;
  } apartment_;
  msimeui::DeviceResources device_;
  CandidatePalette palette_;
  TrayMenuMetrics metrics_;
  std::vector<TrayMenuItem> items_;
  TrayMenuGeometry geometry_{};
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  HWND window_ = nullptr;
  size_t hovered_ = static_cast<size_t>(-1);
  unsigned dpi_ = 96;
  bool failed_ = false;
};
} // namespace msime::windows

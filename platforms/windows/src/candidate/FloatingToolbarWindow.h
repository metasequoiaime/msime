#pragma once
#include "CandidatePalette.h"
#include "ModeMailbox.h"
#include "FloatingToolbarSettings.h"
#include "ToolbarIcons.h"
#include <functional>
#include <array>
#include <optional>
// windows.h first: its DrawText macro has to reach the Direct2D declarations.
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
// Null toggles the stored value for the toolbar. A value is the Server
// session's explicit target, so a delayed preference write cannot invert a
// newer state that was already persisted by another surface.
struct CharacterSetClick {
  std::optional<bool> desired;
};
using CharacterSetClickWorker = SingleClickWorker<CharacterSetClick>;
class FloatingToolbarWindow final {
public:
  using Reader = std::function<std::optional<ModePresentation>()>;
  using Click = std::function<void(const ModeClick &)>;
  using Action = std::function<void()>;
  using PositionChanged = std::function<void(POINT)>;
  FloatingToolbarWindow(Reader reader, Click click);
  ~FloatingToolbarWindow();
  // Share the candidate card's resolved tokens so one theme covers the surface.
  void set_palette(CandidatePalette palette);
  // Caps Lock and the input mode change what the language button shows.
  void set_language_state(ToolbarLanguageState state) {
    if (state.caps_lock == language_.caps_lock &&
        state.mode == language_.mode &&
        state.dedicated_english == language_.dedicated_english)
      return;
    language_ = state;
    if (window_)
      InvalidateRect(window_, nullptr, FALSE);
  }
  void set_scale(double scale) { scale_ = scale; }
  void set_font_size(int size) { font_size_ = size; }
  void set_items(std::array<bool, 6> items) { items_ = items; }
  // UI-thread only; refresh must remeasure even when input mode is unchanged.
  void set_settings(const FloatingToolbarSettings &settings) {
    if (!settings.valid()) return;
    const double scale = static_cast<double>(settings.scale_percent) / 100.0;
    if (scale_ == scale && font_size_ == static_cast<int>(settings.font_size) &&
        items_ == settings.items && language_button_ == settings.language) return;
    scale_ = scale;
    font_size_ = static_cast<int>(settings.font_size);
    items_ = settings.items;
    language_button_ = settings.language;
    hovered_.reset();
    pressed_.reset();
    shown_.reset();
    shown_character_set_.reset();
  }
  void set_position(std::optional<POINT> position) { dragged_position_ = position; }
  void set_position_changed(PositionChanged callback) { position_changed_ = std::move(callback); }
  // Tells a busy read apart from a client that is gone; see refresh().
  void set_active_reader(std::function<bool()> reader) {
    active_reader_ = std::move(reader);
  }
  void set_character_set_reader(std::function<std::optional<bool>()> reader) {
    character_set_reader_ = std::move(reader);
  }
  // Whether the shared desktop shell is installed beside the Server. Without
  // it the buttons that ask the shell to open a surface cannot do anything,
  // and the tray already shows its matching rows disabled rather than taking
  // a click and silently dropping it.
  void set_shell_available(bool available) {
    if (shell_available_ == available)
      return;
    shell_available_ = available;
    if (window_)
      InvalidateRect(window_, nullptr, FALSE);
  }
  void set_settings_action(Action action) { settings_action_ = std::move(action); }
  void set_character_set_action(Action action) { character_set_action_ = std::move(action); }
  void set_emoji_action(Action action) { emoji_action_ = std::move(action); }
  void set_keyboard_action(Action action) { keyboard_action_ = std::move(action); }
  void set_hide_action(Action action) { hide_action_ = std::move(action); }
  FloatingToolbarWindow(const FloatingToolbarWindow &) = delete;
  FloatingToolbarWindow &operator=(const FloatingToolbarWindow &) = delete;
  void refresh(bool enabled);
  void hide();
  bool failed() const { return failed_; }
  HWND handle() const { return window_; }

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void paint();
  // The product mark, at `pixels` square, or nothing when the executable has
  // no icon resource - which is every unit test that links this library.
  ID2D1Bitmap *logo_bitmap(int pixels);
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
  Reader reader_;
  Click click_;
  Action settings_action_;
  Action character_set_action_;
  Action emoji_action_;
  Action keyboard_action_;
  Action hide_action_;
  PositionChanged position_changed_;
  HWND window_ = nullptr;
  // The icon the mark is drawn from, and the size it was loaded at. Reloaded
  // when the DPI or the user's scale changes, so the mark is never resampled
  // from a frame of the wrong size.
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  std::optional<ModePresentation> shown_;
  std::optional<bool> shown_character_set_;
  std::optional<POINT> dragged_position_;
  // True between WM_ENTERSIZEMOVE and WM_EXITSIZEMOVE, so a programmatic
  // placement is not mistaken for one the user made.
  bool moving_ = false;
  bool user_dragging_ = false;
  bool shell_available_ = true;
  std::function<bool()> active_reader_;
  std::function<std::optional<bool>()> character_set_reader_;
  bool failed_ = false;
  // Pointer feedback. Without these the buttons gave no sign of being buttons.
  std::optional<size_t> hovered_;
  std::optional<size_t> pressed_;
  std::optional<FocusLease> pressed_lease_;
  bool tracking_mouse_ = false;
  // Caps Lock and the input mode, which the language button reflects.
  ToolbarLanguageState language_;
  // True once the toolbar has been positioned. The default corner is only for
  // the first placement; afterwards the user's own position is preserved.
  bool placed_ = false;
  double scale_ = 1.0;
  int font_size_ = 24;
  std::array<bool, 6> items_{true, true, true, true, false, true};
  // Whether the 中/英 button is drawn; the shared `english_mode` toolbar item.
  bool language_button_ = true;
};
} // namespace msime::windows

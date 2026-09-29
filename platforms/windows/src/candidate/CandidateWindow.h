#pragma once
#include "CandidateCardSize.h"
#include "CandidateClickWorker.h"
#include "CandidateFontSettings.h"
#include "CandidateLayoutSettings.h"
#include "CandidateMenuLayout.h"
#include "CandidatePalette.h"
#include "CandidatePresentation.h"
#include "CandidateShadow.h"
#include "CandidateSkin.h"
#include <functional>
#include <memory>
// windows.h first: its DrawText macro has to reach the Direct2D declarations,
// which is how the rest of this UI stack spells DrawTextW.
#include <windows.h>
#include <msimeui/DeviceResources.h>
// IDWriteFontFallback and IDWriteTextFormat1 live here, not in dwrite.h.
#include <dwrite_2.h>

namespace msime::windows {
// Forward declared: the flyout pulls in its own window headers, and only the
// implementation needs them.
class CandidateFlyoutWindow;
// Main/UI thread owns construction, polling, painting and destruction. Reader
// outlives the window and returns a freshly validated value, never Engine
// state.
// One owner-drawn menu row's label. Owner drawing keeps the platform's own
// keyboard handling and dismissal while letting the skin paint the row.
class CandidateWindow final {
public:
  using Reader = std::function<std::optional<CandidatePresentation>()>;
  using Click = std::function<void(const CandidateClick &)>;
  using Page = std::function<void(const CandidatePage &)>;
  using Rendered = std::function<void(const CandidatePresentation &)>;
  explicit CandidateWindow(Reader reader, Click click = {}, unsigned font_size = 16,
                           unsigned preedit_font_size = 16,
                           std::string font_family = "Segoe UI",
                           std::vector<std::string> fallback_fonts = {},
                           std::optional<bool> dark_theme = std::nullopt,
                           bool horizontal = false, bool show_preedit = true,
                           Page page = {}, Rendered rendered = {},
                           bool mouse_wheel = false);
  ~CandidateWindow();
  CandidateWindow(const CandidateWindow &) = delete;
  CandidateWindow &operator=(const CandidateWindow &) = delete;
  void refresh();
  // UI thread only. Invalid updates leave the previous display intact.
  bool set_fonts(const CandidateFontSettings &settings);
  void set_layout(CandidateLayoutSettings settings);
  // Adopt resolved skin tokens. The next refresh repaints with them; the
  // built-in theme stays in place until a package is actually resolved.
  void set_palette(CandidatePalette palette);
  void set_theme_palette(CandidatePalette palette);
  void invalidate_skin_images();
  // Minimum card width asked for by the active skin package, in DIPs.
  void set_skin_min_width(double value) { skin_min_width_ = value; }
  // 候选窗口跟随光标. With this off the card keeps the position it first
  // appeared at until it disappears, rather than tracking the caret through a
  // word. A change takes effect at the next appearance, since the pinned
  // anchor is only forgotten when the card hides.
  void set_follow_cursor(bool enabled) { follow_cursor_ = enabled; }
  // The mascot a package draws above the card. Empty image means none.
  void set_skin_decoration(const CandidateSkinDecoration &decoration) {
    decoration_image_ = decoration.image;
    decoration_top_ = decoration.top_dip;
    decoration_width_ = decoration.width_dip;
    decoration_align_ = decoration.align;
  }
  // The image a package draws over the card surface. Empty image means none.
  void set_skin_background(CandidateSkinBackground background) {
    background_ = std::move(background);
  }
  // The card radius a package asks for; none keeps the theme's.
  void set_skin_corner_radius(std::optional<float> radius) { skin_radius_ = radius; }
  void hide();
  bool failed() const { return failed_; }
  HWND handle() const { return window_; }

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void reposition();
  void invalidate_geometry();
  CandidateBounds card_bounds(const CandidatePresentation &value,
                              const RECT &work, unsigned dpi);
  // Each candidate's runs measured the way paint() draws them, and their wrapped heights. card_bounds and paint share both, so the card is sized for exactly the rows that get drawn.
  std::vector<CandidateItemWidths> measure_items(const CandidatePresentation &value);
  CandidateWrapMeasure wrap_measure(const CandidatePresentation &value);
  void paint();
  std::optional<CandidateClick> hit(int x, int y);
  // The pager arrow under a client point: true for the previous page, false for the next. None over anything else, over the previous arrow on the first page, or without a page callback.
  std::optional<bool> pager_hit(int x, int y);
  void show_context_menu(const CandidateClick &click, POINT client_point);
  Reader reader_;
  Click click_;
  Page page_;
  Rendered rendered_;
  HWND window_ = nullptr;
  std::optional<CandidatePresentation> shown_;
  unsigned shown_dpi_ = 0;
  std::optional<CandidatePresentation> painted_;
  // Rows as last drawn, set together with painted_. Hit testing reads these rather than recomputing, because row heights depend on text the click path should not measure again.
  std::vector<CandidateRowLayout> painted_rows_;
  // The pager as last drawn, in card coordinates, set together with painted_. None when the preedit row drew no pager.
  std::optional<CandidatePagerLayout> painted_pager_;
  std::optional<CandidateClick> pressed_;
  // The pager arrow a press started on; the page turns when the release lands on the same arrow.
  std::optional<bool> pressed_page_;
  std::optional<size_t> hovered_;
  unsigned painted_dpi_ = 0;
  bool failed_ = false;
  unsigned font_size_ = 16;
  unsigned preedit_font_size_ = 16;
  // Direct2D's imaging factory is a COM server, and this thread is the Server's
  // own UI thread, which otherwise never enters an apartment.
  struct Apartment {
    Apartment();
    ~Apartment();
    Apartment(const Apartment &) = delete;
    Apartment &operator=(const Apartment &) = delete;
    bool owned = false;
  } apartment_;
  // Direct2D through the shared UI stack; no second renderer in this tree.
  msimeui::DeviceResources device_;
  CandidatePalette palette_;
  std::wstring font_family_;
  std::optional<CandidateFontSettings> font_settings_;
  std::optional<bool> dark_theme_;
  bool horizontal_ = false;
  bool show_preedit_ = true;
  bool wubi_code_hint_ = true;
  // Configured supplementary faces, in order, for the per-glyph fallback chain.
  // Minimum card width asked for by the active skin package, in DIPs.
  double skin_min_width_ = 0.0;
  bool follow_cursor_ = true;
  // Where this appearance first anchored; cleared in hide().
  std::optional<POINT> anchor_;
  // Decoration artwork: absolute path, how far it rises above the card, and
  // its drawn width. The height follows the image's own aspect ratio.
  std::wstring decoration_image_;
  double decoration_top_ = 0.0;
  double decoration_width_ = 0.0;
  CandidateSkinAlign decoration_align_ = CandidateSkinAlign::right;
  CandidateSkinBackground background_;
  std::optional<float> skin_radius_;
  // Pixels reserved above the card for the artwork, computed when the card is
  // sized and reused when it is painted so the two cannot disagree.
  float decoration_offset_ = 0.0f;
  CandidateShadowInsets shadow_insets_{};
  // Owner-drawn menu labels, kept alive for the duration of the popup: the draw messages carry pointers into this list.
  // Built on first use: most sessions never open the right-click menu, and the flyout owns two windows and two Direct2D devices.
  std::unique_ptr<CandidateFlyoutWindow> flyout_;
  // The candidate the open flyout acts on, recorded afresh on every right click because the flyout itself outlives any one opening.
  CandidateMenuTarget<CandidateClick> menu_target_;
  std::vector<std::wstring> fallback_families_;
  Microsoft::WRL::ComPtr<IDWriteFontFallback> font_fallback_;
  // Tallest this vertical list has been since the last hide(), in physical
  // pixels. Only the flip decision reads it; placement uses the real height.
  int64_t tallest_ = 0;
  // Wheel paging is opt-in. Off, the wheel goes back to DefWindowProc rather
  // than being swallowed by this NOACTIVATE window.
  bool mouse_wheel_ = false;
  int wheel_accumulator_ = 0;
};
} // namespace msime::windows

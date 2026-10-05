#include "FloatingToolbarWindow.h"
#include "FloatingToolbarPlacement.h"
#include "ToolbarIcons.h"
#include "ToolbarLayout.h"
#include "ToolbarCoordinates.h"
#include "ToolbarClick.h"
#include "ToolbarModeCommand.h"
#include "ServerResources.h"
#include "WindowShadow.h"
#include "IconFont.h"
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <windowsx.h>
#include <vector>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t kClassName[] = L"MSIME.Client.Preview.FloatingToolbar" MSIME_EDITION_NAME_SUFFIX;
constexpr int kWidth = 732;
constexpr int kHeight = 52;
int dpi_scale(HWND window, int value) {
  const UINT dpi = GetDpiForWindow(window);
  return MulDiv(value, static_cast<int>(dpi ? dpi : USER_DEFAULT_SCREEN_DPI), USER_DEFAULT_SCREEN_DPI);
}
std::optional<POINT> clamp_position(POINT position, int width, int height) {
  const HMONITOR monitor = MonitorFromPoint(position, MONITOR_DEFAULTTONEAREST);
  MONITORINFO info{};
  info.cbSize = sizeof(info);
  if (!GetMonitorInfoW(monitor, &info)) return std::nullopt;
  const LONG right = std::max(info.rcWork.left,
                              info.rcWork.right - static_cast<LONG>(width));
  const LONG bottom = std::max(info.rcWork.top,
                               info.rcWork.bottom - static_cast<LONG>(height));
  return POINT{std::clamp<LONG>(position.x, info.rcWork.left, right),
               std::clamp<LONG>(position.y, info.rcWork.top, bottom)};
}
bool same(const FocusLease &a, const FocusLease &b) {
  return a.epoch == b.epoch && a.token == b.token &&
         same_ticket(a.transport, b.transport);
}
std::vector<int> slots(const std::array<bool, 6> &items, bool language) {
  return floating_toolbar_slots(items, language);
}
// Buttons that do nothing on their own: they ask the shared desktop shell to
// open a surface. The rest - the three mode toggles, 简繁, voice and hide - are
// handled inside this process and stay usable without a shell.
bool needs_shell(int button) {
  switch (button) {
  case kToolbarEmoji:
  case kToolbarScreenKeyboard:
  case kToolbarSettings:
    return true;
  default:
    return false;
  }
}
} // namespace

FloatingToolbarWindow::FloatingToolbarWindow(Reader reader, Click click)
    : reader_(std::move(reader)), click_(std::move(click)) {
  if (!reader_ || !click_) throw std::invalid_argument("Missing toolbar callback");
  WNDCLASSEXW type{};
  type.cbSize = sizeof(type);
  type.lpfnWndProc = procedure;
  type.hInstance = GetModuleHandleW(nullptr);
  type.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
  type.lpszClassName = kClassName;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Toolbar class unavailable");
  window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                            kClassName, L"MSIME toolbar", WS_POPUP, 0, 0,
                            kWidth, kHeight, nullptr, nullptr, type.hInstance,
                            this);
  if (!window_) throw std::runtime_error("Toolbar window unavailable");
}
FloatingToolbarWindow::~FloatingToolbarWindow() {
  hide();
  if (logo_) DestroyIcon(logo_);
  if (window_) DestroyWindow(window_);
}
ID2D1Bitmap *FloatingToolbarWindow::logo_bitmap(int pixels) {
  if (pixels <= 0) return nullptr;
  if (!logo_ || logo_pixels_ != pixels) {
    // LR_SHARED would hand back a cached system copy at the standard size and
    // ignore the one asked for, which is exactly the resampling to avoid.
    const HANDLE loaded =
        LoadImageW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_MSIME_LOGO),
                   IMAGE_ICON, pixels, pixels, LR_DEFAULTCOLOR);
    if (!loaded) return nullptr;
    if (logo_) DestroyIcon(logo_);
    logo_ = static_cast<HICON>(loaded);
    logo_pixels_ = pixels;
  }
  return device_.GetBitmapFromIcon(logo_, L"icon:toolbar-logo:" + std::to_wstring(pixels));
}
void FloatingToolbarWindow::hide() {
  // A hidden toolbar has no pointer over it; leaving these set would show a
  // stale highlight the next time it appears.
  hovered_.reset();
  pressed_.reset();
  shown_.reset();
  shown_character_set_.reset();
  if (window_) ShowWindow(window_, SW_HIDE);
}
void FloatingToolbarWindow::refresh(bool enabled) {
  if (failed_) return;
  try {
    if (!enabled) { hide(); return; }
    const auto value = reader_();
    // ModeMailbox::snapshot() try-locks the mailbox and the focus gate, so it
    // returns nothing whenever the input queue happens to hold either - which
    // is most of the time while the user is typing. That empty read means
    // "busy", not "no longer active", and hiding on it blinked the toolbar on
    // almost every key: measured 17 hide/show pairs for one short sentence.
    // Keep the last state and wait for a read that succeeds - but only while
    // there is still something to keep it for, or a client that went away
    // would leave the toolbar on screen for good.
    if (!value) {
      // active() waits for the mailbox lock instead of try-ing it, so it
      // answers what the snapshot cannot: whether a focused client is on file.
      // Cleared by disconnect and shutdown, so it goes false exactly when the
      // toolbar has nothing left to describe.
      const bool active = active_reader_ && active_reader_();
      if (!active || !shown_) hide();
      return;
    }
    const auto character_set = character_set_reader_ ? character_set_reader_()
                                                     : std::nullopt;
    const bool changed = !shown_ || !same(shown_->lease, value->lease) ||
                         shown_->chinese != value->chinese ||
                         shown_->chinese_punctuation != value->chinese_punctuation ||
                         shown_->fullwidth != value->fullwidth ||
                         shown_character_set_ != character_set;
    if (!changed && IsWindowVisible(window_)) return;
    shown_ = value;
    shown_character_set_ = character_set;
    RECT work{};
    // With no position of its own the toolbar follows the focused window's
    // monitor. Once it has one - dragged or restored from the config - it stays
    // on whichever screen that position is on, because clamping it against the
    // foreground window's monitor would drag it back across the desktop.
    const HMONITOR monitor =
        dragged_position_
            ? MonitorFromPoint(*dragged_position_, MONITOR_DEFAULTTONEAREST)
        : placed_ ? MonitorFromWindow(window_, MONITOR_DEFAULTTONEAREST)
                  : MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTOPRIMARY);
    MONITORINFO info{};
    info.cbSize = sizeof(info);
    if (!GetMonitorInfoW(monitor, &info)) throw std::runtime_error("Toolbar monitor unavailable");
    work = info.rcWork;
    const auto metrics = toolbar_metrics(static_cast<double>(font_size_));
    const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    const int width = static_cast<int>(std::ceil(
        toolbar_window_width(slots(items_, language_button_).size(), metrics) * unit));
    const int height = static_cast<int>(std::ceil(toolbar_window_height(metrics) * unit));
    const int margin = dpi_scale(window_, 20);
    FloatingToolbarPlacementInput placement;
    placement.width = width;
    placement.height = height;
    placement.margin = margin;
    placement.work_left = work.left;
    placement.work_top = work.top;
    placement.work_right = work.right;
    placement.work_bottom = work.bottom;
    // The remembered position, not the live window rect, is what survives a
    // restart: a drag records it and the config restores it through
    // set_position before the first refresh. It stays empty until the user
    // actually moves the toolbar, so an untouched one keeps following the
    // focused window and re-anchoring to the corner.
    placement.placed = dragged_position_.has_value();
    if (dragged_position_) {
      placement.current_x = dragged_position_->x;
      placement.current_y = dragged_position_->y;
    }
    const auto placed = floating_toolbar_placement(placement);
    if (!SetWindowPos(window_, HWND_TOPMOST, placed.x, placed.y, width, height,
                      SWP_NOACTIVATE | SWP_SHOWWINDOW))
      throw std::runtime_error("Toolbar positioning failed");
    // Only after the move succeeded, so a failed first placement retries the
    // corner rather than preserving a position the window never took.
    placed_ = true;
    InvalidateRect(window_, nullptr, FALSE);
  } catch (...) { failed_ = true; hide(); }
}
FloatingToolbarWindow::Apartment::Apartment() {
  const HRESULT entered =
      CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
  if (FAILED(entered) && entered != RPC_E_CHANGED_MODE)
    throw std::runtime_error("Toolbar apartment unavailable");
  owned = entered != RPC_E_CHANGED_MODE;
}
FloatingToolbarWindow::Apartment::~Apartment() {
  if (owned)
    CoUninitialize();
}
void FloatingToolbarWindow::set_palette(CandidatePalette palette) {
  palette_ = std::move(palette);
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}
void FloatingToolbarWindow::paint() {
  PAINTSTRUCT state{};
  const HDC dc = BeginPaint(window_, &state);
  if (!dc)
    return;
  struct End {
    HWND window;
    PAINTSTRUCT &state;
    ~End() { EndPaint(window, &state); }
  } end{window_, state};
  // Composition rather than an hwnd target: the shadow falls outside the bar,
  // so the window has to carry per-pixel alpha where it is nothing but shadow.
  if (!device_.EnsureForComposition(window_))
    throw std::runtime_error("Toolbar device unavailable");
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Toolbar render target unavailable");
  auto brush = [&](const CandidateColor &color) {
    auto *created = device_.GetSolidColorBrush(
        D2D1::ColorF(color.r, color.g, color.b, color.a));
    if (!created)
      throw std::runtime_error("Toolbar brush unavailable");
    return created;
  };
  const float unit = static_cast<float>(scale_);
  auto *format = device_.GetTextFormat(
      L"Segoe UI", static_cast<float>(font_size_) * unit, DWRITE_FONT_WEIGHT_NORMAL,
      DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
      DWRITE_WORD_WRAPPING_NO_WRAP);
  if (!format)
    throw std::runtime_error("Toolbar text format unavailable");
  const auto bar = toolbar_metrics(static_cast<double>(font_size_));
  const auto card = toolbar_card(slots(items_, language_button_).size(), bar);
  const D2D1_RECT_F card_rect{
      static_cast<float>(card.left) * unit, static_cast<float>(card.top) * unit,
      static_cast<float>(card.right) * unit,
      static_cast<float>(card.bottom) * unit};
  target->BeginDraw();
  // Transparent, not the surface colour: everything outside the bar is either
  // shadow or the desktop showing through.
  target->Clear(D2D1::ColorF(0, 0.0f));
  draw_window_shadow(target, card_rect, palette_.radius * unit,
                     static_cast<float>(bar.shadow.scale));
  const float inset = palette_.border_width * unit / 2.0f;
  const D2D1_ROUNDED_RECT body{{card_rect.left + inset, card_rect.top + inset,
                                card_rect.right - inset,
                                card_rect.bottom - inset},
                               palette_.radius * unit, palette_.radius * unit};
  target->FillRoundedRectangle(body, brush(palette_.surface));
  target->DrawRoundedRectangle(body, brush(palette_.border),
                               palette_.border_width * unit);
  const auto value = reader_();
  if (value && shown_ && same(value->lease, shown_->lease)) {
    auto *factory = device_.GetDWriteFactory();
    const wchar_t *icon_family = icon_font_family(factory);
    // Each button's two-way mode, in slot order. An absent state means the
    // Server has not reported it, and the icon shows a question mark rather
    // than asserting a mode the user is not actually in.
    const std::optional<bool> states[] = {
        value->chinese,
        value->fullwidth,
        value->chinese_punctuation,
        shown_character_set_,
        std::nullopt, std::nullopt, std::nullopt,
        std::nullopt, std::nullopt, std::nullopt, std::nullopt};
    const auto active = slots(items_, language_button_);
    const auto layout = toolbar_metrics(static_cast<double>(font_size_));
    // The product mark at the far left. Drawn before the drag strip and the
    // buttons, and skipped rather than substituted if the icon will not load -
    // a missing mark costs nothing, a placeholder box would look like a bug.
    const auto mark = toolbar_logo(layout);
    // Loaded at the size it is drawn at, in real pixels rather than Direct2D's
    // DIPs: msime.ico carries frames from 16 to 256, and asking for the right
    // one is the difference between a crisp mark and a resampled one.
    const double pixels = (mark.right - mark.left) *
                          toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    if (auto *logo = logo_bitmap(static_cast<int>(std::lround(pixels))))
      target->DrawBitmap(logo,
                         D2D1_RECT_F{static_cast<float>(mark.left) * unit,
                                     static_cast<float>(mark.top) * unit,
                                     static_cast<float>(mark.right) * unit,
                                     static_cast<float>(mark.bottom) * unit},
                         1.0f, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    // The drag strip and the divider that separates it from the buttons. The
    // strip is the only part that drags, so it has to be visible; upstream
    // draws it in the accent colour. Both follow the bar height so they stay
    // centred when the icon size changes.
    const auto height = static_cast<float>(layout.height);
    const float top = card_rect.top;
    const float strip = card_rect.left + static_cast<float>(layout.logo) * unit;
    const float handle_left = strip + 3.0f * unit;
    const D2D1_ROUNDED_RECT handle{{handle_left, top + height * 0.269f * unit,
                                    handle_left + 2.0f * unit,
                                    top + height * 0.731f * unit},
                                   1.0f * unit, 1.0f * unit};
    target->FillRoundedRectangle(handle, brush(palette_.accent));
    const float divider =
        strip + static_cast<float>(layout.handle - 1.0) * unit;
    target->DrawLine({divider, top + height * 0.231f * unit},
                     {divider, top + height * 0.769f * unit},
                     brush(palette_.divider.value_or(palette_.border)), 1.0f * unit);
    for (size_t i = 0; i < active.size(); ++i) {
      const int button = active[i];
      const auto box = toolbar_cell(i, layout);
      const D2D1_RECT_F cell{static_cast<float>(box.left) * unit,
                             static_cast<float>(box.top) * unit,
                             static_cast<float>(box.right) * unit,
                             static_cast<float>(box.bottom) * unit};
      // Hover and press fills, so a button looks like one. Pressed is drawn like the card's selected row, the selected fill with the selected text colour on it, rather than a darker hover. A button the shell would have to answer is drawn disabled: no hover fill and the secondary text colour, exactly as the tray draws a row whose capability is missing.
      const bool usable = shell_available_ || !needs_shell(button);
      const bool down = usable && hovered_ == i && pressed_ == i;
      const auto &glyph_color = !usable ? palette_.number
                                : down  ? palette_.selected_text
                                        : palette_.text;
      if (hovered_ == i && usable) {
        const D2D1_ROUNDED_RECT fill{{cell.left + 2.0f * unit, cell.top,
                                      cell.right - 2.0f * unit, cell.bottom},
                                     palette_.item_radius * unit,
                                     palette_.item_radius * unit};
        target->FillRoundedRectangle(
            fill, brush(down ? palette_.selected : palette_.hover));
      }
      const auto icon = toolbar_icon(button, states[button], language_);
      // Draw the glyph only when the installed icon font really has it;
      // otherwise the text fallback, which is always readable.
      const bool glyph = icon.codepoint && icon_family &&
                         icon_font_has(factory, icon_family, icon.codepoint);
      const wchar_t text[] = {icon.codepoint, L'\0'};
      auto *cell_format =
          glyph ? device_.GetTextFormat(
                      icon_family, static_cast<float>(font_size_) * unit,
                      DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER,
                      DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                      DWRITE_WORD_WRAPPING_NO_WRAP)
                : format;
      if (!cell_format)
        cell_format = format;
      const wchar_t *drawn_text = glyph ? text : icon.fallback;
      const auto length = static_cast<UINT32>(wcslen(drawn_text));
      if (!length)
        continue;
      target->DrawText(drawn_text, length, cell_format, cell,
                       brush(glyph_color));
      // Dedicated English underlines its "En". Upstream insets the line by a
      // twelfth of the cell and floors both the offset and the stroke, so it
      // stays a visible line rather than thinning away at small icon sizes.
      if (icon.underline && !glyph) {
        const float size = static_cast<float>(font_size_) * unit;
        const float side = (cell.right - cell.left) * 0.08f;
        const float y = cell.bottom - (std::max)(2.0f * unit, size * 0.12f);
        target->DrawLine({cell.left + side, y}, {cell.right - side, y},
                         brush(glyph_color),
                         (std::max)(1.0f * unit, size * 0.06f));
      }
    }
  }
  const HRESULT drawn = target->EndDraw();
  // A composition swap chain only reaches the screen once it is presented.
  if (SUCCEEDED(drawn) && FAILED(device_.Present()))
    throw std::runtime_error("Toolbar presentation failed");
  // Signed HRESULT against a macro that is unsigned in some SDK and MinGW
  // versions; see CandidateWindow.cpp for the same comparison.
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    device_.DiscardTarget();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Toolbar drawing failed");
}
LRESULT CALLBACK FloatingToolbarWindow::procedure(HWND window, UINT message,
                                                    WPARAM w, LPARAM l) noexcept {
  auto *self = reinterpret_cast<FloatingToolbarWindow *>(GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) { self = static_cast<FloatingToolbarWindow *>(reinterpret_cast<CREATESTRUCTW *>(l)->lpCreateParams);
    self->window_ = window; SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self)); }
  if (!self) return DefWindowProcW(window, message, w, l);
  try { switch (message) {
    case WM_MOUSEACTIVATE: return MA_NOACTIVATE;
    case WM_ACTIVATE:
      if (self->shown_) self->refresh(true);
      return 0;
    case WM_ERASEBKGND: return 1;
    case WM_POWERBROADCAST:
      if (w != PBT_APMRESUMEAUTOMATIC && w != PBT_APMRESUMECRITICAL &&
          w != PBT_APMRESUMESUSPEND)
        break;
      [[fallthrough]];
    case WM_DISPLAYCHANGE:
    case WM_SETTINGCHANGE:
    case WM_DWMCOMPOSITIONCHANGED:
    case WM_DPICHANGED: {
      const bool visible = IsWindowVisible(window) != FALSE;
      self->shown_.reset();
      self->shown_character_set_.reset();
      self->hovered_.reset();
      self->pressed_.reset();
      self->pressed_lease_.reset();
      // Cached composition surfaces retain their old DPI even when large
      // enough for the new window, and display/DWM/resume edges can invalidate
      // their underlying device. Recreate this window's target only.
      self->device_.DiscardTarget();
      if (visible) self->refresh(true);
      return message == WM_POWERBROADCAST ? TRUE : 0;
    }
    // Deliberately not part of the block above: refresh() itself calls
    // SetWindowPos, which raises WM_SIZE synchronously, and discarding the
    // target from there would pull it out from under the refresh in progress.
    // The re-entrant refresh stops at its own unchanged-state early return.
    case WM_SIZE:
      if (self->shown_) self->refresh(true);
      return 0;
    case WM_PAINT: self->paint(); return 0;
    case WM_ENTERSIZEMOVE:
      self->moving_ = true;
      return 0;
    case WM_MOVE:
      // Only a move the user drove counts. refresh()'s own SetWindowPos raises
      // WM_MOVE too, and treating that as a drag would record the default
      // corner as a chosen position - after which the toolbar would stop
      // following the focused window's monitor and stop re-anchoring when its
      // width changes.
      if (!self->moving_)
        return 0;
      // The window rect, not lParam: WM_MOVE reports the client area's origin,
      // so persisting that shifted the toolbar up and left by the frame on
      // every restart.
      {
        RECT rect{};
        if (self->user_dragging_ && GetWindowRect(window, &rect))
          self->dragged_position_ = POINT{rect.left, rect.top};
      }
      return 0;
    case WM_EXITSIZEMOVE:
      // Once, when the drag ends. The move loop raises WM_MOVE for every frame
      // of the drag, and the listener rewrites the whole configuration file, so
      // persisting there rewrote it dozens of times per second on the UI thread.
      self->moving_ = false;
      if (self->dragged_position_) {
        // The move loop lets the window be dropped past the work area, and the
        // system does not pull it back. Clamping only the remembered position
        // would leave the visible toolbar hanging off the screen until the next
        // refresh, so the window follows the clamp.
        RECT rect{};
        if (GetWindowRect(window, &rect)) {
          const auto clamped = clamp_position(
              POINT{rect.left, rect.top}, rect.right - rect.left, rect.bottom - rect.top);
          if (clamped) {
            self->dragged_position_ = *clamped;
            if (self->dragged_position_->x != rect.left || self->dragged_position_->y != rect.top)
              SetWindowPos(window, nullptr, self->dragged_position_->x,
                           self->dragged_position_->y, 0, 0,
                           SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
          }
        }
        if (self->position_changed_)
          self->position_changed_(*self->dragged_position_);
      }
      return 0;
    case WM_LBUTTONDOWN: {
      self->pressed_.reset();
      self->pressed_lease_.reset();
      // Only the strip left of the first button drags. Treating the whole
      // window as a caption meant a press on a button entered the system move
      // loop, and the click below only ran for whatever button-up survived it.
      const auto drag = toolbar_metrics(static_cast<double>(self->font_size_));
      if (toolbar_drag_at_pixel(GET_X_LPARAM(l), GET_Y_LPARAM(l),
                                GetDpiForWindow(window), self->scale_, drag)) {
        ReleaseCapture();
        SendMessageW(window, WM_NCLBUTTONDOWN, HTCAPTION, 0);
        return 0;
      }
      // Pressing a button shows it pressed until the release is handled.
      self->pressed_ = toolbar_button_at_pixel(
          GET_X_LPARAM(l), GET_Y_LPARAM(l), GetDpiForWindow(window),
          self->scale_, slots(self->items_, self->language_button_).size(), drag);
      const auto value = self->reader_();
      if (self->pressed_ && value && self->shown_ &&
          same(value->lease, self->shown_->lease)) {
        self->pressed_lease_ = value->lease;
        TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
        self->tracking_mouse_ = TrackMouseEvent(&track) != FALSE;
        if (!self->tracking_mouse_) self->pressed_.reset();
        InvalidateRect(window, nullptr, FALSE);
      } else {
        self->pressed_.reset();
      }
      return 0;
    }
    case WM_SETCURSOR:
      // Show the move cursor over the drag strip only, so the buttons keep the
      // ordinary arrow and the strip advertises what it does.
      if (LOWORD(l) == HTCLIENT) {
        POINT cursor{};
        RECT bounds{};
        if (GetCursorPos(&cursor) && ScreenToClient(window, &cursor) &&
            GetClientRect(window, &bounds) &&
            toolbar_drag_at_pixel(
                cursor.x, cursor.y, GetDpiForWindow(window), self->scale_,
                toolbar_metrics(static_cast<double>(self->font_size_)))) {
          SetCursor(LoadCursorW(nullptr, IDC_SIZEALL));
          return TRUE;
        }
      }
      break;
    case WM_MOUSEMOVE: {
      // Hover feedback needs to know where the pointer is; without tracking,
      // the buttons gave no sign that they were buttons at all.
      const int x = GET_X_LPARAM(l);
      const auto active = slots(self->items_, self->language_button_);
      const auto layout = toolbar_metrics(static_cast<double>(self->font_size_));
      const auto hovered = toolbar_button_at_pixel(
          x, GET_Y_LPARAM(l), GetDpiForWindow(window), self->scale_, active.size(), layout);
      if (self->pressed_ && self->pressed_ != hovered) {
        self->pressed_.reset();
        InvalidateRect(window, nullptr, FALSE);
      }
      if (hovered != self->hovered_) {
        self->hovered_ = hovered;
        InvalidateRect(window, nullptr, FALSE);
      }
      // Ask for one leave message so the highlight is dropped when the pointer
      // goes elsewhere; without it the last hovered button stays lit.
      if (!self->tracking_mouse_) {
        TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
        self->tracking_mouse_ = TrackMouseEvent(&track) != FALSE;
      }
      return 0;
    }
    case WM_MOUSELEAVE:
      self->tracking_mouse_ = false;
      [[fallthrough]];
    case WM_CANCELMODE:
    case WM_CAPTURECHANGED:
      if (self->hovered_ || self->pressed_) {
        self->hovered_.reset();
        self->pressed_.reset();
        self->pressed_lease_.reset();
        InvalidateRect(window, nullptr, FALSE);
      }
      return 0;
    case WM_LBUTTONUP: {
      // Release always clears the pressed look, whether or not the release
      // lands on a button - otherwise a press that slid off stays lit.
      if (self->pressed_) {
        InvalidateRect(window, nullptr, FALSE);
      }
      const auto value = self->reader_();
      const int x = GET_X_LPARAM(l);
      const auto active = slots(self->items_, self->language_button_);
      const auto layout = toolbar_metrics(static_cast<double>(self->font_size_));
      const auto position_at =
          toolbar_button_at_pixel(x, GET_Y_LPARAM(l), GetDpiForWindow(window),
                                  self->scale_, active.size(), layout);
      const bool valid_click = toolbar_release(
          self->pressed_, position_at,
          value && self->pressed_lease_ && self->shown_ &&
              same(value->lease, *self->pressed_lease_) &&
              same(value->lease, self->shown_->lease));
      self->pressed_lease_.reset();
      if (valid_click) {
        const size_t position = *position_at;
        const int slot = active[position];
        if (!self->shell_available_ && needs_shell(slot)) {
          // Nothing to open, so the press is not an action. Reported the same
          // way the tray reports it: by doing nothing visible, not by looking
          // pressed and then dropping the command.
        }
        else if (slot <= kToolbarPunctuation) {
          if (auto command = toolbar_mode_command(
                  slot, value->chinese, value->fullwidth, value->chinese_punctuation))
            self->click_(ModeClick{value->lease, *command});
        }
        else if (slot == 3 && self->character_set_action_)
          self->character_set_action_();
        else if (slot == 4 && self->emoji_action_)
          self->emoji_action_();
        else if (slot == 5 && self->keyboard_action_)
          self->keyboard_action_();
        else if (slot == 6 && self->settings_action_)
          self->settings_action_();
        else if (slot == 10 && self->hide_action_)
          self->hide_action_();
      }
      return 0;
    }
    // Let DefWindowProc handle the caption message sent by the drag strip.
    // Sending that same synchronous message again here recurses indefinitely.
  }} catch (...) { self->failed_ = true; self->hide(); return 0; }
  return DefWindowProcW(window, message, w, l);
}
} // namespace msime::windows

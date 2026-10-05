#include "CandidateFlyoutWindow.h"

#include <stdexcept>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t class_name[] = L"MSIME.Client.Preview.CandidateFlyout" MSIME_EDITION_NAME_SUFFIX;
constexpr size_t no_row = static_cast<size_t>(-1);
// Affect only this UI operation; restore the caller's thread context even on
// failure.
struct DpiScope {
  DPI_AWARENESS_CONTEXT previous;
  DpiScope()
      : previous(SetThreadDpiAwarenessContext(
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)) {
    if (!previous)
      throw std::runtime_error("Per-monitor DPI unavailable");
  }
  ~DpiScope() { SetThreadDpiAwarenessContext(previous); }
};
struct Painting {
  HWND window;
  PAINTSTRUCT state{};
  HDC dc;
  explicit Painting(HWND value)
      : window(value), dc(BeginPaint(window, &state)) {}
  ~Painting() { EndPaint(window, &state); }
};
std::wstring wide(const std::string &text) {
  if (text.empty() || text.size() > 256)
    throw std::invalid_argument("Invalid candidate menu label");
  const int count =
      MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), nullptr, 0);
  if (!count)
    throw std::invalid_argument("Invalid candidate menu label");
  std::wstring result(static_cast<size_t>(count), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), result.data(),
                          count) != count)
    throw std::invalid_argument("Invalid candidate menu label");
  return result;
}
} // namespace

CandidateFlyoutWindow::CandidateFlyoutWindow(Chosen chosen)
    : chosen_(std::move(chosen)) {
  if (!chosen_)
    throw std::invalid_argument("Missing candidate menu callback");
  DpiScope dpi_scope;
  WNDCLASSEXW descriptor{};
  descriptor.cbSize = sizeof(descriptor);
  descriptor.lpfnWndProc = procedure;
  descriptor.hInstance = GetModuleHandleW(nullptr);
  descriptor.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
  descriptor.lpszClassName = class_name;
  // This process owns the class; no per-window unregister/re-register race.
  if (!RegisterClassExW(&descriptor) &&
      GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Candidate menu class unavailable");
  create(menu_);
  create(submenu_);
}

void CandidateFlyoutWindow::create(Panel &panel) {
  // No redirection bitmap: the card is composed with per-pixel alpha, which is
  // what gives it rounded corners instead of a rectangular window cut-out.
  panel.window = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW |
                                     WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP,
                                 class_name, L"", WS_POPUP, 0, 0, 1, 1, nullptr,
                                 nullptr, GetModuleHandleW(nullptr), this);
  if (!panel.window)
    throw std::runtime_error("Candidate menu window unavailable");
}

CandidateFlyoutWindow::~CandidateFlyoutWindow() {
  hide();
  if (submenu_.window)
    DestroyWindow(submenu_.window);
  if (menu_.window)
    DestroyWindow(menu_.window);
}

void CandidateFlyoutWindow::set_palette(CandidatePalette palette) {
  palette_ = std::move(palette);
  for (Panel *panel : {&menu_, &submenu_})
    if (panel->window)
      InvalidateRect(panel->window, nullptr, FALSE);
}

bool CandidateFlyoutWindow::visible() const noexcept {
  return menu_.window && IsWindowVisible(menu_.window);
}

CandidateFlyoutWindow::Panel *
CandidateFlyoutWindow::panel_for(HWND window) noexcept {
  if (window == menu_.window)
    return &menu_;
  if (window == submenu_.window)
    return &submenu_;
  return nullptr;
}

void CandidateFlyoutWindow::hide() noexcept {
  close_submenu();
  // Release the capture before hiding: a hidden window holding it would
  // swallow the application's mouse input.
  if (capturing_) {
    capturing_ = false;
    if (GetCapture() == menu_.window)
      ReleaseCapture();
  }
  menu_.hovered = no_row;
  menu_.items.clear();
  if (menu_.window)
    ShowWindow(menu_.window, SW_HIDE);
}

void CandidateFlyoutWindow::close_submenu() noexcept {
  submenu_.hovered = no_row;
  submenu_.items.clear();
  if (submenu_.window)
    ShowWindow(submenu_.window, SW_HIDE);
}

bool CandidateFlyoutWindow::open(int pointer_x, int pointer_y,
                                 size_t code_points, bool actions_available,
                                 int fixed_position) noexcept {
  try {
    if (failed_)
      return false;
    DpiScope dpi_scope;
    hide();
    MONITORINFO monitor{};
    monitor.cbSize = sizeof(monitor);
    if (!GetMonitorInfoW(MonitorFromPoint({pointer_x, pointer_y},
                                          MONITOR_DEFAULTTONEAREST),
                         &monitor))
      throw std::runtime_error("Candidate menu monitor unavailable");
    actions_available_ = actions_available;
    fixed_position_ = fixed_position;
    auto items = candidate_menu_items(code_points, actions_available_);
    menu_.dpi = GetDpiForWindow(menu_.window);
    const auto &work = monitor.rcWork;
    const auto bounds = candidate_menu_bounds(
        pointer_x, pointer_y, work.left, work.top, work.right, work.bottom,
        menu_.dpi, candidate_menu_size(items, metrics_));
    show(menu_, bounds, std::move(items));
    // The flyout never takes focus, so it has no focus to lose. The capture is
    // what lets it see the click outside that should dismiss it.
    SetCapture(menu_.window);
    capturing_ = true;
    return visible();
  } catch (...) {
    // Appearance must not be fatal: report no menu this time.
    hide();
    return false;
  }
}

void CandidateFlyoutWindow::show(Panel &panel,
                                 const CandidateMenuBounds &bounds,
                                 std::vector<CandidateMenuItem> items) {
  panel.items = std::move(items);
  panel.hovered = no_row;
  if (!SetWindowPos(panel.window, HWND_TOPMOST, bounds.x, bounds.y,
                    bounds.width, bounds.height,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW))
    throw std::runtime_error("Candidate menu positioning failed");
  InvalidateRect(panel.window, nullptr, FALSE);
}

std::optional<size_t> CandidateFlyoutWindow::hit(const Panel &panel, int x,
                                                 int y) const {
  if (panel.items.empty() || !panel.dpi)
    return std::nullopt;
  const double scale = static_cast<double>(panel.dpi) / 96.0;
  return candidate_menu_hit(x / scale, y / scale, panel.items, metrics_);
}

bool CandidateFlyoutWindow::contains(POINT screen) const noexcept {
  for (const Panel *panel : {&menu_, &submenu_}) {
    RECT bounds{};
    if (!panel->window || !IsWindowVisible(panel->window) ||
        !GetWindowRect(panel->window, &bounds))
      continue;
    if (PtInRect(&bounds, screen))
      return true;
  }
  return false;
}

void CandidateFlyoutWindow::open_submenu() {
  DpiScope dpi_scope;
  RECT parent{};
  if (!GetWindowRect(menu_.window, &parent))
    return;
  MONITORINFO monitor{};
  monitor.cbSize = sizeof(monitor);
  if (!GetMonitorInfoW(MonitorFromWindow(menu_.window, MONITOR_DEFAULTTONEAREST),
                       &monitor))
    return;
  auto items = candidate_menu_submenu_items(actions_available_, fixed_position_);
  submenu_.dpi = menu_.dpi;
  const double scale = static_cast<double>(menu_.dpi) / 96.0;
  const auto row = candidate_menu_row(menu_.hovered, menu_.items, metrics_);
  const auto &work = monitor.rcWork;
  const auto bounds = candidate_submenu_bounds(
      parent.left, parent.right,
      parent.top + static_cast<int>(row.top * scale), work.left, work.top,
      work.right, work.bottom, submenu_.dpi,
      candidate_menu_size(items, metrics_));
  show(submenu_, bounds, std::move(items));
}

void CandidateFlyoutWindow::track(Panel &panel, int x, int y) {
  const auto row = hit(panel, x, y);
  const size_t hovered = row ? *row : no_row;
  if (hovered == panel.hovered)
    return;
  panel.hovered = hovered;
  InvalidateRect(panel.window, nullptr, FALSE);
  if (&panel != &menu_)
    return;
  // Only the 固定排位 row keeps a submenu open. Moving onto any other row
  // closes it, so the two lists never disagree about what is selected.
  if (hovered != no_row && menu_.items[hovered].submenu)
    open_submenu();
  else
    close_submenu();
}

void CandidateFlyoutWindow::choose(const Panel &panel, size_t index) {
  if (index >= panel.items.size() || panel.items[index].separator ||
      !panel.items[index].available)
    return;
  const auto &item = panel.items[index];
  // A submenu row is not a command; it only opens the list beside it.
  if (item.submenu)
    return;
  const CandidateMenuChoice choice{item.command, item.position};
  hide();
  chosen_(choice);
}

void CandidateFlyoutWindow::paint(Panel &panel) {
  DpiScope dpi_scope;
  Painting painting(panel.window);
  if (!painting.dc)
    throw std::runtime_error("Candidate menu painting unavailable");
  if (panel.items.empty()) {
    ShowWindow(panel.window, SW_HIDE);
    return;
  }
  if (!panel.device.EnsureForComposition(panel.window))
    throw std::runtime_error("Candidate menu device unavailable");
  auto *target = panel.device.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Candidate menu render target unavailable");
  auto brush = [&](const CandidateColor &color) {
    auto *created = panel.device.GetSolidColorBrush(
        D2D1::ColorF(color.r, color.g, color.b, color.a));
    if (!created)
      throw std::runtime_error("Candidate menu brush unavailable");
    return created;
  };
  auto *label_format = panel.device.GetTextFormat(
      L"Segoe UI", 14.0f, DWRITE_FONT_WEIGHT_NORMAL,
      DWRITE_TEXT_ALIGNMENT_LEADING, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
      DWRITE_WORD_WRAPPING_NO_WRAP);
  auto *chevron_format = panel.device.GetTextFormat(
      L"Segoe UI", 12.0f, DWRITE_FONT_WEIGHT_NORMAL,
      DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
      DWRITE_WORD_WRAPPING_NO_WRAP);
  if (!label_format || !chevron_format)
    throw std::runtime_error("Candidate menu text format unavailable");
  const auto size = target->GetSize();
  const float inset = palette_.border_width / 2.0f;
  target->BeginDraw();
  // Clear to nothing: only the rounded card is opaque, so the corners stay
  // transparent rather than showing a square window edge.
  target->Clear(D2D1::ColorF(0.0f, 0.0f, 0.0f, 0.0f));
  const D2D1_ROUNDED_RECT card{
      {inset, inset, size.width - inset, size.height - inset},
      static_cast<float>(metrics_.radius), static_cast<float>(metrics_.radius)};
  // The menu tokens, so the flyout matches the card rather than the system
  // menu it replaces.
  target->FillRoundedRectangle(card, brush(palette_.menu_fill));
  target->DrawRoundedRectangle(card, brush(palette_.menu_border),
                               palette_.border_width);
  for (size_t index = 0; index < panel.items.size(); ++index) {
    const auto row = candidate_menu_row(index, panel.items, metrics_);
    const D2D1_RECT_F rect{static_cast<float>(metrics_.padding),
                           static_cast<float>(row.top),
                           size.width - static_cast<float>(metrics_.padding),
                           static_cast<float>(row.bottom)};
    if (panel.items[index].separator) {
      // A hairline across the card's width, centred in its own row.
      const float y = (rect.top + rect.bottom) / 2.0f;
      target->DrawLine({rect.left, y}, {rect.right, y},
                       brush(palette_.menu_border), 1.0f);
      continue;
    }
    if (index == panel.hovered)
      target->FillRoundedRectangle(
          {rect, palette_.item_radius, palette_.item_radius},
          brush(palette_.menu_hover));
    auto text_color = palette_.menu_text;
    if (!panel.items[index].available)
      text_color.a *= 0.5f;
    auto *row_brush = brush(text_color);
    const auto label = wide(panel.items[index].label);
    target->DrawText(label.c_str(), static_cast<UINT32>(label.size()),
                     label_format,
                     D2D1_RECT_F{rect.left +
                                     static_cast<float>(metrics_.label_inset),
                                 rect.top, rect.right, rect.bottom},
                     row_brush);
    // The chevron says the row opens a list rather than doing something.
    if (panel.items[index].submenu)
      target->DrawText(L"›", 1, chevron_format,
                       D2D1_RECT_F{rect.right -
                                       static_cast<float>(metrics_.chevron_column),
                                   rect.top, rect.right, rect.bottom},
                       row_brush);
  }
  const HRESULT drawn = target->EndDraw();
  // A composition swap chain only reaches the screen once it is presented.
  if (SUCCEEDED(drawn) && FAILED(panel.device.Present()))
    throw std::runtime_error("Candidate menu presentation failed");
  // Signed HRESULT against a macro that is unsigned in some SDK and MinGW
  // versions; see CandidateWindow.cpp for the same comparison.
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    // Losing the device is not a presentation failure; rebuild on the next
    // opening rather than hiding a live menu.
    panel.device.DiscardTarget();
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Candidate menu drawing failed");
}

LRESULT CALLBACK CandidateFlyoutWindow::procedure(HWND window, UINT message,
                                                  WPARAM wparam,
                                                  LPARAM lparam) noexcept {
  auto *self = reinterpret_cast<CandidateFlyoutWindow *>(
      GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    self = static_cast<CandidateFlyoutWindow *>(
        reinterpret_cast<CREATESTRUCTW *>(lparam)->lpCreateParams);
    SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
  }
  if (self) {
    try {
      Panel *panel = self->panel_for(window);
      switch (message) {
      case WM_MOUSEACTIVATE:
        // The menu acts on clicks without taking focus from the application
        // the user is typing into.
        return MA_NOACTIVATE;
      case WM_MOUSEMOVE: {
        // The menu holds the capture, so it receives the pointer wherever it
        // is - including over the submenu, which has to be routed by position
        // rather than by which window the message arrived on.
        POINT screen{static_cast<short>(LOWORD(lparam)),
                     static_cast<short>(HIWORD(lparam))};
        if (panel)
          ClientToScreen(window, &screen);
        for (Panel *target : {&self->submenu_, &self->menu_}) {
          if (!target->window || !IsWindowVisible(target->window))
            continue;
          POINT local = screen;
          ScreenToClient(target->window, &local);
          RECT client{};
          if (!GetClientRect(target->window, &client) ||
              !PtInRect(&client, local))
            continue;
          self->track(*target, local.x, local.y);
          return 0;
        }
        // Off both lists. Clear the menu's highlight, but leave the submenu
        // alone: the pointer crosses the gap between the two on its way there.
        if (self->menu_.hovered != no_row && !self->submenu_.items.empty())
          return 0;
        self->track(self->menu_, -1, -1);
        return 0;
      }
      case WM_LBUTTONUP:
      case WM_RBUTTONUP: {
        POINT screen{static_cast<short>(LOWORD(lparam)),
                     static_cast<short>(HIWORD(lparam))};
        if (panel)
          ClientToScreen(window, &screen);
        // A click outside both lists dismisses, which is the whole reason the
        // capture is held.
        if (!self->contains(screen)) {
          self->hide();
          return 0;
        }
        for (Panel *target : {&self->submenu_, &self->menu_}) {
          if (!target->window || !IsWindowVisible(target->window))
            continue;
          POINT local = screen;
          ScreenToClient(target->window, &local);
          if (const auto row = self->hit(*target, local.x, local.y)) {
            self->choose(*target, *row);
            return 0;
          }
        }
        return 0;
      }
      case WM_CAPTURECHANGED:
        // Something else took the mouse. The menu can no longer see the click
        // that would dismiss it, so it must not stay up.
        if (self->capturing_) {
          self->capturing_ = false;
          self->hide();
        }
        return 0;
      case WM_POWERBROADCAST:
        if (wparam != PBT_APMRESUMEAUTOMATIC &&
            wparam != PBT_APMRESUMECRITICAL && wparam != PBT_APMRESUMESUSPEND)
          break;
        [[fallthrough]];
      case WM_DPICHANGED:
      case WM_DISPLAYCHANGE:
      case WM_DWMCOMPOSITIONCHANGED:
      case WM_SETTINGCHANGE:
        // Neither panel retains the pointer anchor needed to place the flyout
        // again after the display environment changes. Discard both targets
        // and close the whole capture-owned menu; the next right click opens
        // it from the current pointer, DPI and monitor work area.
        self->menu_.device.DiscardTarget();
        self->submenu_.device.DiscardTarget();
        self->hide();
        return message == WM_POWERBROADCAST ? TRUE : 0;
      case WM_PAINT:
        if (panel)
          self->paint(*panel);
        return 0;
      }
    } catch (...) {
      self->failed_ = true;
      self->hide(); // No exception may cross the Win32 callback.
      return 0;
    }
  }
  return DefWindowProcW(window, message, wparam, lparam);
}
} // namespace msime::windows

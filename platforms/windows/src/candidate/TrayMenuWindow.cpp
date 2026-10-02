#include "TrayMenuWindow.h"
#include "IconFont.h"
#include "ServerResources.h"
#include <cmath>
#include <stdexcept>
#include <string>

namespace msime::windows {
namespace {
constexpr wchar_t class_name[] = L"MSIME.Client.Preview.TrayMenu";
constexpr size_t no_row = static_cast<size_t>(-1);
// Segoe Fluent Icons / MDL2 CheckMark, the mark native Windows menus draw.
constexpr wchar_t check_mark_glyph = 0xE73E;
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
    throw std::invalid_argument("Invalid tray menu label");
  const int count =
      MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), nullptr, 0);
  if (!count)
    throw std::invalid_argument("Invalid tray menu label");
  std::wstring result(static_cast<size_t>(count), L'\0');
  if (MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text.data(),
                          static_cast<int>(text.size()), result.data(),
                          count) != count)
    throw std::invalid_argument("Invalid tray menu label");
  return result;
}
} // namespace

TrayMenuWindow::Apartment::Apartment() {
  const HRESULT entered =
      CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
  // S_FALSE only means this thread was already inside the same apartment; the
  // reference still has to be released. A different mode is left untouched.
  if (FAILED(entered) && entered != RPC_E_CHANGED_MODE)
    throw std::runtime_error("Tray menu apartment unavailable");
  owned = entered != RPC_E_CHANGED_MODE;
}
TrayMenuWindow::Apartment::~Apartment() {
  if (owned)
    CoUninitialize();
}

TrayMenuWindow::TrayMenuWindow(TrayMenuCapabilities capabilities,
                               Command command, State state)
    : capabilities_(capabilities), command_(std::move(command)),
      state_(std::move(state)) {
  if (!command_ || !state_)
    throw std::invalid_argument("Missing tray menu callback");
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
    throw std::runtime_error("Tray menu class unavailable");
  // No redirection bitmap: the card is composed with per-pixel alpha, which is
  // what gives it rounded corners instead of a rectangular window cut-out.
  window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW |
                                WS_EX_TOPMOST | WS_EX_NOREDIRECTIONBITMAP,
                            class_name, L"", WS_POPUP, 0, 0, 1, 1, nullptr,
                            nullptr, descriptor.hInstance, this);
  if (!window_)
    throw std::runtime_error("Tray menu window unavailable");
}
TrayMenuWindow::~TrayMenuWindow() {
  if (window_)
    DestroyWindow(window_);
  if (logo_)
    DestroyIcon(logo_);
}
ID2D1Bitmap *TrayMenuWindow::logo_bitmap(int pixels) {
  if (pixels <= 0)
    return nullptr;
  if (!logo_ || logo_pixels_ != pixels) {
    // Loaded at the drawn size rather than LR_SHARED's cached standard size, as the floating toolbar does, so the mark is not resampled.
    const HANDLE loaded =
        LoadImageW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_MSIME_LOGO),
                   IMAGE_ICON, pixels, pixels, LR_DEFAULTCOLOR);
    if (!loaded)
      return nullptr;
    if (logo_)
      DestroyIcon(logo_);
    logo_ = static_cast<HICON>(loaded);
    logo_pixels_ = pixels;
  }
  return device_.GetBitmapFromIcon(logo_, L"icon:tray-logo:" +
                                              std::to_wstring(pixels));
}
void TrayMenuWindow::refresh_items() {
  items_ = tray_menu_items(capabilities_, state_());
  geometry_ = tray_menu_geometry(items_, metrics_);
}
void TrayMenuWindow::set_palette(CandidatePalette palette) {
  palette_ = std::move(palette);
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}
bool TrayMenuWindow::visible() const {
  return window_ && IsWindowVisible(window_);
}
void TrayMenuWindow::hide() {
  hovered_ = no_row;
  items_.clear();
  geometry_ = {};
  if (window_)
    ShowWindow(window_, SW_HIDE);
}
bool TrayMenuWindow::open(int icon_center_x, int icon_top) noexcept {
  try {
    show(icon_center_x, icon_top);
    return visible();
  } catch (...) {
    // Appearance must not be fatal: leave failed_ alone and report no menu.
    hide();
    return false;
  }
}
bool TrayMenuWindow::pointer_inside() const noexcept {
  POINT cursor{};
  RECT bounds{};
  if (!window_ || !GetCursorPos(&cursor) || !GetWindowRect(window_, &bounds))
    return false;
  return PtInRect(&bounds, cursor) != FALSE;
}
void TrayMenuWindow::show(int icon_center_x, int icon_top) {
  DpiScope dpi_scope;
  if (failed_) {
    hide();
    return;
  }
  MONITORINFO monitor{};
  monitor.cbSize = sizeof(monitor);
  if (!GetMonitorInfoW(MonitorFromPoint({icon_center_x, icon_top},
                                        MONITOR_DEFAULTTONEAREST),
                       &monitor))
    throw std::runtime_error("Tray menu monitor unavailable");
  // Read the state once per opening: the rows show what the Server reports now, not what a click later assumed.
  refresh_items();
  hovered_ = no_row;
  const auto &work = monitor.rcWork;
  dpi_ = GetDpiForWindow(window_);
  metrics_ = tray_menu_fitted_metrics(
      items_, TrayMenuMetrics{},
      static_cast<double>(work.bottom - work.top) * 96.0 /
          static_cast<double>(dpi_));
  geometry_ = tray_menu_geometry(items_, metrics_);
  const auto bounds =
      tray_menu_bounds(icon_center_x, icon_top, work.left, work.top, work.right,
                       work.bottom, dpi_, geometry_.size);
  if (!SetWindowPos(window_, HWND_TOPMOST, bounds.x, bounds.y, bounds.width,
                    bounds.height, SWP_NOACTIVATE | SWP_SHOWWINDOW))
    throw std::runtime_error("Tray menu positioning failed");
  InvalidateRect(window_, nullptr, FALSE);
}
std::optional<size_t> TrayMenuWindow::hit(int x, int y) const {
  if (items_.empty() || !dpi_)
    return std::nullopt;
  const double scale = static_cast<double>(dpi_) / 96.0;
  return tray_menu_hit(x / scale, y / scale, items_, metrics_);
}
void TrayMenuWindow::choose(size_t index) {
  if (index >= items_.size() || !tray_menu_actionable(items_[index]))
    return;
  const auto command = items_[index].command;
  // A command that could not run leaves the menu open, so a failure is not mistaken for an applied action.
  if (command_(command) && tray_menu_closes_after(command)) {
    hide();
    return;
  }
  // A switch that stays open redraws from the live state, so the row shows what the Server now reports rather than what the click assumed.
  refresh_items();
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
}
void TrayMenuWindow::paint() {
  DpiScope dpi_scope;
  Painting painting(window_);
  if (!painting.dc)
    throw std::runtime_error("Tray menu painting unavailable");
  if (items_.empty() || geometry_.rows.size() != items_.size()) {
    hide();
    return;
  }
  if (!device_.EnsureForComposition(window_))
    throw std::runtime_error("Tray menu device unavailable");
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Tray menu render target unavailable");
  auto brush = [&](const CandidateColor &color) {
    auto *created = device_.GetSolidColorBrush(
        D2D1::ColorF(color.r, color.g, color.b, color.a));
    if (!created)
      throw std::runtime_error("Tray menu brush unavailable");
    return created;
  };
  auto text_format = [&](const wchar_t *family, double size,
                         DWRITE_FONT_WEIGHT weight,
                         DWRITE_TEXT_ALIGNMENT alignment) {
    auto *format = device_.GetTextFormat(
        family, static_cast<float>(size), weight, alignment,
        DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP);
    if (!format)
      throw std::runtime_error("Tray menu text format unavailable");
    return format;
  };
  auto *label_format =
      text_format(L"Segoe UI", metrics_.font_size, DWRITE_FONT_WEIGHT_NORMAL,
                  DWRITE_TEXT_ALIGNMENT_LEADING);
  auto *title_format =
      text_format(L"Segoe UI", metrics_.font_size, DWRITE_FONT_WEIGHT_SEMI_BOLD,
                  DWRITE_TEXT_ALIGNMENT_LEADING);
  auto *hint_format =
      text_format(L"Segoe UI", metrics_.hint_font_size,
                  DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_TRAILING);
  auto *caption_format =
      text_format(L"Segoe UI", metrics_.hint_font_size,
                  DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_LEADING);
  auto *tool_caption_format =
      text_format(L"Segoe UI", metrics_.hint_font_size,
                  DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER);
  icon_text_format_ =
      text_format(L"Segoe UI", metrics_.font_size, DWRITE_FONT_WEIGHT_NORMAL,
                  DWRITE_TEXT_ALIGNMENT_CENTER);
  if (!icon_family_)
    icon_family_ = icon_font_family(device_.GetDWriteFactory());
  // A glyph from the toolbar's icon font, or its text fallback when this Windows build's font lacks it: a missing glyph would otherwise draw a blank box.
  auto draw_glyph = [&](wchar_t glyph, const wchar_t *fallback,
                        const D2D1_RECT_F &rect, ID2D1Brush *fill) {
    const bool has_glyph =
        glyph && icon_family_ &&
        icon_font_has(device_.GetDWriteFactory(), icon_family_, glyph);
    const wchar_t single[] = {glyph, L'\0'};
    const wchar_t *text = has_glyph ? single : fallback;
    if (!text || !*text)
      return;
    auto *format = has_glyph ? text_format(icon_family_, metrics_.icon_font_size,
                                           DWRITE_FONT_WEIGHT_NORMAL,
                                           DWRITE_TEXT_ALIGNMENT_CENTER)
                             : icon_text_format_;
    target->DrawText(text, static_cast<UINT32>(wcslen(text)), format, rect,
                     fill);
  };
  auto draw_text = [&](const std::string &value, IDWriteTextFormat *format,
                       const D2D1_RECT_F &rect, ID2D1Brush *fill) {
    if (value.empty())
      return;
    const auto text = wide(value);
    target->DrawText(text.c_str(), static_cast<UINT32>(text.size()), format,
                     rect, fill);
  };
  const auto size = target->GetSize();
  const float inset = palette_.border_width / 2.0f;
  const float row_inset = static_cast<float>(metrics_.inset);
  const float item_radius = static_cast<float>(metrics_.item_radius);
  const auto secondary = tray_menu_secondary_color(palette_);
  target->BeginDraw();
  // Clear to nothing: only the rounded card is opaque, so the corners stay
  // transparent rather than showing a square window edge.
  target->Clear(D2D1::ColorF(0.0f, 0.0f, 0.0f, 0.0f));
  const D2D1_ROUNDED_RECT card{
      {inset, inset, size.width - inset, size.height - inset},
      static_cast<float>(metrics_.radius), static_cast<float>(metrics_.radius)};
  target->FillRoundedRectangle(card, brush(palette_.surface));
  target->DrawRoundedRectangle(card, brush(palette_.border),
                               palette_.border_width);
  for (size_t index = 0; index < items_.size(); ++index) {
    const auto &item = items_[index];
    const auto &row = geometry_.rows[index];
    const D2D1_RECT_F rect{static_cast<float>(row.left),
                           static_cast<float>(row.top),
                           static_cast<float>(row.right),
                           static_cast<float>(row.bottom)};
    const bool hovered = index == hovered_ && tray_menu_actionable(item);
    switch (item.kind) {
    case TrayMenuRowKind::Header: {
      // The design's head: the 18px product mark and the name in semibold.
      const float logo = static_cast<float>(metrics_.logo_size);
      const float centre = (rect.top + rect.bottom) / 2.0f;
      const D2D1_RECT_F mark{rect.left + row_inset, centre - logo / 2.0f,
                             rect.left + row_inset + logo,
                             centre + logo / 2.0f};
      float text_left = rect.left + row_inset;
      const int pixels = static_cast<int>(
          std::lround(metrics_.logo_size * static_cast<double>(dpi_) / 96.0));
      if (auto *bitmap = logo_bitmap(pixels)) {
        target->DrawBitmap(bitmap, mark, 1.0f,
                           D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
        text_left = mark.right + static_cast<float>(metrics_.gap);
      }
      draw_text(item.label, title_format,
                {text_left, rect.top, rect.right - row_inset, rect.bottom},
                brush(palette_.text));
      break;
    }
    case TrayMenuRowKind::Separator: {
      // A 1px hairline across the card's content width, centred in its margins.
      const float centre = (rect.top + rect.bottom) / 2.0f;
      target->FillRectangle({rect.left, centre - 0.5f, rect.right, centre + 0.5f},
                            brush(palette_.border));
      break;
    }
    case TrayMenuRowKind::Label:
      // A group caption: 12px secondary text, 4px above and 2px below.
      draw_text(item.label, caption_format,
                {rect.left + row_inset, rect.top + 4.0f, rect.right - row_inset,
                 rect.bottom - 2.0f},
                brush(secondary));
      break;
    case TrayMenuRowKind::Item: {
      if (hovered)
        target->FillRoundedRectangle({rect, item_radius, item_radius},
                                     brush(palette_.hover));
      // An unavailable row is dimmed with the muted token instead of hidden.
      auto *text_brush =
          brush(item.available ? palette_.text : palette_.number);
      auto *hint_brush = brush(item.available ? secondary : palette_.number);
      const float mark_left =
          rect.left + static_cast<float>(tray_menu_mark_x(metrics_));
      if (item.checked)
        draw_glyph(check_mark_glyph, L"✓",
                   {mark_left, rect.top,
                    mark_left + static_cast<float>(metrics_.mark_column),
                    rect.bottom},
                   text_brush);
      // Labels start after the mark column so they line up across rows; the hint is right-aligned in the same span, as the design's flex row puts it.
      const D2D1_RECT_F text{
          rect.left + static_cast<float>(tray_menu_label_x(metrics_)), rect.top,
          rect.right - row_inset, rect.bottom};
      draw_text(item.label, label_format, text, text_brush);
      draw_text(item.hint, hint_format, text, hint_brush);
      break;
    }
    case TrayMenuRowKind::Tool: {
      // A tool that is on is filled with the accent, the switch the old row drew; the rest take the hover fill like any row.
      const bool on = item.toggle && item.checked && item.available;
      const D2D1_ROUNDED_RECT cell{
          {rect.left + 1.0f, rect.top, rect.right - 1.0f, rect.bottom},
          item_radius, item_radius};
      if (on)
        target->FillRoundedRectangle(cell, brush(palette_.accent));
      else if (hovered)
        target->FillRoundedRectangle(cell, brush(palette_.hover));
      const auto glyph_color =
          on ? candidate_on_accent(palette_.accent)
             : item.available ? palette_.text : palette_.number;
      const auto caption_color =
          on ? candidate_on_accent(palette_.accent)
             : item.available ? secondary : palette_.number;
      draw_glyph(item.icon, item.icon_fallback,
                 {rect.left, rect.top + 6.0f, rect.right, rect.top + 28.0f},
                 brush(glyph_color));
      draw_text(item.label, tool_caption_format,
                {rect.left + 2.0f, rect.top + 30.0f, rect.right - 2.0f,
                 rect.bottom - 6.0f},
                brush(caption_color));
      break;
    }
    }
  }
  const HRESULT drawn = target->EndDraw();
  // A composition swap chain only reaches the screen once it is presented.
  if (SUCCEEDED(drawn) && FAILED(device_.Present()))
    throw std::runtime_error("Tray menu presentation failed");
  // Signed HRESULT against a macro that is unsigned in some SDK and MinGW
  // versions; see CandidateWindow.cpp for the same comparison.
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    // Losing the device is not a presentation failure; rebuild on the next
    // opening rather than hiding a live menu.
    device_.DiscardTarget();
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Tray menu drawing failed");
}
LRESULT CALLBACK TrayMenuWindow::procedure(HWND window, UINT message,
                                           WPARAM wparam,
                                           LPARAM lparam) noexcept {
  auto *self = reinterpret_cast<TrayMenuWindow *>(
      GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    self = static_cast<TrayMenuWindow *>(
        reinterpret_cast<CREATESTRUCTW *>(lparam)->lpCreateParams);
    SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
    self->window_ = window;
  }
  if (self) {
    try {
      switch (message) {
      case WM_MOUSEACTIVATE:
        // The menu acts on clicks without taking focus from the application
        // the user is typing into.
        return MA_NOACTIVATE;
      case WM_MOUSEMOVE: {
        const auto row = self->hit(static_cast<short>(LOWORD(lparam)),
                                   static_cast<short>(HIWORD(lparam)));
        const size_t hovered = row ? *row : no_row;
        if (hovered != self->hovered_) {
          self->hovered_ = hovered;
          InvalidateRect(window, nullptr, FALSE);
          TRACKMOUSEEVENT track{sizeof(track), TME_LEAVE, window, 0};
          TrackMouseEvent(&track);
        }
        return 0;
      }
      case WM_MOUSELEAVE:
        if (self->hovered_ != no_row) {
          self->hovered_ = no_row;
          InvalidateRect(window, nullptr, FALSE);
        }
        return 0;
      case WM_LBUTTONUP: {
        const auto row = self->hit(static_cast<short>(LOWORD(lparam)),
                                   static_cast<short>(HIWORD(lparam)));
        if (row)
          self->choose(*row);
        return 0;
      }
      case WM_KILLFOCUS:
        self->hide();
        return 0;
      case WM_POWERBROADCAST:
        if (wparam != PBT_APMRESUMEAUTOMATIC &&
            wparam != PBT_APMRESUMECRITICAL && wparam != PBT_APMRESUMESUSPEND)
          break;
        [[fallthrough]];
      case WM_DISPLAYCHANGE:
      case WM_DWMCOMPOSITIONCHANGED:
      case WM_DPICHANGED:
        // This transient menu does not retain the tray icon anchor needed to
        // place itself again. Close it and discard the display-bound target;
        // the next click reopens from the current icon, DPI and work area.
        self->device_.DiscardTarget();
        self->hide();
        return message == WM_POWERBROADCAST ? TRUE : 0;
      case WM_PAINT:
        self->paint();
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

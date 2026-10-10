#include "TrayMenuWindow.h"
#include "AccessibleWindow.h"
#include "IconFont.h"
#include "TrayMenuAccessibility.h"
#include "ServerResources.h"
#include "WaveOverlayUtils.h"
#include <bitset>
#include <cmath>
#include <stdexcept>
#include <string>
#include <utility>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t class_name[] = L"MSIME.Client.Preview.TrayMenu" MSIME_EDITION_NAME_SUFFIX;
constexpr size_t no_row = static_cast<size_t>(-1);
// Segoe Fluent Icons / MDL2 CheckMark, the mark native Windows menus draw.
constexpr wchar_t check_mark_glyph = 0xE73E;
// 同一套图标字体里的 ChevronLeft 和 ChevronRight：返回行和翻页行的箭头。
constexpr wchar_t chevron_left_glyph = 0xE76B;
constexpr wchar_t chevron_right_glyph = 0xE76C;
// 键盘钩子把导航键投递给卡片的消息，wparam 是 TrayMenuKey。
constexpr UINT keyboard_message = WM_APP + 0x54;
// 低级键盘钩子在装它的线程（UI 线程）上回调，这几项只在那个线程上读写。`swallowed_keys` 记下吞掉了按下的键，它们的松开也一并吞掉，应用不会收到没有按下的松开。
HHOOK keyboard_hook = nullptr;
TrayMenuWindow *keyboard_owner = nullptr;
std::bitset<256> swallowed_keys;
// 读屏键（Caps Lock、Insert）此刻是否按着，由钩子按它看到的按下和松开记录：读屏自己的钩子会吞掉读屏键，GetAsyncKeyState 读不到它按着。
bool caps_lock_held = false;
bool insert_held = false;
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
  create_window();
}
TrayMenuWindow::TrayMenuWindow(Items items, Command command)
    : command_(std::move(command)), items_builder_(std::move(items)) {
  if (!command_ || !items_builder_)
    throw std::invalid_argument("Missing tray menu callback");
  create_window();
}
void TrayMenuWindow::create_window() {
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
  // 与 macOS 输入菜单（原生 NSMenu）自带的读屏对应：读屏通过 UI Automation 读到每一行，键盘导航时焦点跟着高亮走。
  accessible_ = std::make_unique<AccessibleWindow>(window_);
}
TrayMenuWindow::~TrayMenuWindow() {
  stop_keyboard();
  // 先断开读屏拿着的提供者，再销毁窗口。
  accessible_.reset();
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
  items_ = items_builder_ ? items_builder_() : tray_menu_items(capabilities_, state_(), page_);
  geometry_ = items_.empty() ? TrayMenuGeometry{} : tray_menu_geometry(items_, metrics_);
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
  stop_keyboard();
  // 收起之前报告菜单关闭，读屏随即离开这张卡片。
  if (accessible_ && visible())
    accessible_->menu_closed();
  hovered_ = no_row;
  keyboard_highlight_ = false;
  items_.clear();
  geometry_ = {};
  if (window_)
    ShowWindow(window_, SW_HIDE);
  sync_accessibility();
}
void TrayMenuWindow::sync_accessibility() {
  if (!accessible_)
    return;
  if (!visible() || items_.empty() || geometry_.rows.size() != items_.size()) {
    accessible_->publish({AccessibleContainer::Menu, {}, {}});
    return;
  }
  const auto highlight = keyboard_highlight_ && hovered_ < items_.size()
                             ? std::optional<size_t>(hovered_)
                             : std::nullopt;
  accessible_->publish(tray_menu_accessible_tree(items_, geometry_,
                                                 static_cast<double>(dpi_) / 96.0, highlight));
}
void TrayMenuWindow::invoke_accessible(int id, LPARAM token) {
  if (!accessible_ || !accessible_->current(token) || !visible() || id < 1 ||
      static_cast<size_t>(id) > items_.size())
    return;
  // 读屏执行和按键一样算在用卡片，不按闲置收起。
  keyboard_at_ = GetTickCount64();
  choose(static_cast<size_t>(id - 1), true);
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
bool TrayMenuWindow::open_beside(int anchor_center_x, int anchor_top,
                                 int anchor_bottom) noexcept {
  try {
    place(anchor_center_x, anchor_top, anchor_bottom);
    return visible();
  } catch (...) {
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
  // 每次打开都从主页开始，和原生菜单每次从顶层展开一样。
  page_ = TrayMenuPage::Main;
  place(icon_center_x, icon_top, std::nullopt);
}
void TrayMenuWindow::place(int icon_center_x, int icon_top,
                           std::optional<int> anchor_bottom) {
  DpiScope dpi_scope;
  if (failed_) {
    hide();
    return;
  }
  const bool was_visible = visible();
  anchor_x_ = icon_center_x;
  anchor_top_ = icon_top;
  anchor_bottom_ = anchor_bottom;
  MONITORINFO monitor{};
  monitor.cbSize = sizeof(monitor);
  const HMONITOR target = MonitorFromPoint({icon_center_x, icon_top}, MONITOR_DEFAULTTONEAREST);
  if (!GetMonitorInfoW(target, &monitor))
    throw std::runtime_error("Tray menu monitor unavailable");
  // Read the state once per opening: the rows show what the Server reports now, not what a click later assumed.
  refresh_items();
  hovered_ = no_row;
  keyboard_highlight_ = false;
  // 空的一组行没有可画的东西，不开卡片。
  if (items_.empty()) {
    hide();
    return;
  }
  const auto &work = monitor.rcWork;
  // 尺寸按卡片要去的那块显示器的 DPI 算，而不是窗口上次所在的显示器：工具栏在缩放不同的副屏上时，按旧 DPI 算的卡片挪过去后大小不对。
  dpi_ = monitor_effective_dpi(target);
  metrics_ = tray_menu_fitted_metrics(
      items_, TrayMenuMetrics{},
      static_cast<double>(work.bottom - work.top) * 96.0 /
          static_cast<double>(dpi_));
  geometry_ = tray_menu_geometry(items_, metrics_);
  const auto bounds =
      anchor_bottom
          ? toolbar_menu_bounds(icon_center_x, icon_top, *anchor_bottom, work.left,
                                work.top, work.right, work.bottom, dpi_,
                                geometry_.size)
          : tray_menu_bounds(icon_center_x, icon_top, work.left, work.top,
                             work.right, work.bottom, dpi_, geometry_.size);
  placing_ = true;
  const BOOL moved = SetWindowPos(window_, HWND_TOPMOST, bounds.x, bounds.y, bounds.width,
                                  bounds.height, SWP_NOACTIVATE | SWP_SHOWWINDOW);
  placing_ = false;
  if (!moved)
    throw std::runtime_error("Tray menu positioning failed");
  // 挪动期间的消息（显示环境变化、失去焦点）可能已经把卡片收起；收起的卡片不能装键盘钩子，否则导航键在所有程序里都被吞掉。
  if (!visible())
    return;
  start_keyboard();
  InvalidateRect(window_, nullptr, FALSE);
  sync_accessibility();
  // 翻页时卡片一直开着，只有从隐藏到显示才算打开菜单。
  if (accessible_ && !was_visible)
    accessible_->menu_opened();
}
void TrayMenuWindow::start_keyboard() {
  if (keyboard_owner == this)
    return;
  // 另一张卡片还开着时钩子已经装着，换个主人即可。
  if (!keyboard_hook) {
    keyboard_hook = SetWindowsHookExW(WH_KEYBOARD_LL, keyboard_procedure,
                                      GetModuleHandleW(nullptr), 0);
    // 装不上只是少了键盘导航，鼠标照常可用。
    if (!keyboard_hook)
      return;
    // 上一张卡片收起时，吞掉的键可能还没松开，钩子先卸了，那次松开已经交给了应用。新装的钩子不能再按旧记录吞掉下一次松开，否则应用只收到按下、收不到松开。
    swallowed_keys.reset();
    caps_lock_held = false;
    insert_held = false;
  }
  keyboard_owner = this;
  keyboard_at_ = 0;
}
void TrayMenuWindow::stop_keyboard() {
  if (keyboard_owner != this)
    return;
  keyboard_owner = nullptr;
  if (keyboard_hook)
    UnhookWindowsHookEx(keyboard_hook);
  keyboard_hook = nullptr;
}
LRESULT CALLBACK TrayMenuWindow::keyboard_procedure(int code, WPARAM wparam,
                                                    LPARAM lparam) noexcept {
  if (code != HC_ACTION || !keyboard_owner || !lparam)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  const auto *event = reinterpret_cast<const KBDLLHOOKSTRUCT *>(lparam);
  // 别的程序模拟的按键（包括共享应用面板往编辑器里输入的文字）不当作导航。
  if (event->flags & LLKHF_INJECTED)
    return CallNextHookEx(nullptr, code, wparam, lparam);
  const unsigned virtual_key = event->vkCode & 0xFFu;
  const bool released = wparam == WM_KEYUP || wparam == WM_SYSKEYUP;
  if (virtual_key == tray_menu_caps_lock_key)
    caps_lock_held = !released;
  else if (virtual_key == tray_menu_insert_key)
    insert_held = !released;
  if (released) {
    if (swallowed_keys.test(virtual_key)) {
      swallowed_keys.reset(virtual_key);
      return 1;
    }
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  // 卡片已经看不见时钩子不再导航，按下原样交给应用；正常情况下 hide() 会先卸掉钩子，这里兜住没卸掉的情况。
  if (!IsWindowVisible(keyboard_owner->window_)) {
    swallowed_keys.reset(virtual_key);
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  // 读屏的命令原样交给读屏，卡片不导航也不收起；读屏在读这张卡片，算作在用它，不按闲置收起。
  const bool reader_key_down =
      caps_lock_held || insert_held ||
      ((GetAsyncKeyState(VK_CAPITAL) | GetAsyncKeyState(VK_INSERT)) & 0x8000) != 0;
  if (tray_menu_screen_reader_key(virtual_key, reader_key_down)) {
    keyboard_owner->keyboard_at_ = GetTickCount64();
    swallowed_keys.reset(virtual_key);
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  // Shift 也算修饰键：Shift+方向键之类不是菜单导航，和原生菜单一样收起卡片并交给应用。吞掉它们会让 TIP 只看到 Shift 的按下和松开，把这次 Shift 当成单按，切换中英文。
  const bool modified =
      ((GetAsyncKeyState(VK_CONTROL) | GetAsyncKeyState(VK_MENU) | GetAsyncKeyState(VK_SHIFT) |
        GetAsyncKeyState(VK_LWIN) | GetAsyncKeyState(VK_RWIN)) &
       0x8000) != 0;
  const auto &owner = *keyboard_owner;
  const bool highlighted = owner.hovered_ < owner.items_.size() &&
                           tray_menu_actionable(owner.items_[owner.hovered_]);
  const auto key = tray_menu_key(virtual_key, modified, highlighted);
  // 交给应用的按下，它的松开也交给应用。
  if (key == TrayMenuKey::None) {
    swallowed_keys.reset(virtual_key);
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  // 钩子回调要尽快返回，执行命令、重画卡片都放到卡片自己的消息里做。
  PostMessageW(keyboard_owner->window_, keyboard_message,
               static_cast<WPARAM>(key), 0);
  if (!tray_menu_key_swallowed(key)) {
    swallowed_keys.reset(virtual_key);
    return CallNextHookEx(nullptr, code, wparam, lparam);
  }
  swallowed_keys.set(virtual_key);
  return 1;
}
void TrayMenuWindow::key(TrayMenuKey key) {
  if (!visible() || items_.empty())
    return;
  keyboard_at_ = GetTickCount64();
  const auto current =
      hovered_ < items_.size() ? std::optional<size_t>(hovered_) : std::nullopt;
  const auto result =
      tray_menu_key_result(items_, current, key, page_ != TrayMenuPage::Main);
  if (result.close) {
    hide();
    return;
  }
  if (result.back) {
    show_page(TrayMenuPage::Main, true);
    return;
  }
  const size_t highlight = result.highlight ? *result.highlight : no_row;
  if (highlight != hovered_ || !keyboard_highlight_) {
    hovered_ = highlight;
    keyboard_highlight_ = true;
    InvalidateRect(window_, nullptr, FALSE);
    // 键盘高亮就是读屏的焦点：报告焦点移到这一行，读屏随即读出它。
    sync_accessibility();
    if (accessible_ && hovered_ < items_.size())
      accessible_->focus(static_cast<int>(hovered_ + 1));
  }
  if (result.activate && result.highlight)
    choose(*result.highlight, true);
}
void TrayMenuWindow::show_page(TrayMenuPage page, bool from_keyboard) {
  const auto from = page_;
  page_ = page;
  place(anchor_x_, anchor_top_, anchor_bottom_);
  if (!visible())
    return;
  // 鼠标翻页时高亮跟着指针走；键盘翻页时先高亮一行，接着按上下键就有起点。
  if (from_keyboard) {
    const auto highlight = tray_menu_page_highlight(items_, from);
    hovered_ = highlight ? *highlight : no_row;
    keyboard_highlight_ = true;
  }
  InvalidateRect(window_, nullptr, FALSE);
  sync_accessibility();
  if (from_keyboard && accessible_ && hovered_ < items_.size())
    accessible_->focus(static_cast<int>(hovered_ + 1));
}
std::optional<size_t> TrayMenuWindow::hit(int x, int y) const {
  if (items_.empty() || !dpi_)
    return std::nullopt;
  const double scale = static_cast<double>(dpi_) / 96.0;
  return tray_menu_hit(x / scale, y / scale, items_, metrics_);
}
void TrayMenuWindow::choose(size_t index, bool from_keyboard) {
  if (index >= items_.size() || !tray_menu_actionable(items_[index]))
    return;
  const auto command = items_[index].command;
  // 翻页只在卡片里处理，不交给 Server。
  if (const auto page = tray_menu_page_target(command)) {
    show_page(*page, from_keyboard);
    return;
  }
  if (command == TrayMenuCommand::SelectTheme) {
    if (theme_action_ && theme_action_(items_[index].value))
      hide();
    return;
  }
  // A command that could not run leaves the menu open, so a failure is not mistaken for an applied action.
  if (command_(command) && tray_menu_closes_after(command)) {
    hide();
    return;
  }
  // A switch that stays open redraws from the live state, so the row shows what the Server now reports rather than what the click assumed.
  refresh_items();
  if (window_)
    InvalidateRect(window_, nullptr, FALSE);
  sync_accessibility();
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
      const D2D1_RECT_F mark{mark_left, rect.top,
                             mark_left + static_cast<float>(metrics_.mark_column),
                             rect.bottom};
      // 返回行在勾选列画向左的箭头，勾选行画勾。
      if (item.back)
        draw_glyph(chevron_left_glyph, L"‹", mark, text_brush);
      else if (item.checked)
        draw_glyph(check_mark_glyph, L"✓", mark, text_brush);
      // Labels start after the mark column so they line up across rows; the hint is right-aligned in the same span, as the design's flex row puts it.
      D2D1_RECT_F text{
          rect.left + static_cast<float>(tray_menu_label_x(metrics_)), rect.top,
          rect.right - row_inset, rect.bottom};
      // 翻页行在最右边画向右的箭头，提示文字让到箭头左边。
      if (item.submenu) {
        const float column = static_cast<float>(metrics_.mark_column);
        draw_glyph(chevron_right_glyph, L"›",
                   {text.right - column, rect.top, text.right, rect.bottom},
                   hint_brush);
        text.right -= column + static_cast<float>(metrics_.gap) / 2.0f;
      }
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
          // 指针接管了高亮，键盘焦点不再停在原来那一行。
          if (std::exchange(self->keyboard_highlight_, false))
            self->sync_accessibility();
        }
        return 0;
      }
      case WM_MOUSELEAVE:
        if (self->hovered_ != no_row) {
          self->hovered_ = no_row;
          InvalidateRect(window, nullptr, FALSE);
          if (std::exchange(self->keyboard_highlight_, false))
            self->sync_accessibility();
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
      case WM_DPICHANGED:
        // place() 把卡片挪到另一块 DPI 不同的显示器时同步收到这条消息：尺寸已经按那块显示器算好，只丢掉按旧 DPI 建的绘制目标，不采用系统建议的矩形，也不收起刚打开的卡片。
        if (self->placing_) {
          self->device_.DiscardTarget();
          return 0;
        }
        self->device_.DiscardTarget();
        self->hide();
        return 0;
      case WM_POWERBROADCAST:
        if (wparam != PBT_APMRESUMEAUTOMATIC &&
            wparam != PBT_APMRESUMECRITICAL && wparam != PBT_APMRESUMESUSPEND)
          break;
        [[fallthrough]];
      case WM_DISPLAYCHANGE:
      case WM_DWMCOMPOSITIONCHANGED:
        // This transient menu does not retain the tray icon anchor needed to
        // place itself again. Close it and discard the display-bound target;
        // the next click reopens from the current icon, DPI and work area.
        self->device_.DiscardTarget();
        self->hide();
        return message == WM_POWERBROADCAST ? TRUE : 0;
      case WM_PAINT:
        self->paint();
        return 0;
      case keyboard_message:
        self->key(static_cast<TrayMenuKey>(wparam));
        return 0;
      case WM_GETOBJECT:
        if (self->accessible_)
          if (const auto answer = self->accessible_->answer(wparam, lparam))
            return *answer;
        break;
      case accessible_invoke_message:
        self->invoke_accessible(static_cast<int>(wparam), lparam);
        return 0;
      case WM_DESTROY:
        if (self->accessible_)
          self->accessible_->disconnect();
        break;
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

#include "FloatingToolbarWindow.h"
#include "AccessibleWindow.h"
#include "FloatingToolbarPlacement.h"
#include "ToolbarAccessibility.h"
#include "ToolbarIcons.h"
#include "ToolbarLayout.h"
#include "ToolbarCoordinates.h"
#include "ToolbarClick.h"
#include "ToolbarModeCommand.h"
#include "ToolbarTooltips.h"
#include "ServerResources.h"
#include "WindowShadow.h"
#include "WaveOverlayUtils.h"
#include "IconFont.h"
#include <algorithm>
#include <cmath>
#include <stdexcept>
#include <string>
#include <windowsx.h>
#include <commctrl.h>
#include <vector>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t kClassName[] = L"MSIME.Client.Preview.FloatingToolbar" MSIME_EDITION_NAME_SUFFIX;
constexpr int kWidth = 732;
constexpr int kHeight = 52;
// 图片工具栏自己的悬停/按下药丸的圆角，DIP。普通条的悬停圆角跟按钮尺寸走（toolbar_button_radius），toolbar_palette 不再设 item_radius，药丸用这个固定值，保持合并前图片工具栏的 6.5。
constexpr float kImagePillRadius = 6.5f;
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
// 按钮名字交给读屏时用 UTF-8（AccessibleElement 的约定）。
std::string utf8(const std::wstring &text) {
  if (text.empty())
    return {};
  const int count = WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()),
                                        nullptr, 0, nullptr, nullptr);
  if (count <= 0)
    return {};
  std::string result(static_cast<size_t>(count), '\0');
  if (WideCharToMultiByte(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), result.data(),
                          count, nullptr, nullptr) != count)
    return {};
  return result;
}
} // namespace

std::vector<int> FloatingToolbarWindow::slots() const {
  if (image_.active()) {
    std::vector<int> active;
    for (const auto &button : image_.buttons) active.push_back(button.id);
    return active;
  }
  return floating_toolbar_slots(layout_, MSIME_EDITION_HANDWRITING != 0);
}
ToolbarRect FloatingToolbarWindow::cell(size_t index) const {
  return image_.active() ? image_.buttons.at(index).rect : toolbar_cell(index, metrics());
}
std::optional<size_t> FloatingToolbarWindow::button_at_pixel(double x, double y) const {
  if (image_.active()) {
    const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    return image_.button_at(x / unit, y / unit);
  }
  return toolbar_button_at_pixel(x, y, GetDpiForWindow(window_), scale_, slots().size(), metrics());
}
bool FloatingToolbarWindow::drag_at_pixel(double x, double y) const {
  if (image_.active()) {
    const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    return image_.drag_at(x / unit, y / unit);
  }
  return toolbar_drag_at_pixel(x, y, GetDpiForWindow(window_), scale_, slots().size(), metrics());
}
bool FloatingToolbarWindow::card_at_pixel(double x, double y) const {
  return image_.active() ? (button_at_pixel(x, y).has_value() || drag_at_pixel(x, y))
      : toolbar_card_at_pixel(x, y, GetDpiForWindow(window_), scale_, slots().size(), metrics());
}
void FloatingToolbarWindow::update_image() {
  image_dirty_ = false;
  if (!image_.active() || !shown_) return;
  const std::string language = language_.caps_lock ? "caps" : !shown_->chinese ? "en"
      : language_.dedicated_english ? "en-candidate" : language_.mode == scheme::InputMode::Japanese ? "ja" : "cn";
  // 按共享键序请求，不用普通条标点在前的顺序，见 floating_toolbar_image_slots。
  image_.update(floating_toolbar_image_slots(floating_toolbar_slots(layout_, MSIME_EDITION_HANDWRITING != 0)),
      {language, shown_->fullwidth ? "full" : "half", shown_->chinese_punctuation ? "cn" : "en",
       shown_character_set_.value_or(false) ? "traditional" : "simplified", "", "", ""});
}
void FloatingToolbarWindow::set_image_skin(nlohmann::json manifest, std::filesystem::path directory) {
  try {
    image_.set_skin(std::move(manifest), std::move(directory));
    image_dirty_ = true;
    if (!image_.active() && window_) {
      SetWindowRgn(window_, nullptr, TRUE);
      KillTimer(window_, 1);
    }
    shown_.reset();
    if (window_) InvalidateRect(window_, nullptr, FALSE);
  } catch (...) { fail(failure_at_stage("toolbar_image", static_cast<uint32_t>(GetLastError()))); }
}
ToolbarMetrics FloatingToolbarWindow::metrics() const {
  return toolbar_metrics(static_cast<double>(font_size_), true, layout_.show_logo);
}
// 只有设置和手写要靠外壳打开：设置按钮要设置窗口，手写按钮要共享应用。三个模式按钮、简繁、语音、切换输入方案和隐藏都在这个进程里处理；表情和屏幕键盘没有共享应用时打开系统自带的面板，所以也总是可用。
bool FloatingToolbarWindow::usable(int button) const {
  switch (button) {
  case kToolbarSettings:
    return settings_available_;
  case kToolbarHandwriting:
    return panels_available_;
  default:
    return true;
  }
}
ToolbarMenuAnchor FloatingToolbarWindow::menu_anchor(std::optional<size_t> index,
                                                     int x) const {
  // 工具栏不感知 DPI，这里（它的窗口过程里）量到的外框和点击坐标都是逻辑坐标；弹出菜单在每显示器感知的上下文里按物理像素摆放，所以在每显示器感知的上下文里再量一次同一个外框，把锚点换过去。缩放不是 100% 时不换的话，菜单会开到离按钮很远的地方，副屏旁边还可能开到另一块屏上。
  RECT window{};
  GetWindowRect(window_, &window);
  RECT physical = window;
  {
    WaveOverlayDpiScope dpi_scope;
    if (!GetWindowRect(window_, &physical))
      physical = window;
  }
  const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
  const auto card = image_.active() ? ToolbarRect{0, 0, image_.width, image_.height} : toolbar_card(slots().size(), metrics());
  double centre = static_cast<double>(x);
  if (index) {
    const auto box = cell(*index);
    centre = (box.left + box.right) / 2.0 * unit;
  }
  const auto horizontal = [&](int value) {
    return toolbar_physical_coordinate(value, window.left, window.right, physical.left,
                                       physical.right);
  };
  const auto vertical = [&](int value) {
    return toolbar_physical_coordinate(value, window.top, window.bottom, physical.top,
                                       physical.bottom);
  };
  return {horizontal(window.left + static_cast<int>(std::lround(centre))),
          vertical(window.top + static_cast<int>(std::lround(card.top * unit))),
          vertical(window.top + static_cast<int>(std::lround(card.bottom * unit)))};
}
void FloatingToolbarWindow::sync_tooltips() {
  if (!tooltip_)
    return;
  TTTOOLINFOW tool{};
  // V2 的大小在没有 comctl32 v6 清单的进程里也被接受；带 lpReserved 的完整大小会让旧版控件拒绝登记。
  tool.cbSize = TTTOOLINFOW_V2_SIZE;
  tool.hwnd = window_;
  for (size_t id = 0; id < tooltip_tools_; ++id) {
    tool.uId = id;
    SendMessageW(tooltip_, TTM_DELTOOLW, 0, reinterpret_cast<LPARAM>(&tool));
  }
  tooltip_tools_ = 0;
  const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
  const auto count = slots().size();
  for (size_t index = 0; index < count; ++index) {
    const auto cell = this->cell(index);
    tool = {};
    tool.cbSize = TTTOOLINFOW_V2_SIZE;
    tool.uFlags = TTF_SUBCLASS;
    tool.hwnd = window_;
    tool.uId = index;
    tool.rect = {static_cast<LONG>(std::floor(cell.left * unit)),
                 static_cast<LONG>(std::floor(cell.top * unit)),
                 static_cast<LONG>(std::ceil(cell.right * unit)),
                 static_cast<LONG>(std::ceil(cell.bottom * unit))};
    tool.lpszText = LPSTR_TEXTCALLBACKW;
    if (!SendMessageW(tooltip_, TTM_ADDTOOLW, 0, reinterpret_cast<LPARAM>(&tool)))
      break;
    tooltip_tools_ = index + 1;
  }
}
std::wstring FloatingToolbarWindow::tooltip_text(size_t index) {
  const auto active = slots();
  if (index >= active.size())
    return {};
  return button_name(active[index], reader_());
}
std::wstring FloatingToolbarWindow::button_name(int button,
                                                const std::optional<ModePresentation> &value) const {
  std::optional<bool> state;
  if (value && button <= kToolbarCharacterSet) {
    switch (button) {
    case kToolbarLanguage:
      state = value->chinese;
      break;
    case kToolbarFullwidth:
      state = value->fullwidth;
      break;
    case kToolbarPunctuation:
      state = value->chinese_punctuation;
      break;
    default:
      state = shown_character_set_;
      break;
    }
  }
  return toolbar_tooltip(button, state, language_, scheme_title_, panels_available_,
                         MSIME_EDITION_DISPLAY_NAME);
}
void FloatingToolbarWindow::sync_accessibility(const std::optional<ModePresentation> &value) {
  if (!accessible_)
    return;
  std::vector<ToolbarAccessibleButton> buttons;
  for (const int button : slots())
    buttons.push_back({button, utf8(button_name(button, value)), usable(button)});
  auto tree = toolbar_accessible_tree(
      buttons, metrics(), toolbar_pixel_unit(GetDpiForWindow(window_), scale_), layout_.show_logo,
      MSIME_EDITION_DISPLAY_NAME_UTF8);
  if (image_.active()) {
    tree = toolbar_accessible_tree(buttons, metrics(), 1, false, MSIME_EDITION_DISPLAY_NAME_UTF8);
    const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    for (size_t at = 0; at < tree.elements.size(); ++at) {
      const auto box = cell(at);
      tree.elements[at].bounds = {std::floor(box.left * unit), std::floor(box.top * unit),
                                  std::ceil(box.right * unit), std::ceil(box.bottom * unit)};
    }
  }
  accessible_->publish(std::move(tree));
}
void FloatingToolbarWindow::invoke_accessible(int id, LPARAM token) {
  if (!accessible_ || !accessible_->current(token) || !IsWindowVisible(window_))
    return;
  // 与鼠标点击同样的条件：按钮画出来时的焦点租约还是当前的。
  const auto value = reader_();
  if (!value || !shown_ || !same(value->lease, shown_->lease))
    return;
  const auto active = slots();
  const auto found = std::find_if(active.begin(), active.end(), [id](int button) {
    return toolbar_accessible_id(button) == id;
  });
  if (found == active.end())
    return;
  if (activity_action_)
    activity_action_();
  run(static_cast<size_t>(found - active.begin()), *value, 0);
}
void FloatingToolbarWindow::run(size_t position, const ModePresentation &value, int x) {
  const auto active = slots();
  if (position >= active.size())
    return;
  const int slot = active[position];
  if (!usable(slot)) {
    // 没有可打开的界面，按下不算动作：和托盘一样什么都不做，不显示按下再丢掉命令。
  }
  else if (slot <= kToolbarPunctuation) {
    if (auto command = toolbar_mode_command(
            slot, value.chinese, value.fullwidth, value.chinese_punctuation))
      click_(ModeClick{value.lease, *command});
  }
  else if (slot == 3 && character_set_action_)
    character_set_action_();
  else if (slot == 4 && emoji_action_)
    emoji_action_();
  else if (slot == 5 && keyboard_action_)
    keyboard_action_();
  else if (slot == 6 && settings_action_)
    settings_action_();
  else if (slot == kToolbarHandwriting && handwriting_action_)
    handwriting_action_();
  else if (slot == kToolbarVoice && voice_action_)
    voice_action_();
  else if (slot == kToolbarInputScheme && input_scheme_action_)
    input_scheme_action_(menu_anchor(position, x));
  else if (slot == 10 && hide_action_)
    hide_action_();
}

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
  // 与 macOS 工具栏每个按钮的 accessibilityLabel 对应：读屏通过 UI Automation 读到按钮的名字和状态，也能执行按钮。
  accessible_ = std::make_unique<AccessibleWindow>(window_);
  // 悬停提示，与 macOS 每个按钮的 toolTip 对应。建不出来不算工具栏失败，只是没有提示。
  INITCOMMONCONTROLSEX controls{sizeof(controls), ICC_BAR_CLASSES};
  if (InitCommonControlsEx(&controls))
    tooltip_ = CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                               TOOLTIPS_CLASSW, nullptr,
                               WS_POPUP | TTS_ALWAYSTIP | TTS_NOPREFIX, CW_USEDEFAULT,
                               CW_USEDEFAULT, CW_USEDEFAULT, CW_USEDEFAULT, window_,
                               nullptr, type.hInstance, nullptr);
}
FloatingToolbarWindow::~FloatingToolbarWindow() {
  hide();
  // 先断开读屏拿着的提供者，再销毁窗口。
  accessible_.reset();
  if (logo_) DestroyIcon(logo_);
  if (tooltip_) DestroyWindow(tooltip_);
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
  if (window_) KillTimer(window_, 1);
  // A hidden toolbar has no pointer over it; leaving these set would show a
  // stale highlight the next time it appears.
  hovered_.reset();
  pressed_.reset();
  shown_.reset();
  shown_character_set_.reset();
  // 提示是工具栏拥有的弹出窗口，工具栏隐藏时它不会跟着消失，正开着的提示要收起来。
  if (tooltip_) SendMessageW(tooltip_, TTM_POP, 0, 0);
  // 隐藏的工具栏没有可读的东西。
  if (accessible_) accessible_->publish({AccessibleContainer::ToolBar, toolbar_accessible_name, {}});
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
    const bool changed = image_dirty_ || !shown_ || !same(shown_->lease, value->lease) ||
                         shown_->chinese != value->chinese ||
                         shown_->chinese_punctuation != value->chinese_punctuation ||
                         shown_->fullwidth != value->fullwidth ||
                         shown_character_set_ != character_set;
    if (!changed && IsWindowVisible(window_)) return;
    shown_ = value;
    shown_character_set_ = character_set;
    update_image();
    if (image_.active() && (!(image_.width > 0) || !(image_.height > 0))) { hide(); return; }
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
    const auto layout = metrics();
    const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    const int width = static_cast<int>(std::ceil(
        (image_.active() ? image_.width : toolbar_window_width(slots().size(), layout)) * unit));
    const int height = static_cast<int>(std::ceil((image_.active() ? image_.height : toolbar_window_height(layout)) * unit));
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
    if (image_.active()) {
      image_.apply_region(window_, unit);
      SetTimer(window_, 1, 33, nullptr);
    }
    sync_tooltips();
    InvalidateRect(window_, nullptr, FALSE);
  } catch (...) { fail(failure_at_stage("refresh", static_cast<uint32_t>(GetLastError()))); }
}
void FloatingToolbarWindow::fail(ComponentFailureSite site) {
  if (!failed_) failure_site_ = site;
  failed_ = true;
  hide();
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
  const auto bar = metrics();
  // 文字按钮（中/英、标点、全半角、简繁，以及字形缺失时的替代字）用中文界面字体的常规字重，字号取工具栏字号的 0.95，与 macOS 按钮标题相同；藏文的 ཀ 换成能画藏文的字体。
  const float text_size = static_cast<float>(bar.icon) * toolbar_glyph_scale * unit;
  auto text_format = [&](const wchar_t *family) {
    auto *created = device_.GetTextFormat(
        family ? family : toolbar_text_font, text_size, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
        DWRITE_WORD_WRAPPING_NO_WRAP);
    if (!created)
      throw std::runtime_error("Toolbar text format unavailable");
    return created;
  };
  // The face every text button uses, created before drawing starts so a missing one fails here rather than half way through a frame.
  (void)text_format(nullptr);
  const auto card = toolbar_card(slots().size(), bar);
  const D2D1_RECT_F card_rect{
      static_cast<float>(card.left) * unit, static_cast<float>(card.top) * unit,
      static_cast<float>(card.right) * unit,
      static_cast<float>(card.bottom) * unit};
  target->BeginDraw();
  // Transparent, not the surface colour: everything outside the bar is either
  // shadow or the desktop showing through.
  target->Clear(D2D1::ColorF(0, 0.0f));
  if (!image_.active()) {
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
  } else {
    image_.draw_background(target, unit);
  }
  const auto value = reader_();
  // 这一帧读不到模式（输入队列正忙，`reader_` 给不出值）或者读到的租约不是画面上这一份时，照上一帧已经画过的那份画，**不要**整块跳过：背景、阴影和圆角在上面已经无条件画好了，跳过内容的那一帧就只剩一块空白的圆角白条，logo、分隔线和按钮全不见（用户报「悬浮窗经常变成一块白的，什么字都没有」）。本函数末尾对读屏树早就是这个办法——「这次没读到模式……读屏的树也留着上一棵」——绘制这一侧漏了同样的兜底。
  const ModePresentation *source =
      (value && shown_ && same(value->lease, shown_->lease)) ? &*value
      : shown_                                               ? &*shown_
                                                             : nullptr;
  if (source) {
    auto *factory = device_.GetDWriteFactory();
    const wchar_t *icon_family = icon_font_family(factory);
    // Each button's two-way mode, in slot order. An absent state means the
    // Server has not reported it, and the icon shows a question mark rather
    // than asserting a mode the user is not actually in.
    const std::optional<bool> states[] = {
        source->chinese,
        source->fullwidth,
        source->chinese_punctuation,
        shown_character_set_,
        std::nullopt, std::nullopt, std::nullopt,
        std::nullopt, std::nullopt, std::nullopt, std::nullopt, std::nullopt};
    const auto active = slots();
    const auto layout = bar;
    // The normal bar's chrome: the product mark at the far left and the hairline after it. An image toolbar draws none of it - its background image is the whole bar, and its alpha decides where the bar drags (ToolbarImageState::drag_at).
    if (!image_.active()) {
      // The product mark. Drawn before the divider and the buttons, and skipped rather than substituted if the icon will not load - a missing mark costs nothing, a placeholder box would look like a bug.
      const auto mark = toolbar_logo(layout);
      if (layout_.show_logo) {
        // 按画出来的实际像素尺寸加载，而不是 Direct2D 的 DIP：msime.ico 带 16 到 256 的各档图像，取对那一档图标才清晰，取错就是重新采样过的模糊图标。
        const double pixels = (mark.right - mark.left) *
                              toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
        if (auto *logo = logo_bitmap(static_cast<int>(std::lround(pixels))))
          target->DrawBitmap(logo,
                             D2D1_RECT_F{static_cast<float>(mark.left) * unit,
                                         static_cast<float>(mark.top) * unit,
                                         static_cast<float>(mark.right) * unit,
                                         static_cast<float>(mark.bottom) * unit},
                             1.0f, D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
      } else {
        // logo 关掉时画握把：两列三行小圆点，尺寸与 macOS 的握把相同（点径 2.4、间距 4.4，再乘用户缩放），颜色取分隔线色。
        const float dot = 1.2f * unit;
        const float pitch = 4.4f * unit;
        const float centre_x = static_cast<float>(mark.left + mark.right) / 2.0f * unit;
        const float centre_y = static_cast<float>(mark.top + mark.bottom) / 2.0f * unit;
        auto *grip = brush(palette_.divider.value_or(palette_.border));
        for (int column = 0; column < 2; ++column)
          for (int row = 0; row < 3; ++row)
            target->FillEllipse(
                D2D1::Ellipse({centre_x + (static_cast<float>(column) - 0.5f) * pitch,
                               centre_y + static_cast<float>(row - 1) * pitch},
                              dot, dot),
                grip);
      }
      // The hairline between the logo and the buttons, 1.2 DIP wide and as tall as a button, as on macOS. There is no accent drag bar beside it: the logo, or the grip in its place, is the drag cue, and every part of the bar that is not a button drags. With no buttons there is nothing to separate, and macOS hides it too.
      if (!active.empty()) {
        const auto line = toolbar_divider(layout);
        target->FillRectangle(D2D1_RECT_F{static_cast<float>(line.left) * unit,
                                          static_cast<float>(line.top) * unit,
                                          static_cast<float>(line.right) * unit,
                                          static_cast<float>(line.bottom) * unit},
                              brush(palette_.divider.value_or(palette_.border)));
      }
    }
    const float button_radius = static_cast<float>(toolbar_button_radius(layout)) * unit;
    for (size_t i = 0; i < active.size(); ++i) {
      const int button = active[i];
      const auto box = this->cell(i);
      const D2D1_RECT_F cell{static_cast<float>(box.left) * unit,
                             static_cast<float>(box.top) * unit,
                             static_cast<float>(box.right) * unit,
                             static_cast<float>(box.bottom) * unit};
      // Hover and press fills, so a button looks like one. On the normal bar that is the whole button square with a quarter of its height as the corner radius, and pressed in the same hover colour, as macOS's MetasequoiaFloatingToolbarButton draws both. An image toolbar keeps its own feedback over the skin's button background: a pill inset 2 DIP a side with a fixed kImagePillRadius corner, and pressed drawn like the card's selected row. A button the shell would have to answer is drawn disabled: no hover fill and the secondary text colour, exactly as the tray draws a row whose capability is missing.
      const bool usable = this->usable(button);
      // An image toolbar sizes each button's glyph from the skin; the normal bar uses the toolbar font size.
      const float icon_size = static_cast<float>(image_.active() ? image_.buttons[i].icon : bar.icon) * unit;
      if (image_.active()) image_.draw(target, image_.buttons[i].background, box, unit);
      const bool down = image_.active() && usable && hovered_ == i && pressed_ == i;
      const auto &glyph_color = !usable ? palette_.number
                                : down  ? palette_.selected_text
                                        : palette_.text;
      if (hovered_ == i && usable) {
        if (image_.active()) {
          const D2D1_ROUNDED_RECT fill{{cell.left + 2.0f * unit, cell.top,
                                        cell.right - 2.0f * unit, cell.bottom},
                                       kImagePillRadius * unit,
                                       kImagePillRadius * unit};
          target->FillRoundedRectangle(
              fill, brush(down ? palette_.selected : palette_.hover));
        } else {
          target->FillRoundedRectangle(D2D1_ROUNDED_RECT{cell, button_radius, button_radius},
                                       brush(palette_.hover));
        }
      }
      // A skin image for the button's current state replaces the glyph.
      if (image_.active() && image_.draw(target, image_.buttons[i].image, box, unit)) continue;
      const auto icon = toolbar_icon(button, states[button], language_);
      // Draw the glyph, at the full font size like macOS's SF Symbols, only when the installed icon font really has it; otherwise the text, which is always readable. The mode buttons have no glyph and are always text.
      IDWriteTextFormat *glyph_format = nullptr;
      if (icon.codepoint && icon_family && icon_font_has(factory, icon_family, icon.codepoint))
        glyph_format = device_.GetTextFormat(
            icon_family, icon_size, DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP);
      const wchar_t text[] = {icon.codepoint, L'\0'};
      // An image toolbar draws its text at the skin's icon size in the shared Chinese UI face (or the Tibetan face the badge needs), as macOS's image canvas draws the system font at iconSizeDip; the normal bar uses the same face at 0.95 of the font size.
      IDWriteTextFormat *cell_format = glyph_format;
      if (!cell_format && image_.active())
        cell_format = device_.GetTextFormat(
            icon.family ? icon.family : toolbar_text_font, icon_size, DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP);
      if (!cell_format)
        cell_format = text_format(icon.family);
      const wchar_t *drawn_text = glyph_format ? text : icon.fallback;
      const auto length = static_cast<UINT32>(wcslen(drawn_text));
      if (!length)
        continue;
      target->DrawText(drawn_text, length, cell_format, cell,
                       brush(glyph_color));
    }
  }
  const HRESULT drawn = target->EndDraw();
  // A composition swap chain only reaches the screen once it is presented.
  const HRESULT presented = SUCCEEDED(drawn) ? device_.Present() : drawn;
  // 设备丢了按 DeviceResources::IsDeviceLost 判断，Present 报的 DXGI_ERROR_DEVICE_REMOVED/RESET 也算，见 CandidateWindow.cpp 的同一处。
  if (msimeui::DeviceResources::IsDeviceLost(presented)) {
    image_.reset_target();
    device_.DiscardTarget();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Toolbar drawing failed");
  if (FAILED(presented))
    throw std::runtime_error("Toolbar presentation failed");
  // 名字按刚画出来的状态取。这次没读到模式（输入队列正忙）或租约不是画面上那个时按钮没有重画，读屏的树也留着上一棵，免得名字在带状态和不带状态之间来回跳、作废读屏刚发出的执行请求。
  if (value && shown_ && same(value->lease, shown_->lease))
    sync_accessibility(value);
}
LRESULT CALLBACK FloatingToolbarWindow::procedure(HWND window, UINT message,
                                                    WPARAM w, LPARAM l) noexcept {
  auto *self = reinterpret_cast<FloatingToolbarWindow *>(GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) { self = static_cast<FloatingToolbarWindow *>(reinterpret_cast<CREATESTRUCTW *>(l)->lpCreateParams);
    self->window_ = window; SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self)); }
  if (!self) return DefWindowProcW(window, message, w, l);
  // 失败后窗口一直隐藏，不再绘制、不再响应指针，免得同一个故障反复触发。
  if (self->failed_) return DefWindowProcW(window, message, w, l);
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
      self->image_.reset_target();
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
    case WM_TIMER:
      if (w == 1 && self->image_.active() && IsWindowVisible(window)) InvalidateRect(window, nullptr, FALSE);
      return 0;
    case WM_ENTERSIZEMOVE: {
      self->moving_ = true;
      RECT rect{};
      self->move_start_.reset();
      if (GetWindowRect(window, &rect))
        self->move_start_ = POINT{rect.left, rect.top};
      return 0;
    }
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
      // 移动循环里的每一帧都记下来，这样拖动途中有 refresh 也按拖到的位置摆，不会把工具栏拽回角上。
      {
        RECT rect{};
        if (GetWindowRect(window, &rect))
          self->dragged_position_ = POINT{rect.left, rect.top};
      }
      return 0;
    case WM_EXITSIZEMOVE:
      // Once, when the drag ends. The move loop raises WM_MOVE for every frame
      // of the drag, and the listener rewrites the whole configuration file, so
      // persisting there rewrote it dozens of times per second on the UI thread.
      self->moving_ = false;
      // 松手时按窗口实际的位置再判一次：系统设置成拖动时只画轮廓的话，窗口要到松手才移动；第一次拖动之前没有记过位置，也要在这里记下，否则生产 Server 永远写不出位置文件。只按了一下没动时保留原来的记录。
      {
        RECT rect{};
        if (self->move_start_ && GetWindowRect(window, &rect)) {
          std::optional<FloatingToolbarPlacement> stored;
          if (self->dragged_position_)
            stored = FloatingToolbarPlacement{self->dragged_position_->x, self->dragged_position_->y};
          const auto ended = floating_toolbar_drag_end(
              stored, FloatingToolbarPlacement{self->move_start_->x, self->move_start_->y},
              FloatingToolbarPlacement{rect.left, rect.top});
          if (ended)
            self->dragged_position_ = POINT{ended->x, ended->y};
        }
        self->move_start_.reset();
      }
      // 拖动时系统的移动循环占着 UI 线程，空闲计时在此期间没有机会重置；按下时记的那次输入到松开时可能已经超过 10 秒，所以松开时再记一次，免得工具栏一放下就隐藏。与 macOS 拖动工具栏会重新计时一致。
      if (self->activity_action_)
        self->activity_action_();
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
      if (self->activity_action_)
        self->activity_action_();
      // Every part of the bar that is not a button drags, as macOS's movableByWindowBackground does: the logo or the grip, the divider, the gaps between buttons, the padding above and below them and the trailing margin. An image toolbar drags wherever its background image is opaque and no button sits (ToolbarImageState::drag_at). A press on a button never does - treating the whole window as a caption meant a press on a button entered the system move loop, and the click below only ran for whatever button-up survived it.
      if (self->drag_at_pixel(GET_X_LPARAM(l), GET_Y_LPARAM(l))) {
        ReleaseCapture();
        SendMessageW(window, WM_NCLBUTTONDOWN, HTCAPTION, 0);
        return 0;
      }
      // Pressing a button shows it pressed until the release is handled.
      self->pressed_ = self->button_at_pixel(GET_X_LPARAM(l), GET_Y_LPARAM(l));
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
      // Show the move cursor over the logo or the grip only, as macOS shows the open hand over its logo view, so the buttons keep the ordinary arrow and the logo advertises what the bar does. The rest of the bar drags too, without saying so. An image toolbar has no logo, so it shows the move cursor over every part that drags.
      if (LOWORD(l) == HTCLIENT) {
        POINT cursor{};
        RECT bounds{};
        if (GetCursorPos(&cursor) && ScreenToClient(window, &cursor) &&
            GetClientRect(window, &bounds) &&
            (self->image_.active()
                 ? self->drag_at_pixel(cursor.x, cursor.y)
                 : toolbar_logo_at_pixel(cursor.x, cursor.y, GetDpiForWindow(window),
                                         self->scale_, self->metrics()))) {
          SetCursor(LoadCursorW(nullptr, IDC_SIZEALL));
          return TRUE;
        }
      }
      break;
    case WM_MOUSEMOVE: {
      // Hover feedback needs to know where the pointer is; without tracking,
      // the buttons gave no sign that they were buttons at all.
      const int x = GET_X_LPARAM(l);
      const auto hovered = self->button_at_pixel(x, GET_Y_LPARAM(l));
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
      const auto position_at = self->button_at_pixel(x, GET_Y_LPARAM(l));
      const bool valid_click = toolbar_release(
          self->pressed_, position_at,
          value && self->pressed_lease_ && self->shown_ &&
              same(value->lease, *self->pressed_lease_) &&
              same(value->lease, self->shown_->lease));
      self->pressed_lease_.reset();
      if (valid_click)
        self->run(*position_at, *value, x);
      return 0;
    }
    case WM_RBUTTONDOWN:
      if (self->activity_action_)
        self->activity_action_();
      return 0;
    case WM_RBUTTONUP: {
      // 卡片上任何位置点右键都打开实用菜单。macOS 只挂在设置按钮上；这里不限按钮，设置按钮被关掉时菜单也够得着，隐藏工具栏这一项更不能因此丢掉。
      const int x = GET_X_LPARAM(l);
      if (self->context_menu_action_ &&
          self->card_at_pixel(x, GET_Y_LPARAM(l)))
        self->context_menu_action_(self->menu_anchor(std::nullopt, x));
      return 0;
    }
    case WM_GETOBJECT:
      if (self->accessible_)
        if (const auto answer = self->accessible_->answer(w, l))
          return *answer;
      break;
    case accessible_invoke_message:
      self->invoke_accessible(static_cast<int>(w), l);
      return 0;
    case WM_DESTROY:
      if (self->accessible_)
        self->accessible_->disconnect();
      break;
    case WM_NOTIFY: {
      auto *header = reinterpret_cast<NMHDR *>(l);
      if (header && self->tooltip_ && header->hwndFrom == self->tooltip_ &&
          header->code == TTN_GETDISPINFOW) {
        auto *info = reinterpret_cast<NMTTDISPINFOW *>(l);
        self->tooltip_text_ = self->tooltip_text(static_cast<size_t>(header->idFrom));
        info->lpszText = self->tooltip_text_.data();
        return 0;
      }
      break;
    }
    // Let DefWindowProc handle the caption message sent by a press on the bar.
    // Sending that same synchronous message again here recurses indefinitely.
  }} catch (...) { self->fail(failure_in_message(message, static_cast<uint32_t>(GetLastError()))); return 0; }
  return DefWindowProcW(window, message, w, l);
}
} // namespace msime::windows

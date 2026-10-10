#include "InputModeHudWindow.h"
#include "ServerResources.h"
#include "ToolbarCoordinates.h"
#include "WindowShadow.h"
#include <oleauto.h>
#include <uiautomation.h>
#include <cmath>
#include <cwchar>
#include <stdexcept>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t kClassName[] = L"MSIME.Client.InputModeHud" MSIME_EDITION_NAME_SUFFIX;
constexpr UINT_PTR kHideTimer = 0x4601;
// 和候选窗一样只在这次界面操作里切到每显示器 DPI v2，结束时恢复调用方的线程设置；窗口在整个生命期里保持 v2，坐标都是物理像素，和 TSF 报来的光标锚点一致。
struct DpiScope {
  DPI_AWARENESS_CONTEXT previous;
  DpiScope()
      : previous(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)) {
    if (!previous)
      throw std::runtime_error("Per-monitor DPI unavailable");
  }
  ~DpiScope() { SetThreadDpiAwarenessContext(previous); }
};
long scaled(double value, double unit) { return static_cast<long>(std::lround(value * unit)); }

// UiaRaiseNotificationEvent 的两个枚举值（NotificationKind_ActionCompleted、NotificationProcessing_MostRecent）。按数值写在这里，不依赖 SDK 头文件按 NTDDI_VERSION 是否声明它们。
constexpr int kNotificationKindActionCompleted = 2;
constexpr int kNotificationProcessingMostRecent = 3;

// 把这次切换念给读屏软件，对应 macOS InputModeHUDPanel 的 NSAccessibilityAnnouncementRequestedNotification。UiaRaiseNotificationEvent 从 Windows 10 1709 起才有，而 Server 的目标版本是 1703，所以运行时从已加载的 uiautomationcore.dll 里取，取不到就不播报；直接导入会让 1703 上的 Server 整个起不来。没有读屏软件在听时什么也不做。提供者用系统给这个窗口的宿主提供者，徽标没有自己的元素树。
void announce_input_mode(HWND window, bool chinese) noexcept {
  if (!UiaClientsAreListening())
    return;
  using RaiseNotification = HRESULT(WINAPI *)(IRawElementProviderSimple *, int, int, BSTR, BSTR);
  static const RaiseNotification raise = []() -> RaiseNotification {
    const HMODULE core = GetModuleHandleW(L"uiautomationcore.dll");
    // 经 void* 转换：GetProcAddress 返回通用的 FARPROC，直接转成真实签名会被 -Wcast-function-type 拒绝。
    return core ? reinterpret_cast<RaiseNotification>(
                      reinterpret_cast<void *>(GetProcAddress(core, "UiaRaiseNotificationEvent")))
                : nullptr;
  }();
  if (!raise)
    return;
  IRawElementProviderSimple *provider = nullptr;
  if (FAILED(UiaHostProviderFromHwnd(window, &provider)) || !provider)
    return;
  BSTR text = SysAllocString(input_mode_hud_announcement(chinese));
  BSTR activity = SysAllocString(L"MSIME.InputModeHud");
  if (text && activity)
    (void)raise(provider, kNotificationKindActionCompleted, kNotificationProcessingMostRecent, text, activity);
  SysFreeString(text);
  SysFreeString(activity);
  provider->Release();
}
} // namespace

InputModeHudWindow::InputModeHudWindow() {
  DpiScope dpi_scope;
  WNDCLASSEXW type{};
  type.cbSize = sizeof(type);
  type.lpfnWndProc = procedure;
  type.hInstance = GetModuleHandleW(nullptr);
  type.hCursor = LoadCursorW(nullptr, MAKEINTRESOURCEW(32512));
  type.lpszClassName = kClassName;
  if (!RegisterClassExW(&type) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS)
    throw std::runtime_error("Input mode HUD class unavailable");
  // 不激活、不进任务栏和 Alt+Tab、置顶；WS_EX_TRANSPARENT 让鼠标点击穿过去，徽标只是看的。
  window_ = CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_TRANSPARENT,
                            kClassName, L"MSIME input mode", WS_POPUP, 0, 0, 1, 1, nullptr,
                            nullptr, type.hInstance, this);
  if (!window_)
    throw std::runtime_error("Input mode HUD window unavailable");
  // 可执行文件里没有图标资源时（单测链接的程序）不给 logo 留位置，和 macOS 取不到图标时一样。
  logo_available_ = FindResourceW(type.hInstance, MAKEINTRESOURCEW(IDI_MSIME_LOGO), RT_GROUP_ICON) != nullptr;
}
InputModeHudWindow::~InputModeHudWindow() {
  hide();
  if (logo_)
    DestroyIcon(logo_);
  if (window_)
    DestroyWindow(window_);
}
InputModeHudWindow::Apartment::Apartment() {
  const HRESULT entered = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
  if (FAILED(entered) && entered != RPC_E_CHANGED_MODE)
    throw std::runtime_error("Input mode HUD apartment unavailable");
  owned = entered != RPC_E_CHANGED_MODE;
}
InputModeHudWindow::Apartment::~Apartment() {
  if (owned)
    CoUninitialize();
}
void InputModeHudWindow::set_palette(CandidatePalette palette) {
  palette_ = std::move(palette);
  if (window_ && IsWindowVisible(window_))
    InvalidateRect(window_, nullptr, FALSE);
}
void InputModeHudWindow::set_settings(const FloatingToolbarSettings &settings) {
  if (!settings.valid())
    return;
  scale_ = static_cast<double>(settings.scale_percent) / 100.0;
  font_size_ = static_cast<double>(settings.font_size);
}
ID2D1Bitmap *InputModeHudWindow::logo_bitmap(int pixels) {
  if (pixels <= 0)
    return nullptr;
  if (!logo_ || logo_pixels_ != pixels) {
    // 按画出来的像素尺寸取图标里对应的那一帧，和工具栏一样不用 LR_SHARED，免得拿到系统缓存的标准尺寸再缩放。
    const HANDLE loaded = LoadImageW(GetModuleHandleW(nullptr), MAKEINTRESOURCEW(IDI_MSIME_LOGO),
                                     IMAGE_ICON, pixels, pixels, LR_DEFAULTCOLOR);
    if (!loaded)
      return nullptr;
    if (logo_)
      DestroyIcon(logo_);
    logo_ = static_cast<HICON>(loaded);
    logo_pixels_ = pixels;
  }
  return device_.GetBitmapFromIcon(logo_, L"icon:input-mode-hud-logo:" + std::to_wstring(pixels));
}
void InputModeHudWindow::hide() {
  if (!window_)
    return;
  KillTimer(window_, kHideTimer);
  ShowWindow(window_, SW_HIDE);
}
std::optional<HudRect> InputModeHudWindow::system_caret(HWND foreground) {
  if (!foreground)
    return std::nullopt;
  GUITHREADINFO info{};
  info.cbSize = sizeof(info);
  if (!GetGUIThreadInfo(GetWindowThreadProcessId(foreground, nullptr), &info) || !info.hwndCaret)
    return std::nullopt;
  if (GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(info.hwndCaret)) !=
      DPI_AWARENESS_PER_MONITOR_AWARE)
    return std::nullopt;
  // 换算放在每显示器 DPI v2 的线程上下文里做，得到的才是物理像素。
  const DPI_AWARENESS_CONTEXT previous =
      SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  if (!previous)
    return std::nullopt;
  POINT top_left{info.rcCaret.left, info.rcCaret.top};
  POINT bottom_right{info.rcCaret.right, info.rcCaret.bottom};
  const bool mapped = ClientToScreen(info.hwndCaret, &top_left) && ClientToScreen(info.hwndCaret, &bottom_right);
  SetThreadDpiAwarenessContext(previous);
  if (!mapped)
    return std::nullopt;
  const HudRect caret{top_left.x, top_left.y, bottom_right.x, bottom_right.y};
  return input_mode_hud_usable_caret(caret) ? std::optional<HudRect>(caret) : std::nullopt;
}
void InputModeHudWindow::fail(ComponentFailureSite site) {
  if (!failed_)
    failure_site_ = site;
  failed_ = true;
  hide();
}
void InputModeHudWindow::show(bool chinese, std::optional<HudRect> caret, HWND foreground) {
  if (failed_ || !window_)
    return;
  try {
    DpiScope dpi_scope;
    chinese_ = chinese;
    const bool usable = caret && input_mode_hud_usable_caret(*caret);
    const HMONITOR monitor =
        usable ? MonitorFromPoint(POINT{(caret->left + caret->right) / 2, (caret->top + caret->bottom) / 2},
                                  MONITOR_DEFAULTTONEAREST)
               : MonitorFromWindow(foreground, MONITOR_DEFAULTTOPRIMARY);
    MONITORINFO info{};
    info.cbSize = sizeof(info);
    if (!GetMonitorInfoW(monitor, &info))
      throw std::runtime_error("Input mode HUD monitor unavailable");
    // 先挪到目标显示器上，窗口的 DPI 才是那块显示器的，尺寸按它算。已经在这块显示器上时不挪：连续切换时徽标正显示着，先挪到工作区左上角会在那里闪一下。
    if (MonitorFromWindow(window_, MONITOR_DEFAULTTONULL) != monitor &&
        !SetWindowPos(window_, nullptr, info.rcWork.left, info.rcWork.top, 0, 0,
                      SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE))
      throw std::runtime_error("Input mode HUD move failed");
    const double unit = toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    const auto metrics = input_mode_hud_metrics(font_size_, logo_available_);
    InputModeHudPlacementInput placement;
    if (usable)
      placement.caret = *caret;
    placement.work = {info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom};
    placement.card_width = scaled(metrics.card_width(), unit);
    placement.card_height = scaled(metrics.height, unit);
    placement.shadow_left = scaled(metrics.shadow.left, unit);
    placement.shadow_top = scaled(metrics.shadow.top, unit);
    const double dpi_unit = toolbar_pixel_unit(GetDpiForWindow(window_), 1.0);
    placement.margin = scaled(8.0, dpi_unit);
    placement.gap = scaled(10.0, dpi_unit);
    const auto placed = input_mode_hud_placement(placement);
    const int width = static_cast<int>(std::ceil(metrics.window_width() * unit));
    const int height = static_cast<int>(std::ceil(metrics.window_height() * unit));
    KillTimer(window_, kHideTimer);
    if (!SetWindowPos(window_, HWND_TOPMOST, static_cast<int>(placed.x), static_cast<int>(placed.y),
                      width, height, SWP_NOACTIVATE | SWP_SHOWWINDOW))
      throw std::runtime_error("Input mode HUD positioning failed");
    InvalidateRect(window_, nullptr, FALSE);
    if (!SetTimer(window_, kHideTimer, input_mode_hud_visible_ms, nullptr))
      throw std::runtime_error("Input mode HUD timer unavailable");
    announce_input_mode(window_, chinese);
  } catch (...) {
    fail(failure_at_stage("show", static_cast<uint32_t>(GetLastError())));
  }
}
void InputModeHudWindow::paint() {
  PAINTSTRUCT state{};
  const HDC dc = BeginPaint(window_, &state);
  if (!dc)
    return;
  struct End {
    HWND window;
    PAINTSTRUCT &state;
    ~End() { EndPaint(window, &state); }
  } end{window_, state};
  // 合成表面而不是窗口目标：阴影落在卡片外面，窗口要按像素带透明度。
  if (!device_.EnsureForComposition(window_))
    throw std::runtime_error("Input mode HUD device unavailable");
  auto *target = device_.GetRenderTarget();
  if (!target)
    throw std::runtime_error("Input mode HUD render target unavailable");
  auto brush = [&](const CandidateColor &color) {
    auto *created = device_.GetSolidColorBrush(D2D1::ColorF(color.r, color.g, color.b, color.a));
    if (!created)
      throw std::runtime_error("Input mode HUD brush unavailable");
    return created;
  };
  const float unit = static_cast<float>(scale_);
  const auto metrics = input_mode_hud_metrics(font_size_, logo_available_);
  const D2D1_RECT_F card{static_cast<float>(metrics.shadow.left) * unit,
                         static_cast<float>(metrics.shadow.top) * unit,
                         static_cast<float>(metrics.shadow.left + metrics.card_width()) * unit,
                         static_cast<float>(metrics.shadow.top + metrics.height) * unit};
  target->BeginDraw();
  target->Clear(D2D1::ColorF(0, 0.0f));
  draw_window_shadow(target, card, palette_.radius * unit, static_cast<float>(metrics.shadow.scale));
  const float inset = palette_.border_width * unit / 2.0f;
  const D2D1_ROUNDED_RECT body{{card.left + inset, card.top + inset, card.right - inset, card.bottom - inset},
                               palette_.radius * unit, palette_.radius * unit};
  target->FillRoundedRectangle(body, brush(palette_.surface));
  target->DrawRoundedRectangle(body, brush(palette_.border), palette_.border_width * unit);
  float left = card.left + static_cast<float>(metrics.inset) * unit;
  if (metrics.logo > 0.0) {
    const float side = static_cast<float>(metrics.logo) * unit;
    const float top = (card.top + card.bottom - side) / 2.0f;
    // 以真实像素取图标帧：msime.ico 带 16 到 256 的各档，取对尺寸才清晰。
    const double pixels = metrics.logo * toolbar_pixel_unit(GetDpiForWindow(window_), scale_);
    if (auto *logo = logo_bitmap(static_cast<int>(std::lround(pixels))))
      target->DrawBitmap(logo, D2D1_RECT_F{left, top, left + side, top + side}, 1.0f,
                         D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    left += side + static_cast<float>(metrics.spacing) * unit;
  }
  auto *format = device_.GetTextFormat(L"Microsoft YaHei UI", static_cast<float>(metrics.font * 0.95) * unit,
                                       DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_TEXT_ALIGNMENT_CENTER,
                                       DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP);
  if (!format)
    throw std::runtime_error("Input mode HUD text format unavailable");
  const wchar_t *text = input_mode_hud_text(chinese_);
  const D2D1_RECT_F glyph{left, card.top, left + static_cast<float>(metrics.glyph) * unit, card.bottom};
  target->DrawText(text, static_cast<UINT32>(wcslen(text)), format, glyph, brush(palette_.text));
  const HRESULT drawn = target->EndDraw();
  if (SUCCEEDED(drawn) && FAILED(device_.Present()))
    throw std::runtime_error("Input mode HUD presentation failed");
  // 有符号的 HRESULT 和某些 SDK、MinGW 里无符号的宏比较，见 CandidateWindow.cpp 的同一处。
  if (drawn == static_cast<HRESULT>(D2DERR_RECREATE_TARGET)) {
    device_.DiscardTarget();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Input mode HUD drawing failed");
}
LRESULT CALLBACK InputModeHudWindow::procedure(HWND window, UINT message, WPARAM w, LPARAM l) noexcept {
  auto *self = reinterpret_cast<InputModeHudWindow *>(GetWindowLongPtrW(window, GWLP_USERDATA));
  if (message == WM_NCCREATE) {
    self = static_cast<InputModeHudWindow *>(reinterpret_cast<CREATESTRUCTW *>(l)->lpCreateParams);
    self->window_ = window;
    SetWindowLongPtrW(window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
  }
  if (!self || self->failed_)
    return DefWindowProcW(window, message, w, l);
  try {
    switch (message) {
    case WM_MOUSEACTIVATE:
      return MA_NOACTIVATE;
    case WM_NCHITTEST:
      return HTTRANSPARENT;
    case WM_ERASEBKGND:
      return 1;
    case WM_TIMER:
      if (w == kHideTimer) {
        self->hide();
        return 0;
      }
      break;
    case WM_DISPLAYCHANGE:
    case WM_SETTINGCHANGE:
    case WM_DWMCOMPOSITIONCHANGED:
    case WM_DPICHANGED:
      // 换了显示器设置或 DPI：丢掉旧的合成表面，下次显示时重建；正在显示的这次直接收起。
      self->device_.DiscardTarget();
      self->hide();
      return 0;
    case WM_PAINT:
      self->paint();
      return 0;
    }
  } catch (...) {
    self->fail(failure_in_message(message, static_cast<uint32_t>(GetLastError())));
    return 0;
  }
  return DefWindowProcW(window, message, w, l);
}
} // namespace msime::windows

#include "InputModeHudWindow.h"
#include "../../compat/WindowsDpi.h"
#include "ServerResources.h"
#include "ToolbarCoordinates.h"
#include "WindowShadow.h"
#include <oleauto.h>
#include <uiautomation.h>
#include <wrl/client.h>
#include <cmath>
#include <cwchar>
#include <stdexcept>
#include "../../../../shared/contracts/msime_edition.h"

namespace msime::windows {
namespace {
constexpr wchar_t kClassName[] = L"MSIME.Client.InputModeHud" MSIME_EDITION_NAME_SUFFIX;
constexpr UINT_PTR kHideTimer = 0x4601;
// 淡出的帧计时器，按 input_mode_hud_fade_frame_ms 重画，不透明度降到 0 时隐藏。
constexpr UINT_PTR kFadeTimer = 0x4602;
// 系统支持时只在这次界面操作中切换线程 DPI 并恢复；旧系统沿用宿主启动时设置的系统 DPI。
using DpiScope = msime::windows::compat::ThreadDpiScope;
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
void InputModeHudWindow::set_show_logo(bool show) {
  if (show_logo_ == show)
    return;
  show_logo_ = show;
  // 窗口按旧宽度摆好了，换宽度重画会裁掉或留空，直接收起。
  if (window_ && IsWindowVisible(window_))
    hide();
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
  KillTimer(window_, kFadeTimer);
  opacity_ = 1.0f;
  ShowWindow(window_, SW_HIDE);
}
std::optional<HudRect> InputModeHudWindow::system_caret(HWND foreground) {
  if (!foreground)
    return std::nullopt;
  GUITHREADINFO info{};
  info.cbSize = sizeof(info);
  if (!GetGUIThreadInfo(GetWindowThreadProcessId(foreground, nullptr), &info) || !info.hwndCaret)
    return std::nullopt;
  if (msime::windows::compat::awareness(msime::windows::compat::window_context(info.hwndCaret)) !=
      DPI_AWARENESS_PER_MONITOR_AWARE)
    return std::nullopt;
  // 换算放在每显示器 DPI v2 的线程上下文里做，得到的才是物理像素。
  const DPI_AWARENESS_CONTEXT previous =
      msime::windows::compat::thread_context(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
  if (!previous)
    return std::nullopt;
  POINT top_left{info.rcCaret.left, info.rcCaret.top};
  POINT bottom_right{info.rcCaret.right, info.rcCaret.bottom};
  const bool mapped = ClientToScreen(info.hwndCaret, &top_left) && ClientToScreen(info.hwndCaret, &bottom_right);
  msime::windows::compat::thread_context(previous);
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
    // 只按显示器 DPI 换算成像素，不乘工具栏的缩放。
    const double unit = toolbar_pixel_unit(msime::windows::compat::window_dpi(window_), 1.0);
    const auto metrics = input_mode_hud_metrics(show_logo_ && logo_available_);
    InputModeHudPlacementInput placement;
    if (usable)
      placement.caret = *caret;
    placement.work = {info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom};
    placement.card_width = scaled(metrics.card_width(), unit);
    placement.card_height = scaled(metrics.height, unit);
    placement.shadow_left = scaled(metrics.shadow.left, unit);
    placement.shadow_top = scaled(metrics.shadow.top, unit);
    placement.margin = scaled(8.0, unit);
    placement.gap = scaled(10.0, unit);
    const auto placed = input_mode_hud_placement(placement);
    const int width = static_cast<int>(std::ceil(metrics.window_width() * unit));
    const int height = static_cast<int>(std::ceil(metrics.window_height() * unit));
    // 正在淡出的上一次徽标直接回到完全显示，重新计时。
    KillTimer(window_, kHideTimer);
    KillTimer(window_, kFadeTimer);
    opacity_ = 1.0f;
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
  // 固定尺寸：Direct2D 的 DIP 已经带上了显示器 DPI，这里不再乘工具栏的缩放。圆角和描边也是徽标自己的（8 和 1），只有颜色取工具栏的调色板。
  const auto metrics = input_mode_hud_metrics(show_logo_ && logo_available_);
  const D2D1_RECT_F card{static_cast<float>(metrics.shadow.left), static_cast<float>(metrics.shadow.top),
                         static_cast<float>(metrics.shadow.left + metrics.card_width()),
                         static_cast<float>(metrics.shadow.top + metrics.height)};
  const auto radius = static_cast<float>(metrics.radius);
  const auto border = static_cast<float>(metrics.border);
  // 常规字重，与 macOS 的 18pt 系统字相同。
  auto *format = device_.GetTextFormat(L"Microsoft YaHei UI", static_cast<float>(metrics.font),
                                       DWRITE_FONT_WEIGHT_NORMAL, DWRITE_TEXT_ALIGNMENT_CENTER,
                                       DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_WORD_WRAPPING_NO_WRAP);
  if (!format)
    throw std::runtime_error("Input mode HUD text format unavailable");
  target->BeginDraw();
  target->Clear(D2D1::ColorF(0, 0.0f));
  // 淡出时整张徽标连同阴影放进一个带不透明度的图层里画。
  Microsoft::WRL::ComPtr<ID2D1Layer> fade;
  if (opacity_ < 1.0f && SUCCEEDED(target->CreateLayer(nullptr, fade.GetAddressOf())))
    target->PushLayer(D2D1::LayerParameters(D2D1::InfiniteRect(), nullptr, D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
                                            D2D1::IdentityMatrix(), opacity_),
                      fade.Get());
  draw_window_shadow(target, card, radius, static_cast<float>(metrics.shadow.scale));
  const float inset = border / 2.0f;
  const D2D1_ROUNDED_RECT body{{card.left + inset, card.top + inset, card.right - inset, card.bottom - inset},
                               radius, radius};
  target->FillRoundedRectangle(body, brush(palette_.surface));
  target->DrawRoundedRectangle(body, brush(palette_.border), border);
  float left = card.left + static_cast<float>(metrics.inset);
  if (metrics.logo > 0.0) {
    const float side = static_cast<float>(metrics.logo);
    const float top = (card.top + card.bottom - side) / 2.0f;
    // 以真实像素取图标帧：msime.ico 带 16 到 256 的各档，取对尺寸才清晰。
    const double pixels = metrics.logo * toolbar_pixel_unit(msime::windows::compat::window_dpi(window_), 1.0);
    if (auto *logo = logo_bitmap(static_cast<int>(std::lround(pixels))))
      target->DrawBitmap(logo, D2D1_RECT_F{left, top, left + side, top + side}, 1.0f,
                         D2D1_BITMAP_INTERPOLATION_MODE_LINEAR);
    left += side + static_cast<float>(metrics.spacing);
  }
  const wchar_t *text = input_mode_hud_text(chinese_);
  const D2D1_RECT_F glyph{left, card.top, left + static_cast<float>(metrics.glyph), card.bottom};
  target->DrawText(text, static_cast<UINT32>(wcslen(text)), format, glyph, brush(palette_.text));
  if (fade)
    target->PopLayer();
  const HRESULT drawn = target->EndDraw();
  const HRESULT presented = SUCCEEDED(drawn) ? device_.Present() : drawn;
  // 设备丢了按 DeviceResources::IsDeviceLost 判断，Present 报的 DXGI_ERROR_DEVICE_REMOVED/RESET 也算，见 CandidateWindow.cpp 的同一处。
  if (msimeui::DeviceResources::IsDeviceLost(presented)) {
    device_.DiscardTarget();
    InvalidateRect(window_, nullptr, FALSE);
    return;
  }
  if (FAILED(drawn))
    throw std::runtime_error("Input mode HUD drawing failed");
  if (FAILED(presented))
    throw std::runtime_error("Input mode HUD presentation failed");
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
      // 完全显示的 0.6 秒到了：开始淡出，与 macOS 的 fadeOut 一样。帧计时器建不起来时直接隐藏。
      if (w == kHideTimer) {
        KillTimer(window, kHideTimer);
        self->fade_started_ = GetTickCount64();
        if (!SetTimer(window, kFadeTimer, input_mode_hud_fade_frame_ms, nullptr))
          self->hide();
        return 0;
      }
      if (w == kFadeTimer) {
        self->opacity_ = input_mode_hud_fade_opacity(GetTickCount64() - self->fade_started_);
        if (self->opacity_ <= 0.0f)
          self->hide();
        else
          InvalidateRect(window, nullptr, FALSE);
        return 0;
      }
      break;
    case WM_DISPLAYCHANGE:
    case WM_SETTINGCHANGE:
    case WM_DWMCOMPOSITIONCHANGED:
    case WM_DPICHANGED:
      // 换了显示器设置或 DPI：丢掉旧的合成表面，下次显示时重建；正在显示或淡出的这次直接收起。
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

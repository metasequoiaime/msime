#pragma once
#include "CandidatePalette.h"
#include "ComponentFailure.h"
#include "FloatingToolbarSettings.h"
#include "InputModeHudLayout.h"
#include <optional>
// 先包含 windows.h：它的 DrawText 宏要在 Direct2D 的声明之前生效。
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
// 中英文切换提示：切换中英文后在光标旁短暂显示「中」或「英」的小徽标，对应 macOS 的 MSIMEInputModeHUDPanel。不抢焦点、不接收鼠标，配色取悬浮工具栏的调色板，尺寸取工具栏的字号和缩放，0.6 秒后自己隐藏。只在 Server 的界面线程上使用。
class InputModeHudWindow final {
public:
  InputModeHudWindow();
  ~InputModeHudWindow();
  InputModeHudWindow(const InputModeHudWindow &) = delete;
  InputModeHudWindow &operator=(const InputModeHudWindow &) = delete;
  // 与悬浮工具栏同一份调色板，主题和皮肤换了跟着换。
  void set_palette(CandidatePalette palette);
  // 工具栏的字号和缩放；不合法的设置不改动现有尺寸。
  void set_settings(const FloatingToolbarSettings &settings);
  // 在光标旁显示 `chinese` 对应的字。`caret` 是光标所在行的屏幕矩形（物理像素），没有时用 `foreground` 所在显示器的居中位置。
  void show(bool chinese, std::optional<HudRect> caret, HWND foreground);
  void hide();
  // 前台线程的系统光标（GetGUIThreadInfo），换算成屏幕上的物理像素。只认每显示器 DPI 感知的窗口：其他窗口的光标坐标是缩放前的逻辑坐标，换算不可靠，宁可退回别的位置。
  static std::optional<HudRect> system_caret(HWND foreground);
  // 失败后保持隐藏，之后的调用都不再做事；Server 去掉这个提示继续运行。
  bool failed() const { return failed_; }
  const std::optional<ComponentFailureSite> &failure_site() const { return failure_site_; }

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  void paint();
  void fail(ComponentFailureSite site);
  ID2D1Bitmap *logo_bitmap(int pixels);
  // 取 Direct2D 的图像工厂要用 COM，本线程自己进入一个单线程套间。
  struct Apartment {
    Apartment();
    ~Apartment();
    Apartment(const Apartment &) = delete;
    Apartment &operator=(const Apartment &) = delete;
    bool owned = false;
  } apartment_;
  msimeui::DeviceResources device_;
  CandidatePalette palette_;
  HWND window_ = nullptr;
  HICON logo_ = nullptr;
  int logo_pixels_ = 0;
  bool logo_available_ = false;
  bool chinese_ = true;
  double scale_ = 1.0;
  double font_size_ = 24.0;
  bool failed_ = false;
  std::optional<ComponentFailureSite> failure_site_;
};
} // namespace msime::windows

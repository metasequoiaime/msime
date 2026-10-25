#pragma once

#include <X11/Xlib.h>
#include <cairo/cairo.h>

#include <cstdint>
#include <functional>
#include <vector>

namespace msime::linux_host {

// 打开一条浮层用的 X 连接（name 为空时用 DISPLAY），连不上时返回空指针。连接断开时（X 服务器退出，Wayland 会话里 Xwayland 崩溃或被合成器重启）Xlib 默认会结束整个进程，对 IBus 引擎和 fcitx5 守护进程来说就是丢掉全部输入；这里给这条连接装上不退出的处理（libX11 1.7 起的 XSetIOErrorExitHandler，另加一个只对这些连接返回、别的连接照旧交给之前处理的进程级 I/O 错误处理）：断开后 *lost 变成 true，之后这条连接上的 Xlib 调用都是空操作，调用方在自己的调用栈之外关掉它、下次要画时再连。lost 指向的变量要活得比连接久。更早的 libX11 没有这个接口（legacy 包的 Debian 10 是 1.6.7），那里断开仍会结束进程，与以前相同。
// 不支持 root overlay 的 xwayland-satellite 同样返回空指针，避免浮层变成顶层并抢焦点。
Display *open_overlay_display(const char *name, bool *lost);
// 关掉 open_overlay_display 打开的连接。
void close_overlay_display(Display *display);

// 一块由输入法自己画的 override-redirect 窗口：双拼键位图、悬浮工具栏、它弹出的菜单和按钮提示都是这一种。不进任务栏、不被窗口管理器摆布、不抢焦点；拿得到 32 位 ARGB visual 时窗口带透明，圆角和阴影由画法自己画。
//
// 合成管理器可以随时出现或离开，所以每次画都重新判断：没有合成器时 ARGB 窗口的透明像素会显示成黑色，这时按画出来的 alpha 设一个二值的外形（Shape 扩展），半透明的边缘合在 opaque_fallback 上，与候选窗的原生画布（NativeCanvasX11）同一种退路。
//
// 连接（Display）归调用方：工具栏和它的菜单、提示共用一条连接，事件也由调用方分发。
class OverlayX11Window {
 public:
  // res_name / res_class 写进 WM_CLASS，测试和合成器的规则靠它认窗口；window_type 是 _NET_WM_WINDOW_TYPE 的原子名（例如 "_NET_WM_WINDOW_TYPE_TOOLTIP"）。accepts_input 为 false 时输入形状为空，点击穿透到下面的窗口。
  OverlayX11Window(Display *display, const char *res_name, const char *res_class, const char *window_type,
                   bool accepts_input);
  ~OverlayX11Window();
  OverlayX11Window(const OverlayX11Window &) = delete;
  OverlayX11Window &operator=(const OverlayX11Window &) = delete;

  bool valid() const { return window_ != 0; }
  Window id() const { return window_; }
  bool visible() const { return visible_; }
  int x() const { return x_; }
  int y() const { return y_; }
  int width() const { return width_; }
  int height() const { return height_; }

  // 根窗口坐标里的位置和设备像素尺寸。尺寸变了下一次 paint 按新尺寸画。
  void place(int x, int y, int width, int height);
  void move(int x, int y);
  // 先 place 再 show，窗口不会在上一次的位置上闪一下。
  void show();
  // 已经在屏幕上时抬到所有兄弟窗口之上。
  void raise();
  void hide();
  // 用 draw 画整块窗口：上下文已按 scale 缩放，draw 用逻辑像素画，先清成全透明。opaque_fallback 是没有合成器时半透明像素合上去的底色（0xRRGGBB）。
  void paint(double scale, std::uint32_t opaque_fallback, const std::function<void(cairo_t *)> &draw);
  // 接受点击的区域（设备像素，相对窗口左上角）；空表示整个窗口都接受。只对 accepts_input 的窗口有意义。
  void set_input_region(const std::vector<XRectangle> &rectangles);
  // 有没有合成管理器（EWMH 的 _NET_WM_CM_S<屏幕号> 选区有主人）。
  bool composited() const;

 private:
  Display *display_ = nullptr;
  Window window_ = 0;
  Visual *visual_ = nullptr;
  Colormap colormap_ = 0;
  bool argb_ = false;
  bool accepts_input_ = false;
  bool shape_ = false;
  // 上一次设了二值外形，合成器回来后要把外形清掉。
  bool bounding_shaped_ = false;
  bool visible_ = false;
  int x_ = 0;
  int y_ = 0;
  int width_ = 1;
  int height_ = 1;
};

}  // namespace msime::linux_host

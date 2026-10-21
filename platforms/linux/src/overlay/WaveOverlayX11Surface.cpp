#include "WaveOverlayX11Surface.h"

#include "WaveOverlayPlacement.h"
#include "WaveOverlayX11Placement.h"

#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/Xfixes.h>
#include <X11/extensions/shape.h>
#include <cairo/cairo-xlib.h>

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <string>
#include <vector>

namespace msime::linux_host {
namespace {

// 每次音量变化和每帧动画都会运行 `update()`，所以尺寸不变时最多按这个频率重读显示器、工作区和缩放；`show()` 和布局变化总会重读。
constexpr auto kPlacementRefreshInterval = std::chrono::milliseconds(500);

// 把圆角面板拆成一条条水平带，供没有合成管理器、无法显示半透明圆角时的 Shape 扩展使用。
std::vector<XRectangle> rounded_bands(int width, int height, double radius) {
  std::vector<XRectangle> bands;
  radius = std::clamp(radius, 0.0, std::min(width, height) / 2.0);
  int start = 0;
  int current = -1;
  const auto inset_at = [height, radius](int y) {
    double dy = 0;
    if (y < radius)
      dy = radius - (y + 0.5);
    else if (y >= height - radius)
      dy = (y + 0.5) - (height - radius);
    if (dy <= 0) return 0;
    return static_cast<int>(std::lround(radius - std::sqrt(std::max(0.0, radius * radius - dy * dy))));
  };
  for (int y = 0; y <= height; ++y) {
    const int inset = y < height ? inset_at(y) : -1;
    if (inset == current) continue;
    if (current >= 0 && width - 2 * current > 0)
      bands.push_back({static_cast<short>(current), static_cast<short>(start),
                       static_cast<unsigned short>(width - 2 * current),
                       static_cast<unsigned short>(y - start)});
    start = y;
    current = inset;
  }
  return bands;
}

}  // namespace

WaveOverlayX11Surface::~WaveOverlayX11Surface() { close_display(); }

bool WaveOverlayX11Surface::ensure_display() {
  if (display_)
    return true;
  const auto *display_name = std::getenv("DISPLAY");
  if (!display_name || !*display_name)
    return false;
  display_ = XOpenDisplay(display_name);
  if (!display_)
    return false;
  int fixes_event = 0;
  int fixes_error = 0;
  if (!XFixesQueryExtension(display_, &fixes_event, &fixes_error)) {
    XCloseDisplay(display_);
    display_ = nullptr;
    return false;
  }
  randr_monitors_ = x11_overlay_randr_monitors(display_);
  return true;
}

bool WaveOverlayX11Surface::compositor_running() const {
  const auto selection = "_NET_WM_CM_S" + std::to_string(DefaultScreen(display_));
  const auto atom = XInternAtom(display_, selection.c_str(), False);
  return atom != None && XGetSelectionOwner(display_, atom) != None;
}

// 窗口按当时的合成状态创建：32 位 visual 只有经过合成管理器才显示得出 alpha，没有合成管理器时透明的圆角会变黑。两次录音之间合成管理器启动或停止，下一次 `show()` 会换一个新窗口。
bool WaveOverlayX11Surface::ensure_window() {
  const bool composited = compositor_running();
  if (window_ && composited == made_composited_)
    return true;
  destroy_window();
  const auto screen = DefaultScreen(display_);
  const auto root = RootWindow(display_, screen);
  Visual *visual = DefaultVisual(display_, screen);
  int depth = DefaultDepth(display_, screen);
  XSetWindowAttributes attributes{};
  attributes.override_redirect = True;
  attributes.background_pixel = 0;
  attributes.border_pixel = 0;
  unsigned long mask = CWOverrideRedirect | CWBackPixel | CWBorderPixel;
  bool argb = false;
  if (composited) {
    XVisualInfo visual_template{};
    visual_template.screen = screen;
    visual_template.depth = 32;
    visual_template.c_class = TrueColor;
    int visual_count = 0;
    XVisualInfo *visuals = XGetVisualInfo(display_, VisualScreenMask | VisualDepthMask | VisualClassMask,
                                          &visual_template, &visual_count);
    if (visuals && visual_count > 0) {
      visual = visuals[0].visual;
      depth = visuals[0].depth;
      colormap_ = XCreateColormap(display_, root, visual, AllocNone);
      attributes.colormap = colormap_;
      mask |= CWColormap;
      argb = true;
    }
    if (visuals)
      XFree(visuals);
  }
  // 位置和尺寸由 `place()` 设置，它知道目标显示器、缩放和布局。
  window_ = XCreateWindow(display_, root, 0, 0, 1, 1, 0, depth, InputOutput, visual, mask, &attributes);
  if (!window_) {
    if (colormap_)
      XFreeColormap(display_, colormap_);
    colormap_ = 0;
    return false;
  }
  visual_ = visual;
  // 记下窗口创建时对应的合成状态，即使没找到 ARGB visual 也记，下一次 `show()` 就不会白白重建。
  made_composited_ = composited;
  translucent_ = argb;
  cairo_surface_ = cairo_xlib_surface_create(display_, window_, visual_, 1, 1);
  XSelectInput(display_, window_, ExposureMask | ButtonPressMask | ButtonReleaseMask);
  // 尺寸置零，第一次 `place()` 就会设定窗口尺寸，第一次绘制就会切出形状。
  width_ = 0;
  height_ = 0;
  logical_width_ = 0;
  logical_height_ = 0;
  shaped_ = false;
  return true;
}

void WaveOverlayX11Surface::destroy_window() {
  if (!display_)
    return;
  if (action_pressed_)
    XUngrabPointer(display_, CurrentTime);
  action_pressed_ = false;
  if (cairo_surface_)
    cairo_surface_destroy(cairo_surface_);
  cairo_surface_ = nullptr;
  if (window_)
    XDestroyWindow(display_, window_);
  window_ = 0;
  if (colormap_)
    XFreeColormap(display_, colormap_);
  colormap_ = 0;
  visual_ = nullptr;
  visible_ = false;
}

void WaveOverlayX11Surface::close_display() {
  if (!display_)
    return;
  destroy_window();
  XCloseDisplay(display_);
  display_ = nullptr;
}

// 与 macOS 和 Windows 的定位一致：重读缩放和目标显示器，按当前布局设定窗口尺寸，把浮条居中放在有焦点窗口所在的显示器上，离工作区底边 10 px。
void WaveOverlayX11Surface::place(bool force) {
  if (!display_ || !window_)
    return;
  const auto now = std::chrono::steady_clock::now();
  const bool layout_changed = frame_.width != logical_width_ || frame_.height != logical_height_;
  if (!force && !layout_changed && width_ != 0 && now - placed_at_ < kPlacementRefreshInterval)
    return;
  placed_at_ = now;
  const auto scale = x11_overlay_scale(display_);
  const auto width = static_cast<unsigned>(std::max(1, wave_overlay_scaled(frame_.width, scale)));
  const auto height = static_cast<unsigned>(std::max(1, wave_overlay_scaled(frame_.height, scale)));
  if (width != width_ || height != height_ || scale != scale_ || layout_changed) {
    scale_ = scale;
    width_ = width;
    height_ = height;
    logical_width_ = frame_.width;
    logical_height_ = frame_.height;
    cairo_xlib_surface_set_size(cairo_surface_, static_cast<int>(width_), static_cast<int>(height_));
    shaped_ = false;
  }
  const auto target = x11_overlay_monitor(display_, randr_monitors_);
  const auto position = wave_overlay_monitor_bottom_center(
      target, static_cast<int>(width_), static_cast<int>(height_),
      wave_overlay_scaled(kWaveOverlayWorkAreaGap, scale_));
  XMoveResizeWindow(display_, window_, position.x, position.y, width_, height_);
}

// 只有两个圆形操作按钮接收指针，没有合成管理器时窗口切成圆角面板的形状。尺寸或操作按钮变化时两者都重做；圆角只由尺寸决定，因为每种布局的尺寸各不相同。
void WaveOverlayX11Surface::apply_shapes() {
  if (shaped_ && shaped_actions_ == frame_.actions)
    return;
  shaped_ = true;
  shaped_actions_ = frame_.actions;
  const double unit = static_cast<double>(width_) / std::max(1, frame_.width);
  const auto radius = static_cast<int>(std::ceil(kWaveOverlayActionRadius * unit));
  const auto center_y = static_cast<int>(std::lround(wave_overlay_action_center_y(frame_.height) * unit));
  const auto left_x = static_cast<int>(std::lround(wave_overlay_cancel_center_x() * unit));
  const auto right_x = static_cast<int>(std::lround(wave_overlay_confirm_center_x(frame_.width) * unit));
  XRectangle buttons[2] = {
      {static_cast<short>(left_x - radius), static_cast<short>(center_y - radius),
       static_cast<unsigned short>(2 * radius), static_cast<unsigned short>(2 * radius)},
      {static_cast<short>(right_x - radius), static_cast<short>(center_y - radius),
       static_cast<unsigned short>(2 * radius), static_cast<unsigned short>(2 * radius)}};
  XserverRegion input = XFixesCreateRegion(display_, frame_.actions ? buttons : nullptr, frame_.actions ? 2 : 0);
  if (input) {
    XFixesSetWindowShapeRegion(display_, window_, ShapeInput, 0, 0, input);
    XFixesDestroyRegion(display_, input);
  }
  if (!translucent_) {
    const double logical_radius =
        wave_overlay_corner_radius(frame_.height, frame_.actions, frame_.label, !frame_.transcript.empty());
    auto bands = rounded_bands(static_cast<int>(width_), static_cast<int>(height_), logical_radius * unit);
    XserverRegion bounding = XFixesCreateRegion(display_, bands.data(), static_cast<int>(bands.size()));
    if (bounding) {
      XFixesSetWindowShapeRegion(display_, window_, ShapeBounding, 0, 0, bounding);
      XFixesDestroyRegion(display_, bounding);
    }
  }
}

void WaveOverlayX11Surface::draw() {
  if (!display_ || !window_ || !cairo_surface_ || !visible_)
    return;
  place(false);
  if (!frame_.actions && action_pressed_) {
    XUngrabPointer(display_, CurrentTime);
    action_pressed_ = false;
  }
  apply_shapes();
  auto *cairo = cairo_create(cairo_surface_);
  // 先在屏幕外画好，再一次复制上去，16 ms 的一帧因此不会显示画了一半的面板。
  cairo_push_group(cairo);
  cairo_scale(cairo, static_cast<double>(width_) / std::max(1, frame_.width),
              static_cast<double>(height_) / std::max(1, frame_.height));
  painter_.paint(cairo, frame_, model_, translucent_);
  cairo_pop_group_to_source(cairo);
  cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
  cairo_paint(cairo);
  cairo_destroy(cairo);
  cairo_surface_flush(cairo_surface_);
  XFlush(display_);
}

void WaveOverlayX11Surface::pump_events() {
  if (!display_ || !visible_)
    return;
  while (visible_ && XPending(display_)) {
    XEvent event{};
    XNextEvent(display_, &event);
    if (event.type == Expose && event.xexpose.window == window_ && event.xexpose.count == 0) {
      // override-redirect 窗口自己不保留后备存储；被遮住的部分按上一次的模型重画。
      draw();
      continue;
    }
    const double unit = static_cast<double>(std::max(1, frame_.width)) / std::max(1u, width_);
    if (event.type == ButtonPress && event.xbutton.button == Button1) {
      WaveOverlayModel::Action action;
      if (wave_overlay_hit_action(model_, event.xbutton.x * unit, event.xbutton.y * unit, frame_.width,
                                  frame_.height, action)) {
        pressed_action_ = action;
        action_pressed_ = true;
        XGrabPointer(display_, window_, False, ButtonReleaseMask, GrabModeAsync, GrabModeAsync, None, None,
                     CurrentTime);
      }
    } else if (event.type == ButtonRelease && event.xbutton.button == Button1 && action_pressed_) {
      WaveOverlayModel::Action action;
      const bool activated = wave_overlay_hit_action(model_, event.xbutton.x * unit, event.xbutton.y * unit,
                                                     frame_.width, frame_.height, action) &&
                             action == pressed_action_;
      action_pressed_ = false;
      XUngrabPointer(display_, CurrentTime);
      if (activated && action_handler_)
        action_handler_(action);
    }
  }
}

bool WaveOverlayX11Surface::show(const WaveOverlayModel &model) {
  if (!ensure_display() || !ensure_window())
    return false;
  model_ = model;
  frame_ = painter_.frame(model_);
  visible_ = true;
  // Placed before mapping so the bar never flashes at its previous monitor or size.
  place(true);
  XMapRaised(display_, window_);
  draw();
  pump_events();
  return true;
}

void WaveOverlayX11Surface::update(const WaveOverlayModel &model) {
  if (!visible_)
    return;
  model_ = model;
  frame_ = painter_.frame(model_);
  pump_events();
  draw();
}

void WaveOverlayX11Surface::hide() {
  if (display_ && window_ && visible_) {
    XUnmapWindow(display_, window_);
    XFlush(display_);
  }
  if (display_ && action_pressed_)
    XUngrabPointer(display_, CurrentTime);
  visible_ = false;
  action_pressed_ = false;
}

int WaveOverlayX11Surface::event_fd() const {
  return display_ && visible_ ? ConnectionNumber(display_) : -1;
}

void WaveOverlayX11Surface::dispatch_events() { pump_events(); }

}  // namespace msime::linux_host

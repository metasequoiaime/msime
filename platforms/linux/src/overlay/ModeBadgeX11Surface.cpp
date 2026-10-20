#include "ModeBadgeX11Surface.h"

#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/extensions/shape.h>
#include <cairo/cairo-xlib.h>

#include <cstdlib>
#include <cstring>
#include <string>

#include "ModeBadgePainter.h"
#include "WaveOverlayX11Placement.h"

namespace msime::linux_host {

namespace {

// The margin from the work area's corner in logical pixels, the same as the Wayland badge's; show() multiplies it and the badge's own size by the desktop scale.
constexpr int kEdgeMargin = 24;

}  // namespace

std::unique_ptr<ModeBadgeX11Surface> ModeBadgeX11Surface::create() {
  const auto *display_name = std::getenv("DISPLAY");
  if (!display_name || !*display_name) return nullptr;
  std::unique_ptr<ModeBadgeX11Surface> surface(new ModeBadgeX11Surface());
  if (!surface->ensure_window()) return nullptr;
  return surface;
}

ModeBadgeX11Surface::~ModeBadgeX11Surface() { destroy_window(); }

bool ModeBadgeX11Surface::ensure_window() {
  if (display_ && window_) return true;
  display_ = XOpenDisplay(nullptr);
  if (!display_) return false;
  const auto screen = DefaultScreen(display_);
  const auto root = RootWindow(display_, screen);
  randr_monitors_ = x11_overlay_randr_monitors(display_);
  // 找一个 32 位 ARGB visual，否则圆角之外那几个像素只能是纯色，在任何背景上都是一块
  // 黑角。拿不到就退回默认 visual：形状难看，但提示仍然可用。
  XVisualInfo visual_template{};
  visual_template.screen = screen;
  visual_template.depth = 32;
  visual_template.c_class = TrueColor;
  int visual_count = 0;
  XVisualInfo *visuals = XGetVisualInfo(
      display_, VisualScreenMask | VisualDepthMask | VisualClassMask, &visual_template,
      &visual_count);
  XSetWindowAttributes attributes{};
  attributes.override_redirect = True;  // 不进任务栏、不被窗口管理器摆布、不抢焦点
  attributes.background_pixel = 0;
  attributes.border_pixel = 0;
  unsigned long mask = CWOverrideRedirect | CWBackPixel | CWBorderPixel;
  Visual *visual = DefaultVisual(display_, screen);
  int depth = DefaultDepth(display_, screen);
  if (visuals && visual_count > 0) {
    visual = visuals[0].visual;
    depth = visuals[0].depth;
    argb_ = true;
    attributes.colormap = XCreateColormap(display_, root, visual, AllocNone);
    colormap_ = attributes.colormap;
    mask |= CWColormap;
  }
  visual_ = visual;
  // Position and size are set by show(), which knows the target monitor, the scale and the badge's measured size.
  window_ = XCreateWindow(display_, root, 0, 0, 1, 1, 0, depth, InputOutput, visual,
                          mask, &attributes);
  if (visuals) XFree(visuals);
  if (!window_) {
    if (colormap_) {
      XFreeColormap(display_, colormap_);
      colormap_ = 0;
    }
    XCloseDisplay(display_);
    display_ = nullptr;
    return false;
  }
  // 提示不接受任何输入：空的输入形状让点击穿透到下面的窗口。没有 Shape 扩展时退而求其次，
  // 只是不选择任何输入事件。
  int shape_event = 0, shape_error = 0;
  if (XQueryExtension(display_, "SHAPE", &shape_event, &shape_error, &shape_error)) {
    const auto region = XCreateRegion();
    XShapeCombineRegion(display_, window_, ShapeInput, 0, 0, region, ShapeSet);
    XDestroyRegion(region);
  }
  XSelectInput(display_, window_, ExposureMask);
  return true;
}

bool ModeBadgeX11Surface::compositing() const {
  if (!display_) return false;
  const auto name = "_NET_WM_CM_S" + std::to_string(DefaultScreen(display_));
  const auto selection = XInternAtom(display_, name.c_str(), True);
  return selection != None && XGetSelectionOwner(display_, selection) != None;
}

bool ModeBadgeX11Surface::show(const std::string &text, const std::string &icon_path,
                               const ModeBadgeStyle &style, std::optional<ModeBadgeCaret> caret) {
  if (!ensure_window()) return false;
  // 合成器、显示器、面板和缩放在两次切换之间都可能变，切换又不频繁，所以每次 show 都重读，不缓存。
  auto metrics = style.metrics;
  metrics.shadow = argb_ && compositing();
  const auto layout = measure_mode_badge(metrics, icon_path);
  const auto scale = x11_overlay_scale(display_);
  const auto width = wave_overlay_scaled(layout.width, scale);
  const auto height = wave_overlay_scaled(layout.height, scale);
  const auto plate_width = wave_overlay_scaled(layout.plate_width, scale);
  const auto plate_height = wave_overlay_scaled(layout.plate_height, scale);
  const auto margin = wave_overlay_scaled(static_cast<int>(layout.margin), scale);
  WaveOverlayPosition position;
  if (caret) {
    // 跟随光标：在光标所在显示器的工作区里按 macOS 的规则放本体，再退出阴影边得到窗口位置。光标矩形不可用时以焦点窗口（再不行用指针）所在的显示器为准，放在 macOS 的回退位置。
    std::optional<WaveOverlayPosition> anchor;
    if (mode_badge_caret_usable(*caret))
      anchor = WaveOverlayPosition{caret->x + caret->width / 2, caret->y + caret->height / 2};
    const auto monitor = x11_overlay_monitor(display_, randr_monitors_, anchor);
    const auto plate = mode_badge_caret_position(
        caret, plate_width, plate_height, monitor.work,
        wave_overlay_scaled(static_cast<int>(ModeBadgeGeometry::caret_gap), scale),
        wave_overlay_scaled(static_cast<int>(ModeBadgeGeometry::screen_margin), scale));
    position = {plate.x - margin, plate.y - margin};
  } else {
    // 不跟随光标时放在焦点窗口（再不行用指针）所在显示器工作区的右下角，边距与 Wayland 徽章相同，GNOME 的 Xwayland 也走这里。
    const auto monitor = x11_overlay_monitor(display_, randr_monitors_);
    const auto plate = wave_overlay_bottom_right(monitor.work, plate_width, plate_height,
                                                 wave_overlay_scaled(kEdgeMargin, scale));
    position = {plate.x - margin, plate.y - margin};
  }
  // 先移动、改尺寸再映射，徽章不会在上一次的显示器或尺寸上闪一下。
  XMoveResizeWindow(display_, window_, position.x, position.y, static_cast<unsigned>(width),
                    static_cast<unsigned>(height));
  XMapRaised(display_, window_);
  text_ = text;
  icon_path_ = icon_path;
  colors_ = style.colors;
  layout_ = layout;
  window_width_ = width;
  window_height_ = height;
  paint(1.0);
  visible_ = true;
  return true;
}

void ModeBadgeX11Surface::paint(double alpha) {
  auto *surface = cairo_xlib_surface_create(display_, window_, visual_, window_width_, window_height_);
  auto *cairo = cairo_create(surface);
  // 画法按 layout 的逻辑像素来，缩放上下文就把本体、阴影、标志、字和描边一起缩放。
  cairo_scale(cairo, static_cast<double>(window_width_) / layout_.width,
              static_cast<double>(window_height_) / layout_.height);
  paint_mode_badge(cairo, layout_, text_, icon_path_, colors_, alpha);
  cairo_destroy(cairo);
  cairo_surface_destroy(surface);
  XFlush(display_);
}

bool ModeBadgeX11Surface::set_opacity(double alpha) {
  if (!visible_ || !display_ || !window_ || !argb_ || !compositing()) return false;
  paint(alpha);
  return true;
}

void ModeBadgeX11Surface::hide() {
  if (!visible_ || !display_ || !window_) return;
  visible_ = false;
  XUnmapWindow(display_, window_);
  XFlush(display_);
}

void ModeBadgeX11Surface::destroy_window() {
  if (display_ && window_) {
    XDestroyWindow(display_, window_);
    window_ = 0;
  }
  if (display_ && colormap_) {
    XFreeColormap(display_, colormap_);
    colormap_ = 0;
  }
  if (display_) {
    XCloseDisplay(display_);
    display_ = nullptr;
  }
  visible_ = false;
}

}  // namespace msime::linux_host

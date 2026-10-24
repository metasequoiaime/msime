#include "OverlayX11Window.h"

#include <X11/Xatom.h>
#include <X11/Xutil.h>
#include <X11/extensions/shape.h>
#include <cairo/cairo-xlib.h>

#include <algorithm>
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace msime::linux_host {

#ifdef MSIME_X11_IO_ERROR_EXIT_HANDLER
namespace {

// 用 open_overlay_display 打开、还没关掉的连接。Xlib 的 I/O 错误处理是整个进程一个，默认的那个打印一行就 exit(1)，不会走到每条连接的退出处理；所以这里装一个进程级的处理：这些连接只打印一行并返回，让 Xlib 接着调用它们自己的退出处理（记下断开），别的连接照旧交给之前的处理（默认的仍然退出）。只在主线程上用：IBus 引擎和 Fcitx5 插件的浮层都在宿主的主循环里。
std::vector<Display *> &overlay_displays() {
  static std::vector<Display *> displays;
  return displays;
}

XIOErrorHandler &previous_io_error_handler() {
  static XIOErrorHandler previous = nullptr;
  return previous;
}

int overlay_io_error(Display *display) {
  const auto &displays = overlay_displays();
  if (std::find(displays.begin(), displays.end(), display) != displays.end()) {
    std::fprintf(stderr, "MSIME: X connection to %s broken; closing the overlay connection\n", DisplayString(display));
    return 0;
  }
  return previous_io_error_handler() ? previous_io_error_handler()(display) : 0;
}

}  // namespace
#endif

namespace {

Window wm_check_window(Display *display, Window window, Atom property) {
  Atom type = None;
  int format = 0;
  unsigned long count = 0, remaining = 0;
  unsigned char *raw = nullptr;
  const auto status = XGetWindowProperty(display, window, property, 0, 1, False, XA_WINDOW,
                                          &type, &format, &count, &remaining, &raw);
  const Window result = status == Success && type == XA_WINDOW && format == 32 && count == 1 &&
                                remaining == 0 && raw
                            ? *reinterpret_cast<const unsigned long *>(raw)
                            : None;
  if (raw) XFree(raw);
  return result;
}

bool satellite_window_manager(Display *display) {
  const auto check = XInternAtom(display, "_NET_SUPPORTING_WM_CHECK", True);
  if (check == None) return false;
  // WM 可在两次读取之间退出。沿浮层现有的主线程探测约定保护查询，再恢复原错误处理器。
  XSync(display, False);
  const auto previous = XSetErrorHandler([](Display *, XErrorEvent *) { return 0; });
  const auto wm = wm_check_window(display, DefaultRootWindow(display), check);
  bool satellite = false;
  if (wm != None && wm_check_window(display, wm, check) == wm) {
    Atom type = None;
    int format = 0;
    unsigned long count = 0, remaining = 0;
    unsigned char *raw = nullptr;
    constexpr char expected[] = "xwayland-satellite";
    const auto property = XInternAtom(display, "_NET_WM_NAME", False);
    const auto utf8 = XInternAtom(display, "UTF8_STRING", False);
    const auto status = XGetWindowProperty(display, wm, property, 0, 64, False, AnyPropertyType,
                                            &type, &format, &count, &remaining, &raw);
    // satellite 0.8.2 写 STRING；同时接受 EWMH 规定的 UTF8_STRING，长度和内容都必须精确匹配。
    satellite = status == Success && (type == XA_STRING || type == utf8) && format == 8 &&
                count == sizeof(expected) - 1 && remaining == 0 && raw &&
                std::memcmp(raw, expected, sizeof(expected) - 1) == 0;
    if (raw) XFree(raw);
  }
  XSync(display, False);
  XSetErrorHandler(previous);
  return satellite;
}

}  // namespace

Display *open_overlay_display(const char *name, bool *lost) {
  *lost = false;
  Display *display = XOpenDisplay(name);
#ifdef MSIME_X11_IO_ERROR_EXIT_HANDLER
  if (display) {
    static const bool installed = [] {
      previous_io_error_handler() = XSetIOErrorHandler(overlay_io_error);
      return true;
    }();
    (void)installed;
    overlay_displays().push_back(display);
    // 这条连接自己的退出处理：不退出，记下断开并返回。
    XSetIOErrorExitHandler(
        display, [](Display *, void *flag) { *static_cast<bool *>(flag) = true; }, lost);
  }
#endif
  // satellite 尚无 root overlay 语义：override-redirect 浮层会变成普通顶层并抢焦点。
  // 工具栏和键位图共用此入口，拒绝后沿已有的无后端路径回退，不修改用户偏好。
  if (display && (satellite_window_manager(display) || *lost)) {
    close_overlay_display(display);
    return nullptr;
  }
  return display;
}

void close_overlay_display(Display *display) {
  if (!display) return;
#ifdef MSIME_X11_IO_ERROR_EXIT_HANDLER
  auto &displays = overlay_displays();
  displays.erase(std::remove(displays.begin(), displays.end(), display), displays.end());
#endif
  XCloseDisplay(display);
}

OverlayX11Window::OverlayX11Window(Display *display, const char *res_name, const char *res_class,
                                   const char *window_type, bool accepts_input)
    : display_(display), accepts_input_(accepts_input) {
  if (!display_) return;
  const auto screen = DefaultScreen(display_);
  const auto root = RootWindow(display_, screen);
  int shape_event = 0, shape_error = 0;
  shape_ = XShapeQueryExtension(display_, &shape_event, &shape_error);
  XVisualInfo info{};
  Visual *visual = DefaultVisual(display_, screen);
  int depth = DefaultDepth(display_, screen);
  // 一律先要 32 位 visual：合成器在不在每次画时再看，窗口建好后就不能再换 visual。
  if (XMatchVisualInfo(display_, screen, 32, TrueColor, &info)) {
    visual = info.visual;
    depth = info.depth;
    argb_ = true;
  }
  visual_ = visual;
  colormap_ = XCreateColormap(display_, root, visual, AllocNone);
  XSetWindowAttributes attributes{};
  attributes.override_redirect = True;
  attributes.colormap = colormap_;
  attributes.border_pixel = 0;
  attributes.background_pixel = 0;
  attributes.event_mask = ExposureMask;
  if (accepts_input_)
    attributes.event_mask |=
        ButtonPressMask | ButtonReleaseMask | PointerMotionMask | EnterWindowMask | LeaveWindowMask;
  window_ = XCreateWindow(display_, root, 0, 0, 1, 1, 0, depth, InputOutput, visual,
                          CWOverrideRedirect | CWColormap | CWBorderPixel | CWBackPixel | CWEventMask, &attributes);
  if (!window_) {
    XFreeColormap(display_, colormap_);
    colormap_ = 0;
    return;
  }
  XClassHint klass{};
  std::string instance = res_name, resource = res_class;
  klass.res_name = instance.data();
  klass.res_class = resource.data();
  XSetClassHint(display_, window_, &klass);
  XStoreName(display_, window_, res_class);
  // 不接受键盘焦点：输入法自己的窗口抢走焦点，按键就不再到正在输入的那个应用。
  XWMHints hints{};
  hints.flags = InputHint;
  hints.input = False;
  XSetWMHints(display_, window_, &hints);
  const auto type = XInternAtom(display_, window_type, False);
  XChangeProperty(display_, window_, XInternAtom(display_, "_NET_WM_WINDOW_TYPE", False), XA_ATOM, 32,
                  PropModeReplace, reinterpret_cast<const unsigned char *>(&type), 1);
  // 不接受输入的窗口用空的输入形状让点击穿透；没有 Shape 扩展时只是不选输入事件。
  if (!accepts_input_ && shape_) {
    const auto region = XCreateRegion();
    XShapeCombineRegion(display_, window_, ShapeInput, 0, 0, region, ShapeSet);
    XDestroyRegion(region);
  }
}

OverlayX11Window::~OverlayX11Window() {
  if (display_ && window_) XDestroyWindow(display_, window_);
  if (display_ && colormap_) XFreeColormap(display_, colormap_);
  if (display_) XFlush(display_);
}

bool OverlayX11Window::composited() const {
  if (!display_) return false;
  const auto name = "_NET_WM_CM_S" + std::to_string(DefaultScreen(display_));
  const auto selection = XInternAtom(display_, name.c_str(), True);
  return selection != None && XGetSelectionOwner(display_, selection) != None;
}

void OverlayX11Window::place(int x, int y, int width, int height) {
  if (!window_) return;
  width = width > 0 ? width : 1;
  height = height > 0 ? height : 1;
  if (x == x_ && y == y_ && width == width_ && height == height_) return;
  x_ = x;
  y_ = y;
  width_ = width;
  height_ = height;
  XMoveResizeWindow(display_, window_, x, y, static_cast<unsigned>(width), static_cast<unsigned>(height));
}

void OverlayX11Window::move(int x, int y) {
  if (!window_ || (x == x_ && y == y_)) return;
  x_ = x;
  y_ = y;
  XMoveWindow(display_, window_, x, y);
  XFlush(display_);
}

void OverlayX11Window::show() {
  if (!window_) return;
  XMapRaised(display_, window_);
  visible_ = true;
  XFlush(display_);
}

void OverlayX11Window::raise() {
  if (!window_ || !visible_) return;
  XRaiseWindow(display_, window_);
  XFlush(display_);
}

void OverlayX11Window::hide() {
  if (!window_ || !visible_) return;
  visible_ = false;
  XUnmapWindow(display_, window_);
  XFlush(display_);
}

void OverlayX11Window::set_input_region(const std::vector<XRectangle> &rectangles) {
  if (!window_ || !accepts_input_ || !shape_) return;
  if (rectangles.empty()) {
    XShapeCombineMask(display_, window_, ShapeInput, 0, 0, None, ShapeSet);
    return;
  }
  const auto region = XCreateRegion();
  for (auto rectangle : rectangles) XUnionRectWithRegion(&rectangle, region, region);
  XShapeCombineRegion(display_, window_, ShapeInput, 0, 0, region, ShapeSet);
  XDestroyRegion(region);
}

void OverlayX11Window::paint(double scale, std::uint32_t opaque_fallback, const std::function<void(cairo_t *)> &draw) {
  if (!window_) return;
  auto *buffer = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width_, height_);
  auto *cairo = cairo_create(buffer);
  cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
  cairo_set_source_rgba(cairo, 0, 0, 0, 0);
  cairo_paint(cairo);
  cairo_set_operator(cairo, CAIRO_OPERATOR_OVER);
  cairo_save(cairo);
  cairo_scale(cairo, scale, scale);
  draw(cairo);
  cairo_restore(cairo);
  const bool see_through = argb_ && composited();
  if (!see_through && shape_) {
    // 没有合成器：按 alpha 设二值外形，圆角外的像素不画；半透明的边缘合在底色上，不留黑边。
    cairo_surface_flush(buffer);
    const int stride = cairo_image_surface_get_stride(buffer);
    const auto *pixels = cairo_image_surface_get_data(buffer);
    const auto row_bytes = static_cast<std::size_t>((width_ + 7) / 8);
    std::vector<char> bits(row_bytes * static_cast<std::size_t>(height_), 0);
    for (int y = 0; y < height_; ++y)
      for (int x = 0; x < width_; ++x) {
        std::uint32_t pixel = 0;
        std::memcpy(&pixel, pixels + y * stride + x * 4, 4);
        // 只留下够实的像素：阴影那几层很淡的像素在没有合成器时只会是一圈脏边。
        if ((pixel >> 24) >= 0x80)
          bits[static_cast<std::size_t>(y) * row_bytes + static_cast<std::size_t>(x / 8)] |=
              static_cast<char>(1u << (x % 8));
      }
    const auto mask = XCreateBitmapFromData(display_, window_, bits.data(), static_cast<unsigned>(width_),
                                            static_cast<unsigned>(height_));
    XShapeCombineMask(display_, window_, ShapeBounding, 0, 0, mask, ShapeSet);
    XFreePixmap(display_, mask);
    bounding_shaped_ = true;
    cairo_set_operator(cairo, CAIRO_OPERATOR_DEST_OVER);
    cairo_set_source_rgb(cairo, ((opaque_fallback >> 16) & 0xff) / 255.0, ((opaque_fallback >> 8) & 0xff) / 255.0,
                         (opaque_fallback & 0xff) / 255.0);
    cairo_paint(cairo);
  } else if (see_through && bounding_shaped_) {
    XShapeCombineMask(display_, window_, ShapeBounding, 0, 0, None, ShapeSet);
    bounding_shaped_ = false;
  }
  cairo_destroy(cairo);
  auto *target = cairo_xlib_surface_create(display_, window_, visual_, width_, height_);
  cairo = cairo_create(target);
  cairo_set_operator(cairo, CAIRO_OPERATOR_SOURCE);
  cairo_set_source_surface(cairo, buffer, 0, 0);
  cairo_paint(cairo);
  cairo_destroy(cairo);
  cairo_surface_destroy(target);
  cairo_surface_destroy(buffer);
  XFlush(display_);
}

}  // namespace msime::linux_host

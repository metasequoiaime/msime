#include "NativeCanvasX11.h"

#include <fcitx-utils/event.h>
#include "../overlay/WaveOverlayX11Placement.h"
#include <X11/Xutil.h>
#include <X11/extensions/shape.h>
#include <cairo-xlib.h>

namespace msime::linux_host {

struct NativeCanvasX11::Impl {
  explicit Impl(fcitx::EventLoop &loop) : loop(loop) {}
  ~Impl() { close(); }
  fcitx::EventLoop &loop;
  Display *display = nullptr;
  Window window = 0;
  Visual *visual = nullptr;
  Colormap colormap = 0;
  std::string name;
  bool visible = false, composited = false;
  std::unique_ptr<fcitx::EventSourceIO> input;
  std::unique_ptr<fcitx::EventSourceTime> animation_timer;
  std::uint64_t animation_started_us = 0;
  std::shared_ptr<NativeCanvasAssets> assets;
  NativeCanvasContent content;
  NativeCanvasLayout layout;
  nlohmann::json palette;
  double scale = 1;
  int width = 1, height = 1;
  int pressed = -1, hovered = -1;
  int cursor_x = 0, cursor_y = 0, cursor_height = 0;

  void close() {
    animation_timer.reset();
    input.reset();
    if (display) {
      if (window) XDestroyWindow(display, window);
      if (colormap) XFreeColormap(display, colormap);
      XCloseDisplay(display);
    }
    display = nullptr; window = 0; colormap = 0; visible = false;
  }
  bool open(const std::string &target) {
    if (display && target == name) return true;
    close();
    if (target.rfind("x11:", 0) != 0) return false;
    display = XOpenDisplay(target.substr(4).c_str());
    if (!display) return false;
    name = target;
    const int screen = DefaultScreen(display);
    const auto root = RootWindow(display, screen);
    int shape_event = 0, shape_error = 0;
    if (!XShapeQueryExtension(display, &shape_event, &shape_error)) { close(); return false; }
    visual = DefaultVisual(display, screen);
    int depth = DefaultDepth(display, screen);
    // 只有合成管理器存在才选 ARGB；无合成器时用不透明底色保持文字可读。
    const auto selection = "_NET_WM_CM_S" + std::to_string(screen);
    XVisualInfo info{};
    composited = XGetSelectionOwner(display, XInternAtom(display, selection.c_str(), False)) != None &&
        XMatchVisualInfo(display, screen, 32, TrueColor, &info);
    if (composited) {
      visual = info.visual; depth = info.depth;
    }
    colormap = XCreateColormap(display, root, visual, AllocNone);
    XSetWindowAttributes attributes{};
    attributes.override_redirect = True;
    attributes.colormap = colormap;
    attributes.border_pixel = 0;
    attributes.background_pixel = 0;
    attributes.event_mask = ExposureMask | ButtonPressMask | ButtonReleaseMask | PointerMotionMask | EnterWindowMask | LeaveWindowMask;
    window = XCreateWindow(display, root, 0, 0, 1, 1, 0, depth, InputOutput, visual,
        CWOverrideRedirect | CWColormap | CWBorderPixel | CWBackPixel | CWEventMask, &attributes);
    if (!window) { close(); return false; }
    XClassHint klass{};
    char instance[] = "msime-candidate-canvas", resource[] = "MSIMECandidateCanvas";
    klass.res_name = instance; klass.res_class = resource;
    XSetClassHint(display, window, &klass);
    XStoreName(display, window, "MSIME candidate canvas");
    XWMHints hints{};
    hints.flags = InputHint; hints.input = False;
    XSetWMHints(display, window, &hints);
    const auto type = XInternAtom(display, "_NET_WM_WINDOW_TYPE_TOOLTIP", False);
    XChangeProperty(display, window, XInternAtom(display, "_NET_WM_WINDOW_TYPE", False),
        XA_ATOM, 32, PropModeReplace, reinterpret_cast<const unsigned char *>(&type), 1);
    input = loop.addIOEvent(ConnectionNumber(display), fcitx::IOEventFlag::In,
        [this](fcitx::EventSourceIO *, int, fcitx::IOEventFlags) { events(); return true; });
    return true;
  }
  int hit(int x, int y) const {
    return native_canvas_hit(content, layout, x / scale, y / scale);
  }
  void events() {
    while (display && XPending(display)) {
      XEvent event{};
      XNextEvent(display, &event);
      if (!visible) continue;
      if (event.type == Expose && event.xexpose.count == 0) paint();
      if (event.type == MotionNotify || event.type == EnterNotify) {
        const int at = event.type == MotionNotify ? hit(event.xmotion.x, event.xmotion.y) : hit(event.xcrossing.x, event.xcrossing.y);
        if (hovered != at) { hovered = at; paint(); }
      }
      if (event.type == LeaveNotify && hovered != -1) { hovered = -1; paint(); }
      if (event.type == ButtonPress) {
        if (event.xbutton.button == Button3) menu(hit(event.xbutton.x, event.xbutton.y));
        if (event.xbutton.button == Button1) { pressed = hit(event.xbutton.x, event.xbutton.y); paint(); }
        if (content.wheel_paging && (event.xbutton.button == Button4 || event.xbutton.button == Button5)) {
          const auto page = content.page;
          const bool next = event.xbutton.button == Button5;
          if (page && (next ? content.next : content.previous)) page(next);
        }
      }
      if (event.type == ButtonRelease && event.xbutton.button == Button1) {
        const int item = pressed;
        pressed = -1;
        paint();
        if (item == -1 || item != hit(event.xbutton.x, event.xbutton.y)) continue;
        // 回调可能在同一栈上更新 content，先复制函数对象，避免调用中析构它自己。
        const auto select = content.select;
        const auto page = content.page;
        if (item >= 0 && select) select(item);
        else if (item <= -2 && page) page(item == -3);
      }
    }
  }
  WaveOverlayWorkArea workArea() const {
    const bool randr = x11_overlay_randr_monitors(display);
    auto work = x11_overlay_monitor(display, randr).work;
    if (!randr) return work;
    const auto root = RootWindow(display, DefaultScreen(display));
    int count = 0;
    if (auto *monitors = XRRGetMonitors(display, root, True, &count)) {
      // 跨显示器的编辑器以光标所在屏幕定位，不能取整个活动窗口的中心。
      for (int at = 0; at < count; ++at) {
        const auto &monitor = monitors[at];
        if (cursor_x < monitor.x || cursor_y < monitor.y ||
            cursor_x >= monitor.x + monitor.width || cursor_y >= monitor.y + monitor.height) continue;
        work = wave_overlay_monitor_work({monitor.x, monitor.y, monitor.width, monitor.height},
            x11_placement::desktop_work_areas(display, root));
        break;
      }
      XRRFreeMonitors(monitors);
    }
    return work;
  }
  bool measure() {
    auto *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 1, 1);
    auto *cr = cairo_create(surface);
    layout = measure_native_canvas(cr, *assets, content);
    cairo_destroy(cr); cairo_surface_destroy(surface);
    const auto work = workArea();
    const double wanted_width = std::ceil(layout.width * scale), wanted_height = std::ceil(layout.height * scale);
    if (wanted_width <= 0 || wanted_height <= 0 || wanted_width > work.width ||
        wanted_height > work.height || wanted_width > 8192 || wanted_height > 8192) return false;
    width = static_cast<int>(wanted_width); height = static_cast<int>(wanted_height);
    return true;
  }
  void menu(int at) {
    if (at < 0 || !content.actions) return;
    auto actions = content.actions(at);
    if (actions.empty()) return;
    const auto original = content;
    NativeCanvasContent menu_content;
    menu_content.font = content.font;
    menu_content.menu = true;
    menu_content.preedit = "候选操作";
    for (const auto &action : actions) menu_content.items.push_back({{}, action.text});
    menu_content.items.push_back({{}, "返回候选"});
    menu_content.select = [this, actions, original](int index) {
      if (index >= 0 && static_cast<std::size_t>(index) < actions.size()) {
        actions[index].invoke();
      } else {
        content = original;
        pressed = hovered = -1;
        if (measure()) { shape(); move(cursor_x, cursor_y, cursor_height); paint(); }
      }
    };
    content = std::move(menu_content);
    if (!measure()) { content = original; measure(); return; }
    pressed = hovered = -1;
    shape(); move(cursor_x, cursor_y, cursor_height); paint();
  }
  void scheduleAnimation(std::uint64_t elapsed_ms) {
    std::optional<std::uint64_t> next;
    const auto inspect = [&](const std::shared_ptr<const NativeCanvasAnimation> &animation) {
      if (!animation) return;
      const auto sample = sample_native_canvas_animation(*animation, elapsed_ms);
      if (sample.next_ms && (!next || *sample.next_ms < *next)) next = sample.next_ms;
    };
    if (!content.menu) {
      inspect(assets->panel_animation);
      for (const auto &decoration : assets->decorations) inspect(decoration.animation);
    }
    if (!next) {
      if (animation_timer) animation_timer->setEnabled(false);
      return;
    }
    const auto deadline = animation_started_us + (elapsed_ms + *next) * 1000;
    if (!animation_timer) {
      animation_timer = loop.addTimeEvent(CLOCK_MONOTONIC, deadline, 0,
          [this](fcitx::EventSourceTime *, std::uint64_t) {
            paint();
            return true;
          });
    } else {
      animation_timer->setTime(deadline);
      animation_timer->setOneShot();
    }
  }
  void paint() {
    if (!visible || !assets) return;
    auto *buffer = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, width, height);
    auto *cr = cairo_create(buffer);
    cairo_scale(cr, scale, scale);
    const auto elapsed_ms = (fcitx::now(CLOCK_MONOTONIC) - animation_started_us) / 1000;
    paint_native_canvas(cr, *assets, content, layout, palette, elapsed_ms, {hovered, pressed});
    if (!composited) {
      cairo_surface_flush(buffer);
      const int stride = cairo_image_surface_get_stride(buffer);
      const auto *pixels = cairo_image_surface_get_data(buffer);
      const auto row_bytes = static_cast<std::size_t>((width + 7) / 8);
      std::vector<char> bits(row_bytes * static_cast<std::size_t>(height), 0);
      for (int y = 0; y < height; ++y) for (int x = 0; x < width; ++x) {
        std::uint32_t pixel = 0;
        std::memcpy(&pixel, pixels + y * stride + x * 4, 4);
        if (pixel >> 24) bits[static_cast<std::size_t>(y) * row_bytes + x / 8] |= static_cast<char>(1u << (x % 8));
      }
      const auto mask = XCreateBitmapFromData(display, window, bits.data(), static_cast<unsigned>(width), static_cast<unsigned>(height));
      XShapeCombineMask(display, window, ShapeBounding, 0, 0, mask, ShapeSet);
      XFreePixmap(display, mask);
      // 无合成器只能提供二值透明；半透明像素合在主题底色上，不能落成黑边。
      cairo_set_operator(cr, CAIRO_OPERATOR_DEST_OVER);
      native_canvas_color(cr, palette, "surface", "#f8f8f8");
      cairo_paint(cr);
    }
    cairo_destroy(cr);
    auto *surface = cairo_xlib_surface_create(display, window, visual, width, height);
    cr = cairo_create(surface);
    cairo_set_operator(cr, CAIRO_OPERATOR_SOURCE);
    cairo_set_source_surface(cr, buffer, 0, 0);
    cairo_paint(cr);
    cairo_destroy(cr);
    cairo_surface_destroy(surface); cairo_surface_destroy(buffer);
    XFlush(display);
    scheduleAnimation(elapsed_ms);
  }
  void shape() {
    const auto region = XCreateRegion();
    const auto add = [&](const NativeCanvasRect &box) {
      XRectangle rectangle{static_cast<short>(std::floor(box.x * scale)), static_cast<short>(std::floor(box.y * scale)),
          static_cast<unsigned short>(std::ceil(box.width * scale)), static_cast<unsigned short>(std::ceil(box.height * scale))};
      XUnionRectWithRegion(&rectangle, region, region);
    };
    for (const auto &box : layout.items) add(box);
    // 禁用箭头仍占输入区域，点击不应穿透到编辑器或下面的窗口。
    if (layout.pager.visible) { add(layout.previous); add(layout.next); }
    // 挂件和留白不吞鼠标；候选与箭头才接收点击。
    XShapeCombineRegion(display, window, ShapeInput, 0, 0, region, ShapeSet);
    XDestroyRegion(region);
  }
  void move(int x_at, int y_at, int height_at) {
    cursor_x = x_at; cursor_y = y_at; cursor_height = height_at;
    if (!display || !window || !assets) return;
    const auto work = workArea();
    int x = cursor_x - static_cast<int>(std::round(layout.panel.x * scale));
    int y = cursor_y + cursor_height;
    if (y + height > work.y + work.height) y = cursor_y - height;
    x = std::clamp(x, work.x, std::max(work.x, work.x + work.width - width));
    y = std::clamp(y, work.y, std::max(work.y, work.y + work.height - height));
    XMoveResizeWindow(display, window, x, y, static_cast<unsigned>(width), static_cast<unsigned>(height));
    XFlush(display);
  }
};

NativeCanvasX11::NativeCanvasX11(fcitx::EventLoop &loop) : impl_(std::make_unique<Impl>(loop)) {}
NativeCanvasX11::~NativeCanvasX11() = default;
bool NativeCanvasX11::show(const std::string &display, std::shared_ptr<NativeCanvasAssets> assets,
                           NativeCanvasContent content, nlohmann::json palette,
                           int cursor_x, int cursor_y, int cursor_height) {
  auto &state = *impl_;
  if (!assets || content.items.empty() || !state.open(display)) { hide(); return false; }
  state.pressed = state.hovered = -1;
  if (!state.visible || state.assets != assets) state.animation_started_us = fcitx::now(CLOCK_MONOTONIC);
  state.assets = std::move(assets); state.content = std::move(content); state.palette = std::move(palette);
  state.cursor_x = cursor_x; state.cursor_y = cursor_y; state.cursor_height = cursor_height;
  state.scale = x11_overlay_scale(state.display);
  if (!state.measure()) { hide(); return false; }
  state.move(cursor_x, cursor_y, cursor_height);
  state.shape();
  XMapRaised(state.display, state.window);
  state.visible = true;
  state.paint();
  return true;
}
void NativeCanvasX11::move(int x, int y, int height) { impl_->move(x, y, height); }
void NativeCanvasX11::hide() {
  auto &state = *impl_;
  state.pressed = state.hovered = -1;
  if (state.visible && state.display && state.window) {
    XUnmapWindow(state.display, state.window);
    XFlush(state.display);
  }
  state.visible = false;
  if (state.animation_timer) state.animation_timer->setEnabled(false);
  state.content = {};
}

} // msime::linux_host 命名空间

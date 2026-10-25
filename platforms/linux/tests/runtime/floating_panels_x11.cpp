#include "../src/overlay/FloatingToolbar.h"
#include "../src/overlay/FloatingToolbarPainter.h"
#include "../src/overlay/OverlayX11Window.h"
#include "../src/overlay/ShuangpinKeymapX11Surface.h"

#include <X11/Xatom.h>
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <poll.h>
#include <sys/socket.h>
#include <unistd.h>

#include <cairo/cairo-xlib.h>

#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <functional>
#include <iostream>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <vector>

namespace host = msime::linux_host;
using Json = nlohmann::json;
using Clock = std::chrono::steady_clock;

namespace {

void require(bool condition, const char *message) {
  if (!condition) throw std::runtime_error(message);
}

// 一个只为这个测试写的事件循环：poll 浮窗的连接，定时器按截止时间排。
struct PollLoop {
  struct Source {
    int fd = -1;
    Clock::time_point deadline;
    std::function<void()> callback;
    bool alive = true;
  };
  std::vector<std::shared_ptr<Source>> sources;
  host::OverlayLoop loop() {
    host::OverlayLoop result;
    result.watch = [this](int fd, std::function<void()> ready) {
      auto source = std::make_shared<Source>(Source{fd, {}, std::move(ready)});
      sources.push_back(source);
      return host::OverlayLoop::Token(source.get(), [source](void *) { source->alive = false; });
    };
    result.timer = [this](int milliseconds, std::function<void()> fire) {
      auto source = std::make_shared<Source>(
          Source{-1, Clock::now() + std::chrono::milliseconds(milliseconds), std::move(fire)});
      sources.push_back(source);
      return host::OverlayLoop::Token(source.get(), [source](void *) { source->alive = false; });
    };
    return result;
  }
  // 跑 milliseconds 毫秒：连接可读就分发，定时器到期就触发。顺序照 IBus 宿主的 GLib 主循环：这一轮要分发哪些源在分发前就定下来，同一轮里先分发连接、后分发定时器；连接的回调里新排的定时器等下一轮，被它撤掉的定时器这一轮不再触发。
  void run_for(int milliseconds) {
    const auto end = Clock::now() + std::chrono::milliseconds(milliseconds);
    while (Clock::now() < end) {
      std::vector<pollfd> fds;
      std::vector<std::shared_ptr<Source>> watched;
      for (const auto &source : sources)
        if (source->alive && source->fd >= 0) {
          fds.push_back({source->fd, POLLIN, 0});
          watched.push_back(source);
        }
      poll(fds.data(), fds.size(), 10);
      std::vector<std::shared_ptr<Source>> due;
      const auto now = Clock::now();
      for (const auto &source : sources)
        if (source->alive && source->fd < 0 && now >= source->deadline) due.push_back(source);
      for (std::size_t index = 0; index < fds.size(); ++index)
        if ((fds[index].revents & POLLIN) && watched[index]->alive) {
          const auto callback = watched[index]->callback;
          callback();
        }
      for (const auto &source : due)
        if (source->alive) {
          source->alive = false;
          const auto callback = source->callback;
          callback();
        }
    }
  }
  // 还在监视的连接有几条。
  std::size_t watches() const {
    std::size_t count = 0;
    for (const auto &source : sources)
      if (source->alive && source->fd >= 0) ++count;
    return count;
  }
};

struct Placed {
  ::Window id = 0;
  int x = 0, y = 0, width = 0, height = 0;
};

// 根窗口的子窗口里 window 排在第几（越大越靠上）；不在里面时是 -1。
int stacking_index(Display *display, ::Window window) {
  ::Window root = 0, parent = 0;
  ::Window *children = nullptr;
  unsigned count = 0;
  int found = -1;
  if (XQueryTree(display, DefaultRootWindow(display), &root, &parent, &children, &count))
    for (unsigned index = 0; index < count; ++index)
      if (children[index] == window) found = static_cast<int>(index);
  if (children) XFree(children);
  return found;
}

// 根窗口下正在显示、WM_CLASS 是 resource_class 的那块 override-redirect 窗口。
std::optional<Placed> visible(Display *display, const char *resource_class) {
  ::Window root = 0, parent = 0;
  ::Window *children = nullptr;
  unsigned count = 0;
  std::optional<Placed> found;
  if (XQueryTree(display, DefaultRootWindow(display), &root, &parent, &children, &count)) {
    for (unsigned index = 0; index < count; ++index) {
      XWindowAttributes attributes{};
      XClassHint klass{};
      if (!XGetWindowAttributes(display, children[index], &attributes) || attributes.map_state != IsViewable ||
          !attributes.override_redirect || !XGetClassHint(display, children[index], &klass))
        continue;
      if (std::string(klass.res_class) == resource_class)
        found = Placed{children[index], attributes.x, attributes.y, attributes.width, attributes.height};
      XFree(klass.res_name);
      XFree(klass.res_class);
    }
  }
  if (children) XFree(children);
  return found;
}

// 浮窗与观察者使用不同连接；观察连接的 XSync 不能保证浮窗的请求已生效，按实际状态等待。
template <typename Predicate>
void wait_until(Predicate ready, const char *message) {
  const auto deadline = Clock::now() + std::chrono::seconds(2);
  while (!ready()) {
    require(Clock::now() < deadline, message);
    poll(nullptr, 0, 5);
  }
}

// 像 X 服务器那样把一次指针事件送到窗口上；坐标是窗口里的。
void pointer(Display *display, const Placed &window, int type, unsigned button, int x, int y) {
  XEvent event{};
  event.type = type;
  if (type == MotionNotify) {
    event.xmotion.window = window.id;
    event.xmotion.x = x;
    event.xmotion.y = y;
    event.xmotion.x_root = window.x + x;
    event.xmotion.y_root = window.y + y;
    event.xmotion.state = button ? Button1Mask : 0;
  } else {
    event.xbutton.window = window.id;
    event.xbutton.button = button;
    event.xbutton.x = x;
    event.xbutton.y = y;
    event.xbutton.x_root = window.x + x;
    event.xbutton.y_root = window.y + y;
  }
  const long mask = type == MotionNotify ? PointerMotionMask : type == ButtonPress ? ButtonPressMask : ButtonReleaseMask;
  XSendEvent(display, window.id, False, mask, &event);
  XFlush(display);
}

void click(Display *display, PollLoop &loop, const Placed &window, unsigned button, int x, int y) {
  pointer(display, window, ButtonPress, button, x, y);
  loop.run_for(30);
  pointer(display, window, ButtonRelease, button, x, y);
  loop.run_for(60);
}

// 把屏幕上的一块窗口截下来存成 PNG 给人看（设置了 MSIME_TEST_CANVAS_ARTIFACTS 时）。
void save(Display *display, const Placed &window, const char *name) {
  const char *directory = std::getenv("MSIME_TEST_CANVAS_ARTIFACTS");
  if (!directory || !*directory) return;
  const auto screen = DefaultScreen(display);
  auto *source = cairo_xlib_surface_create(display, DefaultRootWindow(display), DefaultVisual(display, screen),
                                           DisplayWidth(display, screen), DisplayHeight(display, screen));
  auto *target = cairo_image_surface_create(CAIRO_FORMAT_RGB24, window.width, window.height);
  auto *cairo = cairo_create(target);
  cairo_set_source_surface(cairo, source, -window.x, -window.y);
  cairo_paint(cairo);
  cairo_destroy(cairo);
  cairo_surface_write_to_png(target, (std::string(directory) + "/" + name).c_str());
  cairo_surface_destroy(target);
  cairo_surface_destroy(source);
}

void toolbar(Display *display) {
  PollLoop loop;
  char directory_template[] = "/tmp/msime-toolbar-x11-XXXXXX";
  const std::filesystem::path directory = mkdtemp(directory_template);
  std::vector<std::pair<host::FloatingToolbarAction, std::string>> actions;
  host::FloatingToolbar::Options options;
  options.display_name = "水杉输入法";
  options.position_file = directory / "floating-toolbar-position.json";
  auto overlay = loop.loop();
  auto toolbar = host::FloatingToolbar::create(
      overlay,
      [&](const void *, host::FloatingToolbarAction action, const std::string &argument) {
        actions.emplace_back(action, argument);
      },
      options, host::FloatingToolbar::default_surfaces(overlay));
  require(toolbar != nullptr, "有 DISPLAY 时应当建出 X11 工具栏");
  const auto config = host::floating_toolbar_config(Json::object(), true, true);
  toolbar->configure(config, host::floating_toolbar_colors(host::CandidateTheme{}));
  host::FloatingToolbarState state;
  state.scheme_title = "全拼";
  state.schemes = {{"quanpin", "全拼"}, {"shuangpin", "双拼（小鹤）"}};
  const int owner = 1;
  toolbar->activate(&owner, state);
  loop.run_for(100);
  auto bar = visible(display, "MSIMEFloatingToolbar");
  require(bar.has_value(), "激活后没有显示工具栏");
  // Xvfb 没有合成器、没有面板：不留阴影边，放在根窗口右下角、离边 20，尺寸与 macOS 的默认一行相同（231 × 44，不画标志）。
  const auto screen = DefaultScreen(display);
  require(bar->width == 231 && bar->height == 44, "工具栏尺寸不对");
  require(bar->x + bar->width == DisplayWidth(display, screen) - 20 &&
              bar->y + bar->height == DisplayHeight(display, screen) - 20,
          "工具栏没有放在右下角");
  save(display, *bar, "floating-toolbar-x11.png");
  const auto layout = host::floating_toolbar_layout(config, false);
  const auto center = [&](std::size_t index) {
    const auto &cell = layout.buttons[index];
    return std::pair{static_cast<int>(cell.x + cell.width / 2), static_cast<int>(cell.y + cell.height / 2)};
  };

  // 点中/英按钮：动作交给属主。
  auto [mode_x, mode_y] = center(0);
  click(display, loop, *bar, Button1, mode_x, mode_y);
  require(actions.size() == 1 && actions[0].first == host::FloatingToolbarAction::ToggleInputMode, "点中/英按钮没有切换");

  // 指针停在按钮上一会儿出提示。
  pointer(display, *bar, MotionNotify, 0, mode_x, mode_y);
  loop.run_for(host::FloatingToolbarTiming::tooltip_ms + 200);
  require(visible(display, "MSIMEFloatingToolbarTooltip").has_value(), "停在按钮上没有出提示");

  // 切换方案按钮弹出方案菜单，点一项就切换并收起菜单。
  auto [scheme_x, scheme_y] = center(1);
  click(display, loop, *bar, Button1, scheme_x, scheme_y);
  auto menu = visible(display, "MSIMEFloatingToolbarMenu");
  require(menu.has_value(), "切换方案按钮没有弹出菜单");
  require(!visible(display, "MSIMEFloatingToolbarTooltip").has_value(), "菜单打开时提示还在");
  require(menu->y + menu->height <= bar->y, "方案菜单没有放在工具栏上方");
  save(display, *menu, "floating-toolbar-scheme-menu-x11.png");
  const auto menu_layout = host::floating_toolbar_menu_layout(host::floating_toolbar_scheme_menu(state), 1.0, false);
  const auto &row = menu_layout.rows[1];
  click(display, loop, *menu, Button1, static_cast<int>(row.x + 10), static_cast<int>(row.y + row.height / 2));
  require(actions.size() == 2 && actions[1].first == host::FloatingToolbarAction::SelectScheme &&
              actions[1].second == "shuangpin",
          "点方案菜单的一项没有切换方案");
  require(!visible(display, "MSIMEFloatingToolbarMenu").has_value(), "选了方案菜单还开着");

  // 设置按钮的右键是工具菜单；点到菜单外就收起，什么也不做。
  auto [settings_x, settings_y] = center(layout.buttons.size() - 1);
  pointer(display, *bar, ButtonPress, Button3, settings_x, settings_y);
  loop.run_for(60);
  menu = visible(display, "MSIMEFloatingToolbarMenu");
  require(menu.has_value(), "设置按钮的右键没有弹出工具菜单");
  pointer(display, *menu, ButtonRelease, Button3, -40, -40);
  loop.run_for(30);
  require(visible(display, "MSIMEFloatingToolbarMenu").has_value(), "右键松开就收起了工具菜单");
  pointer(display, *menu, ButtonPress, Button1, -40, -40);
  loop.run_for(60);
  require(!visible(display, "MSIMEFloatingToolbarMenu").has_value(), "点到菜单外没有收起工具菜单");
  require(actions.size() == 2, "点到菜单外不该触发动作");

  // 按住左端的握把拖动：窗口跟着走，松手后位置写进状态目录。
  const int handle_x = static_cast<int>(layout.handle.x + layout.handle.width / 2);
  const int handle_y = static_cast<int>(layout.handle.y + layout.handle.height / 2);
  pointer(display, *bar, ButtonPress, Button1, handle_x, handle_y);
  loop.run_for(30);
  pointer(display, *bar, MotionNotify, Button1, handle_x - 300, handle_y - 200);
  loop.run_for(30);
  pointer(display, *bar, ButtonRelease, Button1, handle_x - 300, handle_y - 200);
  loop.run_for(60);
  const auto moved = visible(display, "MSIMEFloatingToolbar");
  require(moved && moved->x == bar->x - 300 && moved->y == bar->y - 200, "拖动握把没有移动工具栏");
  std::ifstream stream(*options.position_file);
  const std::string text((std::istreambuf_iterator<char>(stream)), std::istreambuf_iterator<char>());
  const auto saved = host::parse_floating_toolbar_position(text);
  require(saved && saved->x == moved->x && saved->y == moved->y, "拖动后的位置没有记下来");
  require(actions.size() == 2, "拖动不该触发按钮");

  // 有的窗口管理器用 XRaiseWindow 抬起被点中的窗口，把它压到工具栏上面；前台窗口一换，工具栏重新抬到最上面。
  const auto screen_root = DefaultRootWindow(display);
  const ::Window front = XCreateSimpleWindow(display, screen_root, moved->x - 20, moved->y - 20, 400, 120, 0, 0, 0);
  XMapRaised(display, front);
  XSync(display, False);
  require(stacking_index(display, front) > stacking_index(display, moved->id), "测试窗口没有盖住工具栏");
  const unsigned long active = front;
  XChangeProperty(display, screen_root, XInternAtom(display, "_NET_ACTIVE_WINDOW", False), XA_WINDOW, 32,
                  PropModeReplace, reinterpret_cast<const unsigned char *>(&active), 1);
  XFlush(display);
  loop.run_for(100);
  require(stacking_index(display, moved->id) > stacking_index(display, front), "前台窗口换了以后工具栏没有回到最上面");
  // 同一个前台窗口进出全屏时不会改 _NET_ACTIVE_WINDOW，仍应当立即收起和恢复工具栏。
  const auto state_atom = XInternAtom(display, "_NET_WM_STATE", False);
  const unsigned long fullscreen = XInternAtom(display, "_NET_WM_STATE_FULLSCREEN", False);
  XChangeProperty(display, front, state_atom, XA_ATOM, 32, PropModeReplace,
                  reinterpret_cast<const unsigned char *>(&fullscreen), 1);
  XFlush(display);
  wait_until([&] {
    loop.run_for(10);
    return !visible(display, "MSIMEFloatingToolbar").has_value();
  }, "同一前台窗口进入全屏后工具栏仍然显示");
  XDeleteProperty(display, front, state_atom);
  XFlush(display);
  wait_until([&] {
    loop.run_for(10);
    return visible(display, "MSIMEFloatingToolbar").has_value();
  }, "同一前台窗口退出全屏后工具栏没有恢复");
#ifdef MSIME_X11_IO_ERROR_EXIT_HANDLER
  // 工具栏的连接断了（X 服务器或 Xwayland 退出）：进程不退出，下一轮主循环关掉这条连接，再有输入时连上新的连接重新显示。这里只关掉本端的读方向，Xlib 下一次读就读到文件尾，与服务器消失时一样。
  for (const auto &source : loop.sources)
    if (source->alive && source->fd >= 0) shutdown(source->fd, SHUT_RD);
  loop.run_for(100);
  require(!visible(display, "MSIMEFloatingToolbar").has_value(), "连接断开后旧的工具栏窗口还在");
  require(loop.watches() == 0, "连接断开后还在监视读到文件尾的连接");
  require(!toolbar->visible(), "连接断开后工具栏仍报告自己在屏幕上");
  // 前台窗口跨过断线保持不变；断线期间进入全屏，重连必须先重查状态并恢复它的属性订阅。
  XChangeProperty(display, front, state_atom, XA_ATOM, 32, PropModeReplace,
                  reinterpret_cast<const unsigned char *>(&fullscreen), 1);
  XSync(display, False);
  toolbar->note_input(&owner);
  loop.run_for(100);
  require(!visible(display, "MSIMEFloatingToolbar").has_value(), "重连后工具栏盖在全屏窗口上");
  XDeleteProperty(display, front, state_atom);
  XFlush(display);
  wait_until([&] {
    loop.run_for(10);
    return visible(display, "MSIMEFloatingToolbar").has_value();
  }, "重连后当前窗口退出全屏时工具栏没有恢复");
  // 工具栏因全屏而隐藏期间断线，旧的全屏判断也不能阻止下一次输入恢复连接。
  XChangeProperty(display, front, state_atom, XA_ATOM, 32, PropModeReplace,
                  reinterpret_cast<const unsigned char *>(&fullscreen), 1);
  XFlush(display);
  wait_until([&] {
    loop.run_for(10);
    return !visible(display, "MSIMEFloatingToolbar").has_value();
  }, "重连后当前窗口再次进入全屏时工具栏仍然显示");
  for (const auto &source : loop.sources)
    if (source->alive && source->fd >= 0) shutdown(source->fd, SHUT_RD);
  wait_until([&] {
    loop.run_for(10);
    return loop.watches() == 0;
  }, "全屏隐藏期间断线后仍在监视旧连接");
  XDeleteProperty(display, front, state_atom);
  XSync(display, False);
  toolbar->note_input(&owner);
  wait_until([&] {
    loop.run_for(10);
    return visible(display, "MSIMEFloatingToolbar").has_value();
  }, "全屏隐藏期间断线后工具栏无法随输入恢复");
#endif

  XDestroyWindow(display, front);
  XDeleteProperty(display, screen_root, XInternAtom(display, "_NET_ACTIVE_WINDOW", False));
  XSync(display, False);
  loop.run_for(60);

  // 换了输入法：收起。
  toolbar->deactivate(&owner);
  loop.run_for(60);
  require(!visible(display, "MSIMEFloatingToolbar").has_value(), "放开属主后工具栏还在");
  toolbar.reset();
  std::filesystem::remove_all(directory);
}

// IBus 宿主的切换类动作只对有焦点的引擎生效：属主失焦后工具栏留着，但要输入框的按钮画成淡色、点了不做事，打开设置这样的按钮照样能用。
void unfocused_toolbar(Display *display) {
  PollLoop loop;
  std::vector<host::FloatingToolbarAction> actions;
  host::FloatingToolbar::Options options;
  options.display_name = "水杉输入法";
  options.actions_need_focus = true;
  auto overlay = loop.loop();
  auto toolbar = host::FloatingToolbar::create(
      overlay, [&](const void *, host::FloatingToolbarAction action, const std::string &) { actions.push_back(action); },
      options, host::FloatingToolbar::default_surfaces(overlay));
  require(toolbar != nullptr, "有 DISPLAY 时应当建出 X11 工具栏");
  const auto config = host::floating_toolbar_config(Json::object(), true, true);
  toolbar->configure(config, host::floating_toolbar_colors(host::CandidateTheme{}));
  host::FloatingToolbarState state;
  state.scheme_title = "全拼";
  const int owner = 1;
  toolbar->activate(&owner, state);
  toolbar->focus_out(&owner);
  loop.run_for(100);
  const auto bar = visible(display, "MSIMEFloatingToolbar");
  require(bar.has_value(), "属主失焦后工具栏应当留着");
  save(display, *bar, "floating-toolbar-unfocused-x11.png");
  const auto layout = host::floating_toolbar_layout(config, false);
  const auto center = [&](std::size_t index) {
    const auto &cell = layout.buttons[index];
    return std::pair{static_cast<int>(cell.x + cell.width / 2), static_cast<int>(cell.y + cell.height / 2)};
  };
  auto [mode_x, mode_y] = center(0);
  click(display, loop, *bar, Button1, mode_x, mode_y);
  require(actions.empty(), "属主失焦时点中/英按钮不该有动作");
  auto [scheme_x, scheme_y] = center(1);
  click(display, loop, *bar, Button1, scheme_x, scheme_y);
  require(!visible(display, "MSIMEFloatingToolbarMenu").has_value(), "属主失焦时不该弹出方案菜单");
  auto [settings_x, settings_y] = center(layout.buttons.size() - 1);
  click(display, loop, *bar, Button1, settings_x, settings_y);
  require(actions.size() == 1 && actions[0] == host::FloatingToolbarAction::OpenSettings, "属主失焦时设置按钮应当照样能用");
  // 重新获得焦点：中/英按钮又能用了。
  toolbar->activate(&owner, state);
  loop.run_for(60);
  click(display, loop, *bar, Button1, mode_x, mode_y);
  require(actions.size() == 2 && actions[1] == host::FloatingToolbarAction::ToggleInputMode, "获得焦点后中/英按钮应当能用");
  toolbar.reset();
}

void keymap(Display *display) {
  host::ShuangpinKeymapX11Surface keymap;
  const host::ShuangpinKeymapHint hint{"xiaohe", 'H'};
  const auto colors = host::shuangpin_keymap_colors(host::CandidateTheme{});
  // 光标在上半屏：放在光标下方、让开候选窗再隔 8，左边对齐光标。
  require(keymap.show("", hint, colors, {200, 150, 2, 20, 60, std::nullopt}), "键位图没有画出来");
  std::optional<Placed> window;
  wait_until([&] {
    window = visible(display, "MSIMEShuangpinKeymap");
    return window && window->x == 200 && window->y == 150 + 20 + 60 + 8;
  }, "键位图没有放在光标下方");
  require(window->width == 620 && window->height == 203, "键位图尺寸不对");
  save(display, *window, "shuangpin-keymap-x11.png");
  // 宿主量出了候选窗的真实高度（Fcitx5 的原生画布）：按它让开，不用按字号估计的高度。
  require(keymap.show("", hint, colors, {200, 150, 2, 20, 60, 100}), "键位图没有画出来");
  wait_until([&] {
    window = visible(display, "MSIMEShuangpinKeymap");
    return window && window->y == 150 + 20 + 100 + 8;
  }, "键位图没有按量出的候选窗高度让开");
  // 光标靠近底边：翻到光标上方。
  const int bottom = DisplayHeight(display, DefaultScreen(display)) - 40;
  require(keymap.show("", hint, colors, {200, bottom, 2, 20, 60, std::nullopt}), "键位图没有画出来");
  wait_until([&] {
    window = visible(display, "MSIMEShuangpinKeymap");
    return window && window->y + window->height == bottom - 60 - 8;
  }, "键位图没有翻到光标上方");
  // 光标矩形不可用时不画。
  require(!keymap.show("", hint, colors, {0, 0, 0, 0, 60, std::nullopt}), "没有光标时不该画键位图");
  wait_until([&] { return !visible(display, "MSIMEShuangpinKeymap").has_value(); }, "没有光标时键位图还在");
}

// satellite 的 root 浮层会变成可抢焦点的 Wayland 顶层；按真实 EWMH 属性识别，不能只看桌面环境变量。
void satellite_fallback(Display *display) {
  const auto root = DefaultRootWindow(display);
  const auto check = XInternAtom(display, "_NET_SUPPORTING_WM_CHECK", False);
  const auto name = XInternAtom(display, "_NET_WM_NAME", False);
  const auto utf8 = XInternAtom(display, "UTF8_STRING", False);
  const auto wm = XCreateSimpleWindow(display, root, 0, 0, 1, 1, 0, 0, 0);
  const auto set_check = [&](Window window, Window target, Atom type = XA_WINDOW) {
    XChangeProperty(display, window, check, type, 32, PropModeReplace,
                    reinterpret_cast<const unsigned char *>(&target), 1);
  };
  const auto set_name = [&](const std::string &value, Atom type = XA_STRING) {
    XChangeProperty(display, wm, name, type, 8, PropModeReplace,
                    reinterpret_cast<const unsigned char *>(value.data()), value.size());
  };
  const auto expect_connection = [&](bool allowed) {
    XSync(display, False);
    bool lost = false;
    auto *connection = host::open_overlay_display(nullptr, &lost);
    const bool connected = connection != nullptr;
    host::close_overlay_display(connection);
    require(connected == allowed, allowed ? "非 satellite 的 X11 浮层被误禁用" : "satellite 上仍然创建 root 浮层连接");
    require(!lost, "WM 属性探测破坏了 X 连接");
  };
  set_check(root, wm);
  set_check(wm, wm);
  set_name("xwayland-satellite");
  expect_connection(false);
  set_name("xwayland-satellite", utf8);
  expect_connection(false);

  PollLoop loop;
  const auto overlay = loop.loop();
  const auto toolbar = host::FloatingToolbar::create(
      overlay, [](const void *, host::FloatingToolbarAction, const std::string &) {}, {},
      host::FloatingToolbar::default_surfaces(overlay));
  require(toolbar == nullptr, "satellite 上未使用工具栏菜单回退");
  host::ShuangpinKeymapX11Surface keymap;
  require(!keymap.show("", {"xiaohe", 'H'}, host::shuangpin_keymap_colors(host::CandidateTheme{}),
                       {200, 150, 2, 20, 60, std::nullopt}), "satellite 上仍然显示键位图浮层");
  require(!visible(display, "MSIMEFloatingToolbar") && !visible(display, "MSIMEShuangpinKeymap"),
          "satellite 回退后仍有浮层窗口");
  require(loop.watches() == 0, "不可用后端仍然保留连接监听");

  set_name("synthetic-window-manager");
  expect_connection(true);
  set_name("xwayland-satellite-extra");
  expect_connection(true);
  set_name(std::string("xwayland-satellite\0extra", 23));
  expect_connection(true);
  XDeleteProperty(display, wm, name);
  expect_connection(true);
  set_name("xwayland-satellite", XA_ATOM);
  expect_connection(true);
  set_name("xwayland-satellite");
  set_check(root, wm, XA_CARDINAL);
  expect_connection(true);
  set_check(root, wm);
  const Window extra_windows[]{wm, wm};
  XChangeProperty(display, root, check, XA_WINDOW, 32, PropModeReplace,
                  reinterpret_cast<const unsigned char *>(extra_windows), 2);
  expect_connection(true);
  set_check(root, wm);
  set_check(wm, root);
  expect_connection(true);
  XDeleteProperty(display, wm, check);
  expect_connection(true);
  set_check(wm, wm);
  XDestroyWindow(display, wm);
  // 窗口管理器退出时 root 可短暂留着已销毁的窗口编号，不能让 BadWindow 结束输入法。
  expect_connection(true);
  XDeleteProperty(display, root, check);
  expect_connection(true);
}

}  // namespace

// 在 Xvfb 里真的建出悬浮工具栏和双拼键位图的窗口：位置、点按钮、悬停提示、两种菜单、拖动和位置记忆，以及键位图的摆放和翻转。
int main() {
  try {
    Display *display = XOpenDisplay(nullptr);
    require(display != nullptr, "连不上 Xvfb");
    satellite_fallback(display);
    toolbar(display);
    unfocused_toolbar(display);
    keymap(display);
    XCloseDisplay(display);
    std::cout << "floating panels on X11 passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}

#include "FloatingToolbarX11Surface.h"

#include <X11/Xatom.h>
#include <X11/cursorfont.h>

#include <algorithm>
#include <cmath>
#include <cstdlib>
#include <utility>

#include "OverlayX11Window.h"
#include "WaveOverlayX11Placement.h"

namespace msime::linux_host {

namespace {

int device(double logical, double scale) { return static_cast<int>(std::lround(logical * scale)); }
int device_ceil(double logical, double scale) { return static_cast<int>(std::ceil(logical * scale)); }

XRectangle device_rect(const FloatingToolbarRect &rect, double scale) {
  return XRectangle{static_cast<short>(device(rect.x, scale)), static_cast<short>(device(rect.y, scale)),
                    static_cast<unsigned short>(device_ceil(rect.width, scale)),
                    static_cast<unsigned short>(device_ceil(rect.height, scale))};
}

}  // namespace

std::unique_ptr<FloatingToolbarX11Surface> FloatingToolbarX11Surface::create(OverlayLoop loop, Callbacks callbacks) {
  const auto *display = std::getenv("DISPLAY");
  if (!display || !*display) return nullptr;
  std::unique_ptr<FloatingToolbarX11Surface> surface(
      new FloatingToolbarX11Surface(std::move(loop), std::move(callbacks)));
  if (!surface->open()) return nullptr;
  return surface;
}

FloatingToolbarX11Surface::FloatingToolbarX11Surface(OverlayLoop loop, Callbacks callbacks)
    : loop_(std::move(loop)), callbacks_(std::move(callbacks)) {}

FloatingToolbarX11Surface::~FloatingToolbarX11Surface() {
  reconnect_.reset();
  close_connection();
}

bool FloatingToolbarX11Surface::open() {
  // 连接断了（X 服务器或 Xwayland 退出、重启）：不在分发事件的调用栈里时关掉它，下面重新连；分发中先什么也不画，由 dispatch 排的收尾关掉。
  if (display_ && lost_) {
    if (dispatching_) return false;
    close_connection();
  }
  if (display_) return true;
  display_ = open_overlay_display(nullptr, &lost_);
  if (!display_) return false;
  randr_monitors_ = x11_overlay_randr_monitors(display_);
  toolbar_ = std::make_unique<OverlayX11Window>(display_, "msime-floating-toolbar", "MSIMEFloatingToolbar",
                                                "_NET_WM_WINDOW_TYPE_UTILITY", true);
  if (!toolbar_->valid()) {
    toolbar_.reset();
    close_overlay_display(display_);
    display_ = nullptr;
    lost_ = false;
    return false;
  }
  // 前台窗口换了就重查是否全屏：根窗口的 _NET_ACTIVE_WINDOW 一变就有 PropertyNotify。
  active_window_atom_ = XInternAtom(display_, "_NET_ACTIVE_WINDOW", False);
  XSelectInput(display_, DefaultRootWindow(display_), PropertyChangeMask);
  watch_ = loop_.watch(ConnectionNumber(display_), [this] { dispatch(); });
  XFlush(display_);
  return true;
}

// 连接断了时窗口已经不在屏幕上（服务器那头随连接一起销毁了它）：报告不可见，控制器下一次刷新就调 present，经 open 重新连上。
bool FloatingToolbarX11Surface::visible() const { return toolbar_ && !lost_ && toolbar_->visible(); }

void FloatingToolbarX11Surface::dispatch() {
  dispatching_ = true;
  while (display_ && !lost_ && XPending(display_)) {
    XEvent event{};
    XNextEvent(display_, &event);
    handle(event);
  }
  dispatching_ = false;
  if (display_ && lost_) {
    // 连接断了：读到文件尾的套接字一直可读，这个监视现在就撤掉（两个宿主的适配都先复制回调再调用，在回调里撤掉自己是安全的），否则 GLib 每一轮都先分发它，同优先级、后加入的收尾定时器永远轮不到，宿主空转占满一个核。收尾只排一次：在下一轮主循环里关掉连接，工具栏下一次要显示时重新连。
    watch_.reset();
    if (!reconnect_) reconnect_ = loop_.timer(0, [this] { close_connection(); });
  }
}

void FloatingToolbarX11Surface::close_connection() {
  // open 先于排好的收尾关掉连接时把收尾一起撤掉，免得它稍后关掉新连上的连接；收尾自己调到这里时撤掉的是已经触发过的令牌。
  reconnect_.reset();
  watch_.reset();
  drain_.reset();
  tooltip_timer_.reset();
  if (display_ && pointer_grabbed_) XUngrabPointer(display_, CurrentTime);
  pointer_grabbed_ = false;
  if (display_ && move_cursor_ != None) XFreeCursor(display_, move_cursor_);
  move_cursor_ = None;
  cursor_on_handle_ = false;
  tooltip_.reset();
  menu_.reset();
  toolbar_.reset();
  hovered_.reset();
  pressed_.reset();
  menu_hovered_.reset();
  menu_opened_by_press_ = false;
  dragging_ = false;
  dragged_ = false;
  close_overlay_display(display_);
  display_ = nullptr;
  lost_ = false;
}

void FloatingToolbarX11Surface::drain_later() {
  // 正在分发时不另排：上面的循环会接着把队列取空。这也让空闲分发的定时器不会在它自己的回调里被换掉。
  if (!display_ || dispatching_ || XQLength(display_) == 0) return;
  drain_ = loop_.timer(0, [this] { dispatch(); });
}

void FloatingToolbarX11Surface::handle(const XEvent &event) {
  if (event.type == PropertyNotify && event.xproperty.window == DefaultRootWindow(display_)) {
    if (event.xproperty.atom == active_window_atom_) {
      // 换了前台窗口：开着的菜单收起（它是给上一个窗口的），再让控制器重查全屏。有的窗口管理器用 XRaiseWindow 抬起被点中的窗口框，那会把它压到包括 override-redirect 窗口在内的所有兄弟之上；这里把还在屏幕上的工具栏重新抬到最上面，与 macOS 的 NSFloatingWindowLevel 一样总在普通窗口之上。
      if (menu_ && menu_->visible()) close_menu();
      if (toolbar_ && toolbar_->visible()) toolbar_->raise();
      if (callbacks_.focus_changed) callbacks_.focus_changed();
    }
    return;
  }
  if (toolbar_ && event.xany.window == toolbar_->id()) {
    handle_toolbar(event);
  } else if (menu_ && event.xany.window == menu_->id()) {
    handle_menu(event);
  } else if (tooltip_ && event.xany.window == tooltip_->id() && event.type == Expose && event.xexpose.count == 0 &&
             tooltip_->visible()) {
    tooltip_->paint(scale_, content_.colors.surface,
                    [&](cairo_t *cairo) { paint_floating_toolbar_tooltip(cairo, tooltip_layout_, tooltip_text_, content_.colors); });
  }
}

void FloatingToolbarX11Surface::handle_toolbar(const XEvent &event) {
  switch (event.type) {
  case Expose:
    if (event.xexpose.count == 0 && toolbar_->visible()) paint_toolbar();
    break;
  case MotionNotify: {
    if (dragging_) {
      const int dx = event.xmotion.x_root - drag_pointer_x_, dy = event.xmotion.y_root - drag_pointer_y_;
      if (dx || dy) dragged_ = true;
      toolbar_->move(drag_window_x_ + dx, drag_window_y_ + dy);
      break;
    }
    const auto hit = floating_toolbar_hit(layout_, event.xmotion.x / scale_, event.xmotion.y / scale_);
    if (hit.handle != cursor_on_handle_) {
      // 拖动把手上用移动光标，对应 macOS 标志上的张开手形光标；离开把手换回默认光标。
      cursor_on_handle_ = hit.handle;
      if (hit.handle && move_cursor_ == None) move_cursor_ = XCreateFontCursor(display_, XC_fleur);
      XDefineCursor(display_, toolbar_->id(), hit.handle ? move_cursor_ : None);
    }
    set_hovered(hit.button);
    break;
  }
  case EnterNotify: {
    const auto hit = floating_toolbar_hit(layout_, event.xcrossing.x / scale_, event.xcrossing.y / scale_);
    set_hovered(hit.button);
    break;
  }
  case LeaveNotify:
    if (!dragging_) set_hovered(std::nullopt);
    break;
  case ButtonPress: {
    if (callbacks_.interacted) callbacks_.interacted();
    hide_tooltip();
    // 菜单开着时点工具栏只收起菜单，与点到菜单外其它地方一样。
    if (menu_ && menu_->visible()) {
      close_menu();
      break;
    }
    const auto hit = floating_toolbar_hit(layout_, event.xbutton.x / scale_, event.xbutton.y / scale_);
    if (event.xbutton.button == Button1) {
      if (hit.handle) {
        dragging_ = true;
        dragged_ = false;
        drag_pointer_x_ = event.xbutton.x_root;
        drag_pointer_y_ = event.xbutton.y_root;
        drag_window_x_ = toolbar_->x();
        drag_window_y_ = toolbar_->y();
      } else if (hit.button && *hit.button < faces_.size() && faces_[*hit.button].enabled) {
        pressed_ = hit.button;
        paint_toolbar();
      }
    } else if (event.xbutton.button == Button3 && hit.button &&
               content_.config.buttons[*hit.button] == FloatingToolbarButton::Settings) {
      // 设置按钮的右键是工具菜单，与 macOS 的 NSButton.menu（右键或 Control 点按）相同；左键照旧直接打开设置。
      auto items = floating_toolbar_utility_menu(content_.display_name);
      for (auto &item : items)
        item.disabled = !item.separator && !content_.focus_actions && floating_toolbar_action_needs_focus(item.action);
      open_menu(std::move(items), std::nullopt, event.xbutton.x_root, event.xbutton.y_root);
      menu_opened_by_press_ = true;
    }
    break;
  }
  case ButtonRelease: {
    if (event.xbutton.button != Button1) break;
    if (callbacks_.interacted) callbacks_.interacted();
    if (dragging_) {
      dragging_ = false;
      if (dragged_ && callbacks_.moved) {
        const int margin = device(layout_.margin, scale_);
        callbacks_.moved({toolbar_->x() + margin, toolbar_->y() + margin});
      }
      dragged_ = false;
      break;
    }
    if (!pressed_) break;
    const auto index = *pressed_;
    pressed_.reset();
    paint_toolbar();
    const auto hit = floating_toolbar_hit(layout_, event.xbutton.x / scale_, event.xbutton.y / scale_);
    if (hit.button != index || index >= content_.config.buttons.size()) break;
    const auto button = content_.config.buttons[index];
    if (button == FloatingToolbarButton::InputScheme) {
      open_menu(floating_toolbar_scheme_menu(content_.state), layout_.buttons[index], 0, 0);
      break;
    }
    // 回调可能在同一栈上更新工具栏（present 换掉 content_），这里不再读它。
    if (const auto action = floating_toolbar_button_action(button); action && callbacks_.action)
      callbacks_.action(*action, std::string{});
    break;
  }
  default:
    break;
  }
}

void FloatingToolbarX11Surface::handle_menu(const XEvent &event) {
  switch (event.type) {
  case Expose:
    if (event.xexpose.count == 0 && menu_->visible()) paint_menu();
    break;
  case MotionNotify: {
    const auto hit = floating_toolbar_menu_hit(menu_layout_, menu_items_, event.xmotion.x / scale_,
                                               event.xmotion.y / scale_);
    if (hit != menu_hovered_) {
      menu_hovered_ = hit;
      paint_menu();
    }
    break;
  }
  case LeaveNotify:
    if (menu_hovered_) {
      menu_hovered_.reset();
      paint_menu();
    }
    break;
  case ButtonPress: {
    if (callbacks_.interacted) callbacks_.interacted();
    // 抓着指针时，点到菜单外（包括别的应用的窗口）的按下也报到菜单窗口上，坐标落在它外面：收起菜单。
    const bool outside = event.xbutton.x < 0 || event.xbutton.y < 0 || event.xbutton.x >= menu_->width() ||
                         event.xbutton.y >= menu_->height();
    if (outside) close_menu();
    break;
  }
  case ButtonRelease: {
    const auto hit = floating_toolbar_menu_hit(menu_layout_, menu_items_, event.xbutton.x / scale_,
                                               event.xbutton.y / scale_);
    // 右键按下打开的菜单：那次按键的松开落在别处时不收起，让用户再点一项。
    if (menu_opened_by_press_) {
      menu_opened_by_press_ = false;
      if (!hit) break;
    }
    if (!hit) break;
    const auto item = menu_items_[*hit];
    close_menu();
    if (callbacks_.action) callbacks_.action(item.action, item.argument);
    break;
  }
  default:
    break;
  }
}

WaveOverlayWorkArea FloatingToolbarX11Surface::work_area_for(int x, int y) {
  return x11_overlay_monitor(display_, randr_monitors_, WaveOverlayPosition{x, y}).work;
}

void FloatingToolbarX11Surface::present(const Content &content, std::optional<WaveOverlayPosition> saved) {
  if (!open()) return;
  const bool was_visible = toolbar_->visible();
  const int old_margin = device(layout_.margin, scale_);
  const WaveOverlayPosition old_plate{toolbar_->x() + old_margin, toolbar_->y() + old_margin};
  content_ = content;
  faces_.clear();
  for (const auto button : content_.config.buttons) {
    faces_.push_back(floating_toolbar_face(button, content_.state, content_.display_name));
    faces_.back().enabled = content_.focus_actions || !floating_toolbar_button_needs_focus(button);
  }
  // 缩放、合成器和显示器在两次显示之间都可能变，每次都重读。
  scale_ = x11_overlay_scale(display_);
  shadow_ = toolbar_->composited();
  layout_ = floating_toolbar_layout(content_.config, shadow_);
  if (hovered_ && *hovered_ >= faces_.size()) hovered_.reset();
  if (pressed_ && *pressed_ >= faces_.size()) pressed_.reset();
  const int width = device_ceil(layout_.width, scale_), height = device_ceil(layout_.height, scale_);
  const int margin = device(layout_.margin, scale_);
  const int plate_width = width - 2 * margin, plate_height = height - 2 * margin;
  WaveOverlayPosition plate;
  if (was_visible && !dragging_) {
    // 已经在屏幕上：不挪地方，只按新尺寸收进它所在显示器的工作区（macOS 改尺寸时同样按「拖过的位置」收一次）。
    plate = floating_toolbar_position(old_plate, plate_width, plate_height,
                                      work_area_for(old_plate.x + plate_width / 2, old_plate.y + plate_height / 2), 0,
                                      device(FloatingToolbarGeometry::restored_margin, scale_));
  } else if (dragging_) {
    plate = old_plate;
  } else {
    const auto monitor = saved ? x11_overlay_monitor(display_, randr_monitors_,
                                                     WaveOverlayPosition{saved->x + plate_width / 2,
                                                                         saved->y + plate_height / 2})
                               : x11_overlay_monitor(display_, randr_monitors_);
    plate = floating_toolbar_position(saved, plate_width, plate_height, monitor.work,
                                      device(FloatingToolbarGeometry::default_margin, scale_),
                                      device(FloatingToolbarGeometry::restored_margin, scale_));
  }
  toolbar_->place(plate.x - margin, plate.y - margin, width, height);
  // 只有本体接受点击，阴影边让给下面的窗口。
  toolbar_->set_input_region({device_rect(layout_.plate, scale_)});
  if (!was_visible) toolbar_->show();
  paint_toolbar();
  if (menu_ && menu_->visible()) close_menu();
  drain_later();
}

void FloatingToolbarX11Surface::hide() {
  if (!toolbar_) return;
  close_menu();
  hide_tooltip();
  // 隐藏的窗口收不到离开事件，悬停中的按钮下次出现时不该还亮着，与 macOS 的 orderOut: 相同。
  hovered_.reset();
  pressed_.reset();
  dragging_ = false;
  dragged_ = false;
  toolbar_->hide();
  drain_later();
}

void FloatingToolbarX11Surface::paint_toolbar() {
  if (!toolbar_ || !toolbar_->visible()) return;
  FloatingToolbarFrame frame;
  frame.faces = faces_;
  frame.hovered = hovered_;
  frame.pressed = pressed_;
  frame.icon_path = content_.icon_path;
  frame.show_logo = content_.config.show_logo;
  toolbar_->paint(scale_, content_.colors.surface,
                  [&](cairo_t *cairo) { paint_floating_toolbar(cairo, layout_, frame, content_.colors); });
}

void FloatingToolbarX11Surface::set_hovered(std::optional<std::size_t> hovered) {
  if (hovered == hovered_) return;
  hovered_ = hovered;
  hide_tooltip();
  if (hovered_) schedule_tooltip();
  paint_toolbar();
}

void FloatingToolbarX11Surface::schedule_tooltip() {
  tooltip_timer_ = loop_.timer(FloatingToolbarTiming::tooltip_ms, [this] { show_tooltip(); });
}

void FloatingToolbarX11Surface::show_tooltip() {
  if (!hovered_ || *hovered_ >= faces_.size() || pressed_ || dragging_ || (menu_ && menu_->visible())) return;
  tooltip_text_ = faces_[*hovered_].tooltip;
  if (tooltip_text_.empty()) return;
  if (!tooltip_) {
    tooltip_ = std::make_unique<OverlayX11Window>(display_, "msime-floating-toolbar-tooltip",
                                                  "MSIMEFloatingToolbarTooltip", "_NET_WM_WINDOW_TYPE_TOOLTIP", false);
    if (!tooltip_->valid()) {
      tooltip_.reset();
      return;
    }
  }
  tooltip_layout_ = floating_toolbar_tooltip_layout(tooltip_text_, content_.config.scale, shadow_);
  const int width = device_ceil(tooltip_layout_.width, scale_), height = device_ceil(tooltip_layout_.height, scale_);
  const int margin = device(tooltip_layout_.margin, scale_);
  const auto &button = layout_.buttons[*hovered_];
  const int center_x = toolbar_->x() + device(button.x + button.width / 2.0, scale_);
  const int plate_top = toolbar_->y() + device(layout_.plate.y, scale_);
  const int plate_bottom = plate_top + device(layout_.plate.height, scale_);
  const auto work = work_area_for(center_x, plate_top);
  const int gap = device(6.0, scale_);
  // 放在工具栏上方，上面放不下就放到下方；水平居中在按钮上，再收进工作区。
  int y = plate_top - gap - (height - margin);
  if (y < work.y) y = plate_bottom + gap - margin;
  int x = center_x - width / 2;
  x = std::clamp(x, work.x, std::max(work.x, work.x + work.width - width));
  tooltip_->place(x, y, width, height);
  if (!tooltip_->visible()) tooltip_->show();
  tooltip_->paint(scale_, content_.colors.surface,
                  [&](cairo_t *cairo) { paint_floating_toolbar_tooltip(cairo, tooltip_layout_, tooltip_text_, content_.colors); });
  drain_later();
}

void FloatingToolbarX11Surface::hide_tooltip() {
  tooltip_timer_.reset();
  if (tooltip_) tooltip_->hide();
}

void FloatingToolbarX11Surface::open_menu(std::vector<FloatingToolbarMenuItem> items,
                                          std::optional<FloatingToolbarRect> below_button, int pointer_x,
                                          int pointer_y) {
  if (items.empty()) return;
  hide_tooltip();
  if (!menu_) {
    menu_ = std::make_unique<OverlayX11Window>(display_, "msime-floating-toolbar-menu", "MSIMEFloatingToolbarMenu",
                                               "_NET_WM_WINDOW_TYPE_POPUP_MENU", true);
    if (!menu_->valid()) {
      menu_.reset();
      return;
    }
  }
  menu_items_ = std::move(items);
  menu_hovered_.reset();
  menu_opened_by_press_ = false;
  menu_layout_ = floating_toolbar_menu_layout(menu_items_, content_.config.scale, shadow_);
  const int width = device_ceil(menu_layout_.width, scale_), height = device_ceil(menu_layout_.height, scale_);
  const int margin = device(menu_layout_.margin, scale_);
  const int gap = device(4.0, scale_);
  int x = 0, y = 0;
  WaveOverlayWorkArea work;
  if (below_button) {
    // 方案菜单贴着按钮弹出：工具栏默认在屏幕下方，与 macOS 一样放在按钮上方；上面放不下就放到下方。
    x = toolbar_->x() + device(below_button->x, scale_) - margin;
    const int plate_top = toolbar_->y() + device(layout_.plate.y, scale_);
    const int plate_bottom = plate_top + device(layout_.plate.height, scale_);
    work = work_area_for(x + width / 2, plate_top);
    y = plate_top - gap - (height - margin);
    if (y + margin < work.y) y = plate_bottom + gap - margin;
  } else {
    // 右键菜单从指针处弹出，下面放不下就向上。
    work = work_area_for(pointer_x, pointer_y);
    x = pointer_x - margin;
    y = pointer_y - margin;
    if (y + height - margin > work.y + work.height) y = pointer_y - (height - margin);
  }
  x = std::clamp(x, work.x - margin, std::max(work.x - margin, work.x + work.width - width + margin));
  y = std::clamp(y, work.y - margin, std::max(work.y - margin, work.y + work.height - height + margin));
  menu_->place(x, y, width, height);
  menu_->set_input_region({device_rect(menu_layout_.plate, scale_)});
  menu_->show();
  paint_menu();
  pointer_grabbed_ = XGrabPointer(display_, menu_->id(), True, ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
                                  GrabModeAsync, GrabModeAsync, None, None, CurrentTime) == GrabSuccess;
  XFlush(display_);
  drain_later();
}

void FloatingToolbarX11Surface::close_menu() {
  if (pointer_grabbed_) {
    XUngrabPointer(display_, CurrentTime);
    pointer_grabbed_ = false;
  }
  menu_opened_by_press_ = false;
  menu_hovered_.reset();
  if (menu_) menu_->hide();
}

void FloatingToolbarX11Surface::paint_menu() {
  if (!menu_ || !menu_->visible()) return;
  menu_->paint(scale_, content_.colors.surface, [&](cairo_t *cairo) {
    paint_floating_toolbar_menu(cairo, menu_layout_, menu_items_, menu_hovered_, content_.colors);
  });
}

bool FloatingToolbarX11Surface::foreground_fullscreen() {
  if (!display_) return false;
  const auto root = DefaultRootWindow(display_);
  const auto active = x11_placement::cardinal_property(display_, root, active_window_atom_, XA_WINDOW);
  if (active.empty() || active.front() == None) return false;
  const auto state_atom = XInternAtom(display_, "_NET_WM_STATE", False);
  const auto fullscreen_atom = XInternAtom(display_, "_NET_WM_STATE_FULLSCREEN", False);
  // 前台窗口可能在读到它和查询它之间被销毁，Xlib 默认的错误处理会让进程退出，所以查询期间换成忽略错误的处理。
  XSync(display_, False);
  const auto previous = XSetErrorHandler(x11_placement::ignore_x_error);
  const auto states = x11_placement::cardinal_property(display_, static_cast<Window>(active.front()), state_atom, XA_ATOM);
  XSync(display_, False);
  XSetErrorHandler(previous);
  drain_later();
  return std::find(states.begin(), states.end(), static_cast<std::uint32_t>(fullscreen_atom)) != states.end();
}

}  // namespace msime::linux_host

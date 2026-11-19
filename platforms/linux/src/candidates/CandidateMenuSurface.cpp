#include "CandidateMenuSurface.h"

#include <X11/Xatom.h>
#include <cairo/cairo.h>

#include <algorithm>
#include <cmath>
#include <utility>

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

CandidateMenuSurface::~CandidateMenuSurface() {
  close();
  if (display_)
    close_overlay_display(display_);
  display_ = nullptr;
}

bool CandidateMenuSurface::ensure_display() {
  if (display_)
    return !lost_;
  display_ = open_overlay_display(nullptr, &lost_);
  if (!display_)
    return false;
  lost_ = false;
  randr_monitors_ = x11_overlay_randr_monitors(display_);
  return true;
}

bool CandidateMenuSurface::ensure_window() {
  if (window_ && window_->valid())
    return true;
  window_ = std::make_unique<OverlayX11Window>(display_, "msime-candidate-menu", "MSIMECandidateMenu",
                                               "_NET_WM_WINDOW_TYPE_POPUP_MENU", true);
  if (!window_->valid()) {
    window_.reset();
    return false;
  }
  return true;
}

bool CandidateMenuSurface::show(std::vector<CandidateMenuItem> items,
                                const FloatingToolbarColors &colors, CandidateMenuAnchor anchor,
                                ActionHandler on_action, DismissHandler on_dismiss) {
  if (items.empty())
    return false;
  if (!ensure_display() || !ensure_window())
    return false;
  items_ = std::move(items);
  colors_ = colors;
  on_action_ = std::move(on_action);
  on_dismiss_ = std::move(on_dismiss);
  hovered_.reset();
  opened_by_press_ = true;
  // 只有纯 X11 会话的指针模式才取本机指针：candidate_clicked 不带坐标，指针就在
  // IBus 面板的候选行上。Wayland 会话传进来的是光标锚点，读 XQueryPointer 只会拿到
  // Xwayland 根窗口的陈旧角落坐标（child 为 0），那正是菜单跑到桌面右上方的原因。
  if (anchor.kind == CandidateMenuAnchor::Kind::Pointer && (anchor.x < 0 || anchor.y < 0)) {
    Window root_return = 0, child_return = 0;
    int root_x = anchor.x, root_y = anchor.y, win_x = 0, win_y = 0;
    unsigned int mask = 0;
    if (XQueryPointer(display_, DefaultRootWindow(display_), &root_return, &child_return,
                      &root_x, &root_y, &win_x, &win_y, &mask)) {
      anchor.x = root_x;
      anchor.y = root_y;
    }
  }
  // 菜单按候选窗的设备缩放画：x11_overlay_scale 从 X 连接读 Xft.dpi 与 GDK_SCALE，
  // 与模式徽章、语音浮层同一种取法。
  scale_ = x11_overlay_scale(display_);
  layout_ = floating_toolbar_menu_layout(items_, scale_, true);
  const int width = device_ceil(layout_.width, scale_);
  const int height = device_ceil(layout_.height, scale_);
  const int margin = device(layout_.margin, scale_);
  const int gap = device(12, scale_);
  const auto work =
      x11_overlay_monitor(display_, randr_monitors_, WaveOverlayPosition{anchor.x, anchor.y}).work;
  const auto position = candidate_menu_position(anchor, width, height, margin, gap, work);
  window_->place(position.x, position.y, width, height);
  window_->set_input_region({device_rect(layout_.plate, scale_)});
  window_->show();
  paint_menu();
  // 抓住指针：菜单外的按下也报到菜单窗口上（坐标落在窗口外），点外即收起。
  pointer_grabbed_ =
      XGrabPointer(display_, window_->id(), True,
                   ButtonPressMask | ButtonReleaseMask | PointerMotionMask, GrabModeAsync,
                   GrabModeAsync, 0, 0, CurrentTime) == GrabSuccess;
  XFlush(display_);
  visible_ = true;
  return true;
}

void CandidateMenuSurface::close() {
  if (display_ && pointer_grabbed_)
    XUngrabPointer(display_, CurrentTime);
  pointer_grabbed_ = false;
  opened_by_press_ = false;
  hovered_.reset();
  if (window_ && window_->visible())
    window_->hide();
  if (visible_) {
    visible_ = false;
    if (on_dismiss_)
      on_dismiss_();
  }
}

void CandidateMenuSurface::hide() { close(); }

void CandidateMenuSurface::paint_menu() {
  if (!window_ || !window_->visible())
    return;
  window_->paint(scale_, colors_.surface, [&](cairo_t *cairo) {
    paint_floating_toolbar_menu(cairo, layout_, items_, hovered_, colors_);
  });
}

void CandidateMenuSurface::pump_events() {
  if (!display_ || !visible_ || !window_)
    return;
  while (visible_ && XPending(display_)) {
    XEvent event{};
    XNextEvent(display_, &event);
    switch (event.type) {
    case Expose:
      if (event.xexpose.count == 0 && window_->visible())
        paint_menu();
      break;
    case MotionNotify: {
      const auto hit = floating_toolbar_menu_hit(
          layout_, items_, event.xmotion.x / scale_, event.xmotion.y / scale_);
      if (hit != hovered_) {
        hovered_ = hit;
        paint_menu();
      }
      break;
    }
    case LeaveNotify:
      if (hovered_) {
        hovered_.reset();
        paint_menu();
      }
      break;
    case ButtonPress: {
      // 抓着指针时，点到菜单外（包括别的应用的窗口）的按下也报到菜单窗口上，坐标
      // 落在它外面：收起菜单。
      const bool outside = event.xbutton.x < 0 || event.xbutton.y < 0 ||
                           event.xbutton.x >= window_->width() ||
                           event.xbutton.y >= window_->height();
      if (outside)
        close();
      break;
    }
    case ButtonRelease: {
      const auto hit = floating_toolbar_menu_hit(layout_, items_, event.xbutton.x / scale_,
                                                 event.xbutton.y / scale_);
      // 右键按下打开的菜单：那次按键的松开落在别处时不收起，让用户再点一项。
      if (opened_by_press_) {
        opened_by_press_ = false;
        if (!hit)
          break;
      }
      if (!hit)
        break;
      const auto index = *hit;
      close();
      if (on_action_)
        on_action_(index);
      break;
    }
    default:
      break;
    }
  }
}

int CandidateMenuSurface::event_fd() const {
  return display_ && visible_ ? ConnectionNumber(display_) : -1;
}

void CandidateMenuSurface::dispatch_events() { pump_events(); }

}  // namespace msime::linux_host

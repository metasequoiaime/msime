// 候选右键自绘菜单的 X11 弹层（IBus 宿主）。浮层不抢焦点，用指针抓取接住菜单外的
// 点击：点外收起，命中条目后回调一次动作并收起。事件由 IBus 主循环按
// event_fd()/dispatch_events() 泵（与语音浮层同一种模式，见 WaveOverlayX11Surface），
// 生命周期与指针交互照 FloatingToolbarX11Surface 的弹出菜单：右键按下打开时它松开的
// 第一次不算选择。连不上 X（无 X11、Xwayland 不可用）时 show 返回 false，调用方退回
// 候选窗的右键提示条。

#pragma once

#include "../candidates/CandidateMenuItems.h"
#include "../overlay/FloatingPanelColors.h"
#include "../overlay/FloatingToolbarPainter.h"
#include "../overlay/OverlayX11Window.h"
#include "../overlay/WaveOverlayX11Placement.h"

#include <X11/Xlib.h>
#include <cairo/cairo.h>

// X11/X.h 把 None 定义成 0L，会撞上本仓候选枚举里的 None（CandidatePanelLimit 等）。
// 这份头是宿主单元里唯一把 Xlib 和候选头一起拉进来的地方，在这里就地取消定义。
#undef None

#include <functional>
#include <memory>
#include <optional>
#include <vector>

namespace msime::linux_host {

class CandidateMenuSurface {
 public:
  // item_index 是 show 时条目表里的下标；分隔线与禁用项不会成为命中。
  using ActionHandler = std::function<void(std::size_t)>;
  // 菜单收起后的通知（点外、选择或外部 hide），调用方用它同步状态。
  using DismissHandler = std::function<void()>;

  CandidateMenuSurface() = default;
  ~CandidateMenuSurface();

  CandidateMenuSurface(const CandidateMenuSurface &) = delete;
  CandidateMenuSurface &operator=(const CandidateMenuSurface &) = delete;

  // 指针坐标是根窗口坐标；candidate_clicked 事件不带坐标，传负值时在显示连接上
  // XQueryPointer 取当前指针位置（IBus 面板上的右键，指针就在菜单该出现的地方）。
  // 条目为空、连不上显示或建不了窗口时返回 false，菜单不显示。
  bool show(std::vector<CandidateMenuItem> items, const FloatingToolbarColors &colors,
            int pointer_x, int pointer_y, ActionHandler on_action, DismissHandler on_dismiss);
  void hide();
  bool visible() const { return visible_; }
  // 显示中的菜单读指针和 expose 事件所用的显示连接，没有则 -1。
  int event_fd() const;
  void dispatch_events();

 private:
  bool ensure_display();
  bool ensure_window();
  void paint_menu();
  void pump_events();
  void close();

  Display *display_ = nullptr;
  // 显示连接断开时置真（open_overlay_display 的约定），之后 Xlib 调用都是空操作。
  bool lost_ = false;
  std::unique_ptr<OverlayX11Window> window_;
  std::vector<CandidateMenuItem> items_;
  FloatingToolbarColors colors_;
  FloatingToolbarMenuLayout layout_;
  double scale_ = 1.0;
  bool randr_monitors_ = false;
  bool visible_ = false;
  std::optional<std::size_t> hovered_;
  bool pointer_grabbed_ = false;
  bool opened_by_press_ = false;
  ActionHandler on_action_;
  DismissHandler on_dismiss_;
};

}  // namespace msime::linux_host

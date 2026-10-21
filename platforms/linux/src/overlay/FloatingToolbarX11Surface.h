#pragma once

#include <X11/Xlib.h>

#include <memory>
#include <optional>
#include <string>
#include <vector>

#include "FloatingToolbarPainter.h"
#include "FloatingToolbarSurface.h"
#include "OverlayLoop.h"

namespace msime::linux_host {

class OverlayX11Window;

// X11 上的悬浮工具栏：一块接受指针、从不接受键盘焦点的 override-redirect 窗口，按 macOS 的 MetasequoiaFloatingToolbarPanel 画和响应：按住左端的标志（或握把）拖动，点按钮切换或打开面板，指针停住时出提示，切换输入方案按钮弹出方案菜单，设置按钮的右键弹出工具菜单。菜单和提示是同一条连接上的另两块 override-redirect 窗口；菜单打开时抓住指针，点到菜单外就收起。事件经宿主的 OverlayLoop 分发，回调都在宿主主循环里、宿主自己的调用栈之外执行。
class FloatingToolbarX11Surface final : public FloatingToolbarSurface {
 public:
  ~FloatingToolbarX11Surface() override;
  FloatingToolbarX11Surface(const FloatingToolbarX11Surface &) = delete;
  FloatingToolbarX11Surface &operator=(const FloatingToolbarX11Surface &) = delete;

  // 连不上 X 服务器（没有 DISPLAY）时返回空指针。
  static std::unique_ptr<FloatingToolbarX11Surface> create(OverlayLoop loop, Callbacks callbacks);

  void present(const Content &content, std::optional<WaveOverlayPosition> saved) override;
  void hide() override;
  bool visible() const override;
  bool foreground_fullscreen() override;

 private:
  FloatingToolbarX11Surface(OverlayLoop loop, Callbacks callbacks);
  bool open();
  // 关掉连接和它上面的三块窗口，回到还没连过的状态。
  void close_connection();
  void dispatch();
  // 公开方法做过往返请求后，Xlib 可能已把事件读进自己的队列，连接上却不再可读；排一次空闲分发，免得它们等到下一次指针移动。
  void drain_later();
  void handle(const XEvent &event);
  void handle_toolbar(const XEvent &event);
  void handle_menu(const XEvent &event);
  void relayout();
  void paint_toolbar();
  void set_hovered(std::optional<std::size_t> hovered);
  void schedule_tooltip();
  void show_tooltip();
  void hide_tooltip();
  void open_menu(std::vector<FloatingToolbarMenuItem> items, std::optional<FloatingToolbarRect> below_button,
                 int pointer_x, int pointer_y);
  void close_menu();
  void paint_menu();
  WaveOverlayWorkArea work_area_for(int x, int y);

  OverlayLoop loop_;
  Callbacks callbacks_;
  Display *display_ = nullptr;
  // 这条连接断了（见 open_overlay_display）；之后的 Xlib 调用都是空操作，等关掉重连。
  bool lost_ = false;
  bool randr_monitors_ = false;
  OverlayLoop::Token watch_;
  OverlayLoop::Token drain_;
  OverlayLoop::Token reconnect_;
  bool dispatching_ = false;
  OverlayLoop::Token tooltip_timer_;
  std::unique_ptr<OverlayX11Window> toolbar_;
  std::unique_ptr<OverlayX11Window> menu_;
  std::unique_ptr<OverlayX11Window> tooltip_;
  Atom active_window_atom_ = None;
  Cursor move_cursor_ = None;
  bool cursor_on_handle_ = false;

  Content content_;
  FloatingToolbarLayout layout_;
  std::vector<FloatingToolbarFace> faces_;
  double scale_ = 1.0;
  bool shadow_ = false;
  std::optional<std::size_t> hovered_;
  std::optional<std::size_t> pressed_;
  // 按在拖动把手上：按下时的指针根坐标和窗口位置；moved 表示真的拖动过。
  bool dragging_ = false;
  bool dragged_ = false;
  int drag_pointer_x_ = 0, drag_pointer_y_ = 0;
  int drag_window_x_ = 0, drag_window_y_ = 0;

  std::vector<FloatingToolbarMenuItem> menu_items_;
  FloatingToolbarMenuLayout menu_layout_;
  std::optional<std::size_t> menu_hovered_;
  bool pointer_grabbed_ = false;
  // 菜单是由这次按下打开的：同一次按键的松开不算选中菜单项。
  bool menu_opened_by_press_ = false;
  std::string tooltip_text_;
  FloatingToolbarTooltipLayout tooltip_layout_;
};

}  // namespace msime::linux_host

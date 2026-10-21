#pragma once

#include "WaveOverlayPainter.h"
#include "WaveOverlaySurface.h"

#include <X11/Xlib.h>
#include <cairo/cairo.h>

#include <chrono>
#include <functional>
#include <utility>

namespace msime::linux_host {

// 为不提供 layer-shell 协议的桌面准备的轻量 X11 回退。浮层从不获取焦点，只在明确的操作按钮区域接收指针输入。它经 `WaveOverlayPainter` 用 cairo-xlib 和 Pango 绘制：有合成管理器时用 32 位 ARGB visual 和半透明圆角，否则画成不透明，并用 Shape 扩展切成圆角形状。
class WaveOverlayX11Surface final : public WaveOverlaySurface {
 public:
  explicit WaveOverlayX11Surface(ActionHandler action_handler = {})
      : action_handler_(std::move(action_handler)) {}
  ~WaveOverlayX11Surface() override;

  bool show(const WaveOverlayModel &model) override;
  void update(const WaveOverlayModel &model) override;
  void hide() override;
  int event_fd() const override;
  void dispatch_events() override;
  bool animates() const override { return true; }

 private:
  bool ensure_display();
  bool ensure_window();
  bool compositor_running() const;
  void destroy_window();
  void close_display();
  void place(bool force);
  void apply_shapes();
  void draw();
  void pump_events();

  Display *display_ = nullptr;
  Window window_ = 0;
  Visual *visual_ = nullptr;
  Colormap colormap_ = 0;
  cairo_surface_t *cairo_surface_ = nullptr;
  // 窗口创建时有合成管理器在运行，所以它用带 alpha 的 32 位 visual；没有合成管理器时面板改为不透明并切出形状。
  bool translucent_ = false;
  bool made_composited_ = false;
  WaveOverlayPainter painter_;
  // 最近一次显示的模型，用于 Expose 重画和两次更新之间的命中测试。
  WaveOverlayModel model_;
  WaveOverlayFrame frame_;
  bool visible_ = false;
  ActionHandler action_handler_;
  WaveOverlayModel::Action pressed_action_ = WaveOverlayModel::Action::Confirm;
  bool action_pressed_ = false;
  // RandR 1.5 monitor enumeration; without it the bar is placed on the EWMH work area or the root window.
  bool randr_monitors_ = false;
  double scale_ = 1.0;
  // 设备像素尺寸及据以算出它的逻辑尺寸；形状按这组几何尺寸和面板的操作按钮切出。
  unsigned width_ = 0;
  unsigned height_ = 0;
  int logical_width_ = 0;
  int logical_height_ = 0;
  bool shaped_actions_ = false;
  bool shaped_ = false;
  std::chrono::steady_clock::time_point placed_at_{};
};

}  // namespace msime::linux_host

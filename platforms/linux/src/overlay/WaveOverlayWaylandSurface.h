#pragma once

#include "WaveOverlayPainter.h"
#include "WaveOverlaySurface.h"

#include <wayland-client.h>

#include <array>
#include <cstddef>
#include <cstdint>
#include <functional>
#include <utility>
#include <vector>

struct wl_buffer;
struct wl_display;
struct wl_registry;
struct wl_compositor;
struct wl_shm;
struct wl_seat;
struct wl_pointer;
struct wl_surface;
struct wl_output;
struct zwlr_layer_shell_v1;
struct zwlr_layer_surface_v1;

namespace msime::linux_host {

// 原生的 wlroots layer-shell 浮层。它从不请求键盘焦点；不支持的合成器使用工厂里的 IBus 回退。它按当前布局定尺寸，经 `WaveOverlayPainter` 以所在 output 的整数缩放绘制，位于 output 可用区域底边以上 10 px，与 X11 和 macOS 的浮条一样。
class WaveOverlayWaylandSurface final : public WaveOverlaySurface {
 public:
  explicit WaveOverlayWaylandSurface(ActionHandler action_handler = {})
      : action_handler_(std::move(action_handler)) {}
  ~WaveOverlayWaylandSurface() override;

  bool show(const WaveOverlayModel &model) override;
  void update(const WaveOverlayModel &model) override;
  void hide() override;
  int event_fd() const override;
  void dispatch_events() override;
  bool animates() const override { return true; }

 private:
  static void registry_global(void *, wl_registry *, uint32_t, const char *, uint32_t);
  static void registry_remove(void *, wl_registry *, uint32_t);
  static void seat_capabilities(void *, wl_seat *, uint32_t);
  static void seat_name(void *, wl_seat *, const char *);
  static void pointer_enter(void *, wl_pointer *, uint32_t, wl_surface *,
                            wl_fixed_t, wl_fixed_t);
  static void pointer_leave(void *, wl_pointer *, uint32_t, wl_surface *);
  static void pointer_motion(void *, wl_pointer *, uint32_t, wl_fixed_t,
                             wl_fixed_t);
  static void pointer_button(void *, wl_pointer *, uint32_t, uint32_t,
                             uint32_t, uint32_t);
  static void pointer_axis(void *, wl_pointer *, uint32_t, uint32_t,
                           wl_fixed_t);
  static void pointer_frame(void *, wl_pointer *);
  static void pointer_axis_source(void *, wl_pointer *, uint32_t);
  static void pointer_axis_stop(void *, wl_pointer *, uint32_t, uint32_t);
  static void pointer_axis_discrete(void *, wl_pointer *, uint32_t, int32_t);
  static void pointer_axis_value120(void *, wl_pointer *, uint32_t, int32_t);
  static void output_geometry(void *, wl_output *, int32_t, int32_t, int32_t, int32_t, int32_t,
                              const char *, const char *, int32_t);
  static void output_mode(void *, wl_output *, uint32_t, int32_t, int32_t, int32_t);
  static void output_done(void *, wl_output *);
  static void output_scale(void *, wl_output *, int32_t);
  static void surface_enter(void *, wl_surface *, wl_output *);
  static void surface_leave(void *, wl_surface *, wl_output *);
  static void layer_configure(void *, zwlr_layer_surface_v1 *, uint32_t, uint32_t,
                              uint32_t);
  static void layer_closed(void *, zwlr_layer_surface_v1 *);
  static void buffer_release(void *, wl_buffer *);
  bool ensure_surface();
  bool configure_size(bool map);
  bool ensure_buffers(int width, int height);
  void destroy_buffers();
  int buffer_scale() const;
  void draw();
  void pump_events();
  void set_input_region();
  void destroy_surface();
  void release_buffer(std::size_t index);

  struct Output {
    wl_output *output = nullptr;
    uint32_t name = 0;
    int32_t scale = 1;
  };

  wl_display *display_ = nullptr;
  wl_registry *registry_ = nullptr;
  wl_compositor *compositor_ = nullptr;
  uint32_t compositor_version_ = 0;
  wl_shm *shm_ = nullptr;
  wl_seat *seat_ = nullptr;
  wl_pointer *pointer_ = nullptr;
  zwlr_layer_shell_v1 *layer_shell_ = nullptr;
  wl_surface *surface_ = nullptr;
  zwlr_layer_surface_v1 *layer_surface_ = nullptr;
  std::vector<Output> outputs_;
  // 浮层所在的 output，来自 `wl_surface.enter`/`leave`；缓冲区取其中最大的缩放。
  std::vector<wl_output *> entered_;
  bool scale_changed_ = false;
  std::array<wl_buffer *, 2> buffers_{};
  std::array<void *, 2> pixels_{};
  std::array<bool, 2> buffer_busy_{};
  struct BufferContext {
    WaveOverlayWaylandSurface *owner = nullptr;
    std::size_t index = 0;
  };
  std::array<BufferContext, 2> buffer_contexts_{};
  int shm_fd_ = -1;
  std::size_t buffer_size_ = 0;
  std::size_t next_buffer_ = 0;
  // 当前缓冲区的设备像素尺寸。
  int buffer_width_ = 0;
  int buffer_height_ = 0;
  // 上一次用 `set_size` 请求的逻辑尺寸；换成另一尺寸的布局时，绘制之前重新请求。
  int requested_width_ = 0;
  int requested_height_ = 0;
  // 输入区域的几何形状，只有尺寸或操作按钮变化时才重新发送。
  int input_width_ = -1;
  bool input_actions_ = false;
  bool configured_ = false;
  bool visible_ = false;
  bool closed_ = false;
  WaveOverlayPainter painter_;
  WaveOverlayModel model_;
  WaveOverlayFrame frame_;
  ActionHandler action_handler_;
  WaveOverlayModel::Action pressed_action_ = WaveOverlayModel::Action::Confirm;
  bool action_pressed_ = false;
  bool pointer_inside_ = false;
  double pointer_x_ = 0;
  double pointer_y_ = 0;
};

}  // namespace msime::linux_host

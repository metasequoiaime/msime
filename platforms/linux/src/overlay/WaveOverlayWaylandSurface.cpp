#include "WaveOverlayWaylandSurface.h"

#include "WaveOverlayPlacement.h"

#include "wlr-layer-shell-unstable-v1-client-protocol.h"

#include <wayland-client.h>

#include <cairo/cairo.h>

#include <algorithm>
#include <cerrno>
#include <cmath>
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <string>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <poll.h>
#include <unistd.h>

namespace msime::linux_host {
namespace {

int create_shm_file() {
#ifdef SYS_memfd_create
  const auto memfd = static_cast<int>(syscall(SYS_memfd_create,
                                              "msime-wave-overlay", MFD_CLOEXEC));
  if (memfd >= 0)
    return memfd;
#endif
  char name[64];
  std::snprintf(name, sizeof(name), "/msime-wave-%ld", static_cast<long>(getpid()));
  const auto fd = shm_open(name, O_CREAT | O_EXCL | O_RDWR, 0600);
  if (fd >= 0)
    shm_unlink(name);
  return fd;
}

void release_pointer(wl_pointer *pointer) {
  if (wl_pointer_get_version(pointer) >= WL_POINTER_RELEASE_SINCE_VERSION)
    wl_pointer_release(pointer);
  else
    wl_pointer_destroy(pointer);
}

}  // namespace

void WaveOverlayWaylandSurface::registry_global(
    void *data, wl_registry *registry, uint32_t name, const char *interface,
    uint32_t version) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  if (std::strcmp(interface, "wl_compositor") == 0 && !self->compositor_) {
    self->compositor_version_ = std::min(version, 4u);
    self->compositor_ = static_cast<wl_compositor *>(wl_registry_bind(
        registry, name, &wl_compositor_interface, self->compositor_version_));
  } else if (std::strcmp(interface, "wl_output") == 0 && version >= 2) {
    // 版本 2 带来 scale 事件，这是浮条需要从 output 得到的全部信息。
    auto *output = static_cast<wl_output *>(wl_registry_bind(registry, name, &wl_output_interface, 2));
    // 逐个成员赋值，与下面的指针监听器一样：较新的 wayland 头文件增加了 name 和 description，版本 2 的绑定永远收不到它们。
    static const wl_output_listener output_listener = [] {
      wl_output_listener listener{};
      listener.geometry = output_geometry;
      listener.mode = output_mode;
      listener.done = output_done;
      listener.scale = output_scale;
      return listener;
    }();
    wl_output_add_listener(output, &output_listener, self);
    self->outputs_.push_back({output, name, 1});
  } else if (std::strcmp(interface, "wl_shm") == 0 && !self->shm_) {
    self->shm_ = static_cast<wl_shm *>(wl_registry_bind(
        registry, name, &wl_shm_interface, 1));
  } else if (std::strcmp(interface, "wl_seat") == 0 && !self->seat_) {
    self->seat_ = static_cast<wl_seat *>(wl_registry_bind(
        registry, name, &wl_seat_interface, std::min(version, 5u)));
    static const wl_seat_listener seat_listener = {seat_capabilities, seat_name};
    wl_seat_add_listener(self->seat_, &seat_listener, self);
  } else if (std::strcmp(interface, "zwlr_layer_shell_v1") == 0 && !self->layer_shell_) {
    self->layer_shell_ = static_cast<zwlr_layer_shell_v1 *>(wl_registry_bind(
        registry, name, &zwlr_layer_shell_v1_interface, std::min(version, 4u)));
  }
}

void WaveOverlayWaylandSurface::registry_remove(void *data, wl_registry *, uint32_t name) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  for (auto output = self->outputs_.begin(); output != self->outputs_.end(); ++output) {
    if (output->name != name) continue;
    self->entered_.erase(std::remove(self->entered_.begin(), self->entered_.end(), output->output),
                         self->entered_.end());
    wl_output_destroy(output->output);
    self->outputs_.erase(output);
    self->scale_changed_ = true;
    return;
  }
}

void WaveOverlayWaylandSurface::seat_capabilities(void *data, wl_seat *seat,
                                                   uint32_t capabilities) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  const bool pointer_available = (capabilities & WL_SEAT_CAPABILITY_POINTER) != 0;
  if (pointer_available && !self->pointer_) {
    self->pointer_ = wl_seat_get_pointer(seat);
    // 按成员逐个赋值，而不是按位置聚合初始化。wl_pointer_listener 会随协议版本增加
    // 成员（wayland-client 1.23 起有 axis_relative_direction，1.24 起有 warp），按
    // 位置写死的初始化列表在新头文件上触发 -Werror=missing-field-initializers，整个
    // Linux 宿主在装有新 wayland 的机器上就构建不出来；而把新成员补进列表又会让旧头
    // 文件编不过。先值初始化再逐个赋值对两边都成立，未知成员保持空指针。
    static const wl_pointer_listener pointer_listener = [] {
      wl_pointer_listener listener{};
      listener.enter = pointer_enter;
      listener.leave = pointer_leave;
      listener.motion = pointer_motion;
      listener.button = pointer_button;
      listener.axis = pointer_axis;
      listener.frame = pointer_frame;
      listener.axis_source = pointer_axis_source;
      listener.axis_stop = pointer_axis_stop;
      listener.axis_discrete = pointer_axis_discrete;
      // axis_value120 是 wl_pointer 第 8 版的事件，wayland-client 1.21 起的头文件才有这个成员；Debian 10 的 1.16（legacy 包）没有，按生成头文件里的版本宏条件编译。
#ifdef WL_POINTER_AXIS_VALUE120_SINCE_VERSION
      listener.axis_value120 = pointer_axis_value120;
#endif
      return listener;
    }();
    wl_pointer_add_listener(self->pointer_, &pointer_listener, self);
  } else if (!pointer_available && self->pointer_) {
    release_pointer(self->pointer_);
    self->pointer_ = nullptr;
    self->pointer_inside_ = false;
    self->action_pressed_ = false;
  }
}

void WaveOverlayWaylandSurface::seat_name(void *, wl_seat *, const char *) {}

void WaveOverlayWaylandSurface::pointer_enter(void *data, wl_pointer *,
                                               uint32_t, wl_surface *surface,
                                               wl_fixed_t x, wl_fixed_t y) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  if (surface != self->surface_)
    return;
  self->pointer_inside_ = true;
  self->pointer_x_ = wl_fixed_to_double(x);
  self->pointer_y_ = wl_fixed_to_double(y);
}

void WaveOverlayWaylandSurface::pointer_leave(void *data, wl_pointer *,
                                               uint32_t, wl_surface *surface) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  if (surface == self->surface_)
    self->pointer_inside_ = false;
  self->action_pressed_ = false;
}

void WaveOverlayWaylandSurface::pointer_motion(void *data, wl_pointer *,
                                                uint32_t, wl_fixed_t x,
                                                wl_fixed_t y) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  self->pointer_x_ = wl_fixed_to_double(x);
  self->pointer_y_ = wl_fixed_to_double(y);
}

void WaveOverlayWaylandSurface::pointer_button(void *data, wl_pointer *,
                                                uint32_t, uint32_t,
                                                uint32_t button, uint32_t state) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  constexpr uint32_t kLeftButton = 0x110;
  if (button != kLeftButton || !self->pointer_inside_)
    return;
  WaveOverlayModel::Action action;
  if (state == WL_POINTER_BUTTON_STATE_PRESSED &&
      wave_overlay_hit_action(self->model_, self->pointer_x_, self->pointer_y_, self->frame_.width,
                              self->frame_.height, action)) {
    self->pressed_action_ = action;
    self->action_pressed_ = true;
  } else if (state == WL_POINTER_BUTTON_STATE_RELEASED &&
             self->action_pressed_) {
    const bool activated = wave_overlay_hit_action(self->model_, self->pointer_x_, self->pointer_y_,
                                                   self->frame_.width, self->frame_.height, action) &&
                           action == self->pressed_action_;
    self->action_pressed_ = false;
    if (activated && self->action_handler_)
      self->action_handler_(action);
  }
}

void WaveOverlayWaylandSurface::pointer_axis(void *, wl_pointer *, uint32_t,
                                              uint32_t, wl_fixed_t) {}
void WaveOverlayWaylandSurface::pointer_frame(void *, wl_pointer *) {}
void WaveOverlayWaylandSurface::pointer_axis_source(void *, wl_pointer *,
                                                    uint32_t) {}
void WaveOverlayWaylandSurface::pointer_axis_stop(void *, wl_pointer *,
                                                  uint32_t, uint32_t) {}
void WaveOverlayWaylandSurface::pointer_axis_discrete(void *, wl_pointer *,
                                                      uint32_t, int32_t) {}
void WaveOverlayWaylandSurface::pointer_axis_value120(void *, wl_pointer *,
                                                      uint32_t, int32_t) {}

void WaveOverlayWaylandSurface::output_geometry(void *, wl_output *, int32_t, int32_t, int32_t, int32_t,
                                                int32_t, const char *, const char *, int32_t) {}
void WaveOverlayWaylandSurface::output_mode(void *, wl_output *, uint32_t, int32_t, int32_t, int32_t) {}
void WaveOverlayWaylandSurface::output_done(void *, wl_output *) {}

void WaveOverlayWaylandSurface::output_scale(void *data, wl_output *output, int32_t factor) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  for (auto &known : self->outputs_)
    if (known.output == output && known.scale != factor) {
      known.scale = std::max(1, factor);
      self->scale_changed_ = true;
    }
}

void WaveOverlayWaylandSurface::surface_enter(void *data, wl_surface *, wl_output *output) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  if (std::find(self->entered_.begin(), self->entered_.end(), output) == self->entered_.end())
    self->entered_.push_back(output);
  self->scale_changed_ = true;
}

void WaveOverlayWaylandSurface::surface_leave(void *data, wl_surface *, wl_output *output) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  self->entered_.erase(std::remove(self->entered_.begin(), self->entered_.end(), output), self->entered_.end());
  self->scale_changed_ = true;
}

// 布局尺寸用 `set_size` 告知合成器，合成器选定的尺寸仍按告知的尺寸绘制：浮条小而居中，合成器选了别的尺寸也能完整显示。
void WaveOverlayWaylandSurface::layer_configure(
    void *data, zwlr_layer_surface_v1 *surface, uint32_t serial, uint32_t,
    uint32_t) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  zwlr_layer_surface_v1_ack_configure(surface, serial);
  self->configured_ = true;
}

void WaveOverlayWaylandSurface::layer_closed(void *data,
                                             zwlr_layer_surface_v1 *) {
  static_cast<WaveOverlayWaylandSurface *>(data)->closed_ = true;
}

void WaveOverlayWaylandSurface::buffer_release(void *data, wl_buffer *) {
  auto *context = static_cast<BufferContext *>(data);
  context->owner->release_buffer(context->index);
}

WaveOverlayWaylandSurface::~WaveOverlayWaylandSurface() { destroy_surface(); }

bool WaveOverlayWaylandSurface::ensure_surface() {
  if (closed_)
    destroy_surface();
  if (display_ && surface_ && !closed_)
    return configured_ || configure_size(true);
  if (!display_) {
    const auto *socket = std::getenv("WAYLAND_DISPLAY");
    if (!socket || !*socket)
      return false;
    display_ = wl_display_connect(nullptr);
    if (!display_)
      return false;
    registry_ = wl_display_get_registry(display_);
    static const wl_registry_listener registry_listener = {
        registry_global, registry_remove};
    wl_registry_add_listener(registry_, &registry_listener, this);
    if (wl_display_roundtrip(display_) < 0 || !compositor_ || !shm_ || !layer_shell_) {
      destroy_surface();
      return false;
    }
    surface_ = wl_compositor_create_surface(compositor_);
    static const wl_surface_listener surface_listener = [] {
      wl_surface_listener listener{};
      listener.enter = surface_enter;
      listener.leave = surface_leave;
      return listener;
    }();
    wl_surface_add_listener(surface_, &surface_listener, this);
    layer_surface_ = zwlr_layer_shell_v1_get_layer_surface(
        layer_shell_, surface_, nullptr, ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY,
        "msime-linux-wave-overlay");
    static const zwlr_layer_surface_v1_listener layer_listener = {
        layer_configure, layer_closed};
    zwlr_layer_surface_v1_add_listener(layer_surface_, &layer_listener, this);
    // 只设底部锚点时，layer-shell 把浮层水平居中放在合成器选定的 output 上（通常是有焦点的那个）。独占区设为 0 让它避开预留空间的面板，10 px 边距因此从可用区域量起，就像 X11 浮条从工作区量起；它自己不预留空间，也从不请求键盘焦点。
    zwlr_layer_surface_v1_set_anchor(
        layer_surface_, ZWLR_LAYER_SURFACE_V1_ANCHOR_BOTTOM);
    zwlr_layer_surface_v1_set_margin(layer_surface_, 0, 0, kWaveOverlayWorkAreaGap, 0);
    zwlr_layer_surface_v1_set_exclusive_zone(layer_surface_, 0);
    zwlr_layer_surface_v1_set_keyboard_interactivity(
        layer_surface_, ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_NONE);
    if (!configure_size(true)) {
      destroy_surface();
      return false;
    }
  }
  return true;
}

// 用不带缓冲区的 commit 请求当前布局的尺寸，并等待合成器的 configure，这是 layer-shell 在第一个缓冲区之前、以及浮层取消映射后再次显示时（`map`）的要求。已映射时布局变化也这样请求，新缓冲区通常跟在已确认的尺寸之后；合成器对尺寸变化不回 configure 时，浮层仍按上一次的尺寸保持映射，绘制照常进行。
bool WaveOverlayWaylandSurface::configure_size(bool map) {
  configured_ = false;
  requested_width_ = frame_.width;
  requested_height_ = frame_.height;
  zwlr_layer_surface_v1_set_size(layer_surface_, static_cast<uint32_t>(requested_width_),
                                 static_cast<uint32_t>(requested_height_));
  wl_surface_commit(surface_);
  if (wl_display_roundtrip(display_) < 0)
    closed_ = true;
  // roundtrip 也会运行指针处理函数，期间某个操作可能已经收起（取消映射）了浮条；不论 roundtrip 期间收到了什么 configure，这样的浮层在下一次 `show()` 时都需要完整的映射握手。
  if (!map && !closed_)
    configured_ = visible_;
  return configured_ && !closed_;
}

// 只有两个圆形操作按钮接收指针；浮条的其余部分让点击穿过。
void WaveOverlayWaylandSurface::set_input_region() {
  if (!surface_ || !compositor_)
    return;
  if (input_width_ == frame_.width && input_actions_ == frame_.actions)
    return;
  auto *region = wl_compositor_create_region(compositor_);
  if (!region)
    return;
  input_width_ = frame_.width;
  input_actions_ = frame_.actions;
  if (frame_.actions) {
    const auto radius = static_cast<int>(std::ceil(kWaveOverlayActionRadius));
    const auto center_y = static_cast<int>(std::lround(wave_overlay_action_center_y(frame_.height)));
    for (const double center_x : {wave_overlay_cancel_center_x(), wave_overlay_confirm_center_x(frame_.width)})
      wl_region_add(region, static_cast<int>(std::lround(center_x)) - radius, center_y - radius, 2 * radius,
                    2 * radius);
  }
  wl_surface_set_input_region(surface_, region);
  wl_region_destroy(region);
}

void WaveOverlayWaylandSurface::pump_events() {
  if (!display_)
    return;
  if (wl_display_dispatch_pending(display_) < 0) {
    closed_ = true;
    return;
  }
  if (wl_display_prepare_read(display_) != 0)
    return;
  const auto flush_result = wl_display_flush(display_);
  if (flush_result < 0 && errno != EAGAIN) {
    wl_display_cancel_read(display_);
    closed_ = true;
    return;
  }
  pollfd descriptor{wl_display_get_fd(display_), POLLIN, 0};
  const int ready = poll(&descriptor, 1, 0);
  if (ready > 0 && (descriptor.revents & POLLIN)) {
    if (wl_display_read_events(display_) < 0)
      closed_ = true;
  } else {
    wl_display_cancel_read(display_);
  }
  if (!closed_ && wl_display_dispatch_pending(display_) < 0)
    closed_ = true;
  // 移到缩放不同的 output 上时立即重画，浮条显示静止标签时也一样。
  if (scale_changed_ && visible_ && !closed_) {
    scale_changed_ = false;
    draw();
  }
}

// 两个 `width` x `height` 设备像素的缓冲区，布局或缩放改变尺寸时重建。
bool WaveOverlayWaylandSurface::ensure_buffers(int width, int height) {
  if (buffers_[0] && buffers_[1] && width == buffer_width_ && height == buffer_height_)
    return true;
  destroy_buffers();
  const auto stride = width * 4;
  const auto bytes = static_cast<std::size_t>(stride) * static_cast<std::size_t>(height);
  shm_fd_ = create_shm_file();
  if (shm_fd_ < 0 || ftruncate(shm_fd_, static_cast<off_t>(bytes * 2)) != 0) {
    if (shm_fd_ >= 0)
      close(shm_fd_);
    shm_fd_ = -1;
    return false;
  }
  buffer_size_ = bytes * 2;
  auto *mapped = mmap(nullptr, buffer_size_, PROT_READ | PROT_WRITE, MAP_SHARED, shm_fd_, 0);
  if (mapped == MAP_FAILED) {
    close(shm_fd_);
    shm_fd_ = -1;
    buffer_size_ = 0;
    return false;
  }
  pixels_[0] = mapped;
  auto *pool = wl_shm_create_pool(shm_, shm_fd_, static_cast<int>(buffer_size_));
  if (!pool) {
    destroy_buffers();
    return false;
  }
  static const wl_buffer_listener buffer_listener = {buffer_release};
  for (std::size_t index = 0; index < buffers_.size(); ++index) {
    pixels_[index] = static_cast<std::byte *>(mapped) + index * bytes;
    buffers_[index] = wl_shm_pool_create_buffer(pool, static_cast<int>(index * bytes), width, height, stride,
                                                WL_SHM_FORMAT_ARGB8888);
    if (!buffers_[index]) {
      wl_shm_pool_destroy(pool);
      destroy_buffers();
      return false;
    }
    buffer_contexts_[index] = {this, index};
    wl_buffer_add_listener(buffers_[index], &buffer_listener, &buffer_contexts_[index]);
  }
  wl_shm_pool_destroy(pool);
  buffer_width_ = width;
  buffer_height_ = height;
  return true;
}

// 合成器仍持有的缓冲区在销毁后对它那一侧依然有效：shm 内容由合成器复制或保留，直到它释放。
void WaveOverlayWaylandSurface::destroy_buffers() {
  for (auto *buffer : buffers_)
    if (buffer)
      wl_buffer_destroy(buffer);
  if (pixels_[0] && buffer_size_)
    munmap(pixels_[0], buffer_size_);
  if (shm_fd_ >= 0)
    close(shm_fd_);
  buffers_.fill(nullptr);
  pixels_.fill(nullptr);
  buffer_busy_.fill(false);
  shm_fd_ = -1;
  buffer_size_ = 0;
  buffer_width_ = 0;
  buffer_height_ = 0;
}

void WaveOverlayWaylandSurface::release_buffer(std::size_t index) {
  if (index < buffer_busy_.size())
    buffer_busy_[index] = false;
}

// 浮层所在各 output 中最大的整数缩放；合成器还没告知它在哪里时，取全部 output 中最大的。`wl_surface.set_buffer_scale` 需要版本 3 的合成器，更旧的合成器拿到 1x 缓冲区。
int WaveOverlayWaylandSurface::buffer_scale() const {
  if (compositor_version_ < 3)
    return 1;
  int scale = 1;
  for (const auto &output : outputs_) {
    const bool on_it = std::find(entered_.begin(), entered_.end(), output.output) != entered_.end();
    if (on_it || entered_.empty())
      scale = std::max(scale, static_cast<int>(output.scale));
  }
  return scale;
}

void WaveOverlayWaylandSurface::draw() {
  if (!visible_ || !surface_ || !configured_ || closed_)
    return;
  if (frame_.width != requested_width_ || frame_.height != requested_height_) {
    // roundtrip 也会分发指针事件，期间某个操作可能收起浮条。
    if (!configure_size(false) || !visible_)
      return;
  }
  const int scale = buffer_scale();
  const int width = frame_.width * scale;
  const int height = frame_.height * scale;
  if (!ensure_buffers(width, height))
    return;
  std::size_t index = next_buffer_++ % buffers_.size();
  if (buffer_busy_[index]) {
    index = (index + 1) % buffers_.size();
    if (buffer_busy_[index])
      return;
  }
  set_input_region();
  if (!frame_.actions)
    action_pressed_ = false;
  auto *image = cairo_image_surface_create_for_data(static_cast<unsigned char *>(pixels_[index]),
                                                    CAIRO_FORMAT_ARGB32, width, height, width * 4);
  auto *cairo = cairo_create(image);
  cairo_scale(cairo, scale, scale);
  painter_.paint(cairo, frame_, model_, true);
  cairo_destroy(cairo);
  cairo_surface_flush(image);
  cairo_surface_destroy(image);
  if (compositor_version_ >= 3)
    wl_surface_set_buffer_scale(surface_, scale);
  wl_surface_attach(surface_, buffers_[index], 0, 0);
  wl_surface_damage(surface_, 0, 0, frame_.width, frame_.height);
  wl_surface_commit(surface_);
  buffer_busy_[index] = true;
  wl_display_flush(display_);
}

bool WaveOverlayWaylandSurface::show(const WaveOverlayModel &model) {
  model_ = model;
  frame_ = painter_.frame(model_);
  if (!ensure_surface())
    return false;
  visible_ = true;
  draw();
  pump_events();
  return true;
}

void WaveOverlayWaylandSurface::update(const WaveOverlayModel &model) {
  if (!display_ || closed_ || !visible_)
    return;
  model_ = model;
  frame_ = painter_.frame(model_);
  pump_events();
  draw();
}

// 附加空缓冲区会取消映射 layer surface；下一次 `show()` 绘制之前会重新请求尺寸，这是 layer-shell 对重新映射的浮层的要求。
void WaveOverlayWaylandSurface::hide() {
  if (display_ && surface_ && visible_) {
    wl_surface_attach(surface_, nullptr, 0, 0);
    wl_surface_commit(surface_);
    wl_display_flush(display_);
    configured_ = false;
  }
  visible_ = false;
  action_pressed_ = false;
  pointer_inside_ = false;
}

int WaveOverlayWaylandSurface::event_fd() const {
  return display_ && visible_ && !closed_ ? wl_display_get_fd(display_) : -1;
}

void WaveOverlayWaylandSurface::dispatch_events() { pump_events(); }

void WaveOverlayWaylandSurface::destroy_surface() {
  if (display_)
    wl_display_roundtrip(display_);
  destroy_buffers();
  if (layer_surface_)
    zwlr_layer_surface_v1_destroy(layer_surface_);
  if (surface_)
    wl_surface_destroy(surface_);
  if (layer_shell_)
    zwlr_layer_shell_v1_destroy(layer_shell_);
  if (registry_)
    wl_registry_destroy(registry_);
  if (compositor_)
    wl_compositor_destroy(compositor_);
  if (shm_)
    wl_shm_destroy(shm_);
  for (const auto &output : outputs_)
    wl_output_destroy(output.output);
  outputs_.clear();
  entered_.clear();
  // After the roundtrip, which may still create pointer_; a kept seat would never rebind on reconnect.
  if (pointer_)
    release_pointer(pointer_);
  if (seat_) {
    if (wl_seat_get_version(seat_) >= WL_SEAT_RELEASE_SINCE_VERSION)
      wl_seat_release(seat_);
    else
      wl_seat_destroy(seat_);
  }
  if (display_)
    wl_display_disconnect(display_);
  display_ = nullptr;
  registry_ = nullptr;
  compositor_ = nullptr;
  shm_ = nullptr;
  layer_shell_ = nullptr;
  surface_ = nullptr;
  layer_surface_ = nullptr;
  seat_ = nullptr;
  pointer_ = nullptr;
  pointer_inside_ = false;
  action_pressed_ = false;
  compositor_version_ = 0;
  requested_width_ = 0;
  requested_height_ = 0;
  input_width_ = -1;
  input_actions_ = false;
  scale_changed_ = false;
  configured_ = false;
  visible_ = false;
  closed_ = false;
}

}  // namespace msime::linux_host

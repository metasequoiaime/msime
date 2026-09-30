#include "WaveOverlayWaylandSurface.h"

#include "WaveOverlayPlacement.h"

#include "wlr-layer-shell-unstable-v1-client-protocol.h"

#include <wayland-client.h>

#ifdef MSIME_LINUX_WAYLAND_TEXT
#include <cairo/cairo.h>
#include <pango/pangocairo.h>
#endif

#include <algorithm>
#include <cerrno>
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

constexpr int kWidth = 420;
constexpr int kHeight = 132;
constexpr int kStride = kWidth * 4;
constexpr std::size_t kBufferBytes = static_cast<std::size_t>(kStride) * kHeight;
constexpr int kActionCenterInset = 24;
constexpr int kActionCenterY = 56;
constexpr int kActionRadius = 14;

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
    self->compositor_ = static_cast<wl_compositor *>(wl_registry_bind(
        registry, name, &wl_compositor_interface, std::min(version, 4u)));
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

void WaveOverlayWaylandSurface::registry_remove(void *, wl_registry *, uint32_t) {}

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
      listener.axis_value120 = pointer_axis_value120;
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
  self->pointer_x_ = wl_fixed_to_int(x);
  self->pointer_y_ = wl_fixed_to_int(y);
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
  self->pointer_x_ = wl_fixed_to_int(x);
  self->pointer_y_ = wl_fixed_to_int(y);
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
      self->hit_test_action(self->pointer_x_, self->pointer_y_, action)) {
    self->pressed_action_ = action;
    self->action_pressed_ = true;
  } else if (state == WL_POINTER_BUTTON_STATE_RELEASED &&
             self->action_pressed_) {
    const bool activated = self->hit_test_action(
                               self->pointer_x_, self->pointer_y_, action) &&
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

void WaveOverlayWaylandSurface::layer_configure(
    void *data, zwlr_layer_surface_v1 *surface, uint32_t serial, uint32_t width,
    uint32_t height) {
  auto *self = static_cast<WaveOverlayWaylandSurface *>(data);
  zwlr_layer_surface_v1_ack_configure(surface, serial);
  if (width != 0 && height != 0 && (width != kWidth || height != kHeight)) {
    // The fixed-size surface is intentionally advertised; a compositor may
    // still choose a different size, but the next frame remains bounded.
  }
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
  if (display_ && surface_ && configured_ && !closed_)
    return true;
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
    layer_surface_ = zwlr_layer_shell_v1_get_layer_surface(
        layer_shell_, surface_, nullptr, ZWLR_LAYER_SHELL_V1_LAYER_OVERLAY,
        "msime-linux-wave-overlay");
    static const zwlr_layer_surface_v1_listener layer_listener = {
        layer_configure, layer_closed};
    zwlr_layer_surface_v1_add_listener(layer_surface_, &layer_listener, this);
    zwlr_layer_surface_v1_set_size(layer_surface_, kWidth, kHeight);
    // With only the bottom anchor set, layer-shell centers the fixed-width
    // surface horizontally while keeping it above the work area's bottom
    // edge. It remains output-independent and never requests keyboard focus.
    zwlr_layer_surface_v1_set_anchor(
        layer_surface_, ZWLR_LAYER_SURFACE_V1_ANCHOR_BOTTOM);
    zwlr_layer_surface_v1_set_margin(layer_surface_, 0,
                                     kWaveOverlayBottomMargin, 0, 0);
    zwlr_layer_surface_v1_set_exclusive_zone(layer_surface_, -1);
    zwlr_layer_surface_v1_set_keyboard_interactivity(
        layer_surface_, ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_NONE);
    wl_surface_commit(surface_);
    if (wl_display_roundtrip(display_) < 0 || !configured_ || closed_) {
      destroy_surface();
      return false;
    }
  }
  return ensure_buffers();
}

void WaveOverlayWaylandSurface::set_input_region(bool actions_visible) {
  if (!surface_ || !compositor_)
    return;
  auto *region = wl_compositor_create_region(compositor_);
  if (!region)
    return;
  if (actions_visible) {
    wl_region_add(region, kActionCenterInset - kActionRadius,
                  kActionCenterY - kActionRadius, 2 * kActionRadius,
                  2 * kActionRadius);
    wl_region_add(region, kWidth - kActionCenterInset - kActionRadius,
                  kActionCenterY - kActionRadius, 2 * kActionRadius,
                  2 * kActionRadius);
  }
  wl_surface_set_input_region(surface_, region);
  wl_region_destroy(region);
}

bool WaveOverlayWaylandSurface::hit_test_action(
    int x, int y, WaveOverlayModel::Action &action) const {
  if (!actions_visible_)
    return false;
  const auto inside = [y](int center_x, int point_x) {
    const int dx = point_x - center_x;
    const int dy = y - kActionCenterY;
    return dx * dx + dy * dy <= kActionRadius * kActionRadius;
  };
  if (inside(kActionCenterInset, x)) {
    action = WaveOverlayModel::Action::Cancel;
    return true;
  }
  if (inside(kWidth - kActionCenterInset, x)) {
    action = WaveOverlayModel::Action::Confirm;
    return true;
  }
  return false;
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
}

bool WaveOverlayWaylandSurface::ensure_buffers() {
  if (buffers_[0] && buffers_[1])
    return true;
  shm_fd_ = create_shm_file();
  if (shm_fd_ < 0 || ftruncate(shm_fd_, static_cast<off_t>(kBufferBytes * 2)) != 0) {
    if (shm_fd_ >= 0)
      close(shm_fd_);
    shm_fd_ = -1;
    return false;
  }
  buffer_size_ = kBufferBytes * 2;
  auto *mapped = mmap(nullptr, buffer_size_, PROT_READ | PROT_WRITE, MAP_SHARED, shm_fd_, 0);
  if (mapped == MAP_FAILED) {
    close(shm_fd_);
    shm_fd_ = -1;
    return false;
  }
  auto *pool = wl_shm_create_pool(shm_, shm_fd_, static_cast<int>(buffer_size_));
  if (!pool) {
    munmap(mapped, buffer_size_);
    close(shm_fd_);
    shm_fd_ = -1;
    buffer_size_ = 0;
    return false;
  }
  static const wl_buffer_listener buffer_listener = {buffer_release};
  for (std::size_t index = 0; index < buffers_.size(); ++index) {
    pixels_[index] = static_cast<std::byte *>(mapped) + index * kBufferBytes;
    buffers_[index] = wl_shm_pool_create_buffer(
        pool, static_cast<int>(index * kBufferBytes), kWidth, kHeight, kStride,
        WL_SHM_FORMAT_ARGB8888);
    if (!buffers_[index]) {
      wl_shm_pool_destroy(pool);
      munmap(mapped, buffer_size_);
      close(shm_fd_);
      shm_fd_ = -1;
      buffer_size_ = 0;
      buffers_.fill(nullptr);
      pixels_.fill(nullptr);
      return false;
    }
    buffer_contexts_[index] = {this, index};
    wl_buffer_add_listener(buffers_[index], &buffer_listener,
                           &buffer_contexts_[index]);
  }
  wl_shm_pool_destroy(pool);
  return true;
}

void WaveOverlayWaylandSurface::release_buffer(std::size_t index) {
  if (index < buffer_busy_.size())
    buffer_busy_[index] = false;
}

void WaveOverlayWaylandSurface::draw(const WaveOverlayModel &model) {
  if (!visible_ || !surface_ || !configured_)
    return;
  std::size_t index = next_buffer_++ % buffers_.size();
  if (buffer_busy_[index]) {
    index = (index + 1) % buffers_.size();
    if (buffer_busy_[index])
      return;
  }
  auto *pixels = static_cast<uint32_t *>(pixels_[index]);
  actions_visible_ = model.actions_visible;
  set_input_region(model.actions_visible);
  if (!model.actions_visible)
    action_pressed_ = false;
  auto background = model.light_theme ? 0xE6F5F7FAu : 0xE6202124u;
  auto border = model.light_theme ? 0xFFE0E4EAu : 0xFF30343Bu;
  auto foreground = model.light_theme ? 0xFF202124u : 0xFFF5F7FAu;
  auto accent = model.light_theme ? 0xFF3367D6u : 0xFF73A7FFu;
  auto listening_color = 0xFF73A7FFu;
  // The resolved theme's palette when the frontend gave one: its surface (at the same translucency) for the bar, its outline (else the surface) for the frame, its text and its accent, which also colours the level bars while listening. The locked and processing colours are states, not theme colours, and stay.
  if (model.palette) {
    background = 0xE6000000u | model.palette->surface;
    border = 0xFF000000u | model.palette->border.value_or(model.palette->surface);
    foreground = 0xFF000000u | model.palette->text;
    accent = 0xFF000000u | model.palette->accent;
    listening_color = accent;
  }
  std::fill(pixels, pixels + kWidth * kHeight, background);
  const auto status_color = model.locked ? 0xFFFFC857u
                           : model.compact_status == WaveOverlayModel::CompactStatus::Processing
                               ? 0xFFFF8A65u
                               : listening_color;
  for (int y = 12; y < 64; ++y)
    for (int x = 0; x < kWidth; ++x)
      if (y < 16 || y >= 60 || x < 12 || x >= kWidth - 12)
        pixels[y * kWidth + x] = border;
  const auto width = (kWidth - 48) / 12;
  for (std::size_t index_bar = 0; index_bar < model.levels.size(); ++index_bar) {
    const auto height = std::max(4, static_cast<int>(model.levels[index_bar] * 38.0f));
    for (int y = 56 - height / 2; y < 56 + height / 2; ++y)
      for (int x = 0; x < width - 3; ++x)
        pixels[y * kWidth + 24 + static_cast<int>(index_bar) * width + x] = status_color;
  }
  const auto transcript_marker = std::min<std::size_t>(kWidth - 48, model.transcript.size());
  for (std::size_t x = 0; x < transcript_marker; ++x)
    pixels[112 * kWidth + 24 + x] = 0xFF9AA4B2u;
#ifdef MSIME_LINUX_WAYLAND_TEXT
  auto *image = cairo_image_surface_create_for_data(
      reinterpret_cast<unsigned char *>(pixels), CAIRO_FORMAT_ARGB32,
      kWidth, kHeight, kStride);
  auto *cairo = cairo_create(image);
  const auto set_rgb = [cairo](uint32_t rgb, double alpha) {
    cairo_set_source_rgba(cairo, ((rgb >> 16) & 0xff) / 255.0, ((rgb >> 8) & 0xff) / 255.0, (rgb & 0xff) / 255.0,
                          alpha);
  };
  if (model.palette)
    set_rgb(model.palette->border.value_or(model.palette->surface), 1.0);
  else
    cairo_set_source_rgb(cairo, model.light_theme ? 0.96 : 0.125,
                         model.light_theme ? 0.97 : 0.13,
                         model.light_theme ? 0.98 : 0.145);
  cairo_paint(cairo);
  if (model.palette)
    set_rgb(model.palette->surface, 0.9);
  else
    cairo_set_source_rgba(cairo, model.light_theme ? 1.0 : 0.125,
                          model.light_theme ? 1.0 : 0.13,
                          model.light_theme ? 1.0 : 0.145, 0.9);
  cairo_rectangle(cairo, 12, 12, kWidth - 24, 108);
  cairo_fill(cairo);
  if (model.palette && !model.locked)
    set_rgb(model.palette->accent, 1.0);
  else
    cairo_set_source_rgba(cairo, model.locked ? 1.0 : 0.45,
                          model.locked ? 0.78 : 0.65,
                          model.locked ? 0.34 : 1.0, 1.0);
  for (std::size_t index_bar = 0; index_bar < model.levels.size(); ++index_bar) {
    const auto height = std::max(4, static_cast<int>(model.levels[index_bar] * 38.0f));
    cairo_rectangle(cairo, 24 + static_cast<int>(index_bar) * width,
                    56 - height / 2, width - 3, height);
  }
  cairo_fill(cairo);
  auto *layout = pango_cairo_create_layout(cairo);
  auto *font = pango_font_description_from_string("Sans 12");
  pango_layout_set_font_description(layout, font);
  pango_font_description_free(font);
  std::string status = model.locked
                           ? "录音已锁定 · 再按快捷键结束 · Esc 取消"
                           : model.status;
  if (status.empty())
    status = "正在录音…";
  pango_layout_set_text(layout, status.c_str(), -1);
  if (model.palette)
    set_rgb(model.palette->text, 1.0);
  else
    cairo_set_source_rgb(cairo, model.light_theme ? 0.125 : 0.96,
                         model.light_theme ? 0.13 : 0.97,
                         model.light_theme ? 0.145 : 0.98);
  cairo_move_to(cairo, model.actions_visible ? 52 : 24, 70);
  pango_cairo_show_layout(cairo, layout);
  if (model.show_transcript && !model.transcript.empty()) {
    auto transcript = model.transcript;
    if (transcript.size() > 240)
      transcript.resize(240);
    pango_layout_set_text(layout, transcript.c_str(), -1);
    cairo_move_to(cairo, 24, 100);
    pango_cairo_show_layout(cairo, layout);
  }
  g_object_unref(layout);
  cairo_destroy(cairo);
  cairo_surface_destroy(image);
#endif
  if (model.actions_visible) {
    const auto fill_circle = [pixels](int center_x, uint32_t value) {
      for (int y = -kActionRadius; y <= kActionRadius; ++y)
        for (int x = -kActionRadius; x <= kActionRadius; ++x)
          if (x * x + y * y <= kActionRadius * kActionRadius)
            pixels[(kActionCenterY + y) * kWidth + center_x + x] = value;
    };
    fill_circle(kActionCenterInset, accent);
    fill_circle(kWidth - kActionCenterInset, accent);
    for (int offset = -5; offset <= 5; ++offset) {
      pixels[(kActionCenterY + offset) * kWidth + kActionCenterInset + offset] =
          foreground;
      pixels[(kActionCenterY + offset) * kWidth + kActionCenterInset - offset] =
          foreground;
    }
    for (int offset = -4; offset <= 4; ++offset) {
      pixels[(kActionCenterY + offset) * kWidth + kWidth - kActionCenterInset + offset / 2] =
          0xFF202124u;
      pixels[(kActionCenterY + offset) * kWidth + kWidth - kActionCenterInset + 5 - offset / 2] =
          0xFF202124u;
    }
  }
  wl_surface_attach(surface_, buffers_[index], 0, 0);
  wl_surface_damage(surface_, 0, 0, kWidth, kHeight);
  wl_surface_commit(surface_);
  buffer_busy_[index] = true;
  wl_display_flush(display_);
}

bool WaveOverlayWaylandSurface::show(const WaveOverlayModel &model) {
  if (!ensure_surface())
    return false;
  visible_ = true;
  actions_visible_ = model.actions_visible;
  draw(model);
  pump_events();
  return true;
}

void WaveOverlayWaylandSurface::update(const WaveOverlayModel &model) {
  if (!display_ || closed_)
    return;
  actions_visible_ = model.actions_visible;
  pump_events();
  draw(model);
}

void WaveOverlayWaylandSurface::hide() {
  if (display_ && surface_ && visible_) {
    wl_surface_attach(surface_, nullptr, 0, 0);
    wl_surface_damage(surface_, 0, 0, kWidth, kHeight);
    wl_surface_commit(surface_);
    wl_display_flush(display_);
  }
  visible_ = false;
  action_pressed_ = false;
  pointer_inside_ = false;
}

void WaveOverlayWaylandSurface::destroy_surface() {
  if (display_)
    wl_display_roundtrip(display_);
  for (auto *buffer : buffers_)
    if (buffer)
      wl_buffer_destroy(buffer);
  if (pixels_[0] && buffer_size_)
    munmap(pixels_[0], buffer_size_);
  if (shm_fd_ >= 0)
    close(shm_fd_);
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
  buffers_.fill(nullptr);
  pixels_.fill(nullptr);
  buffer_busy_.fill(false);
  shm_fd_ = -1;
  buffer_size_ = 0;
  configured_ = false;
  visible_ = false;
  closed_ = false;
}

}  // namespace msime::linux_host

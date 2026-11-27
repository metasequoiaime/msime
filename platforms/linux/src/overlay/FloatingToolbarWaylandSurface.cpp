#include "FloatingToolbarWaylandSurface.h"

#include <wayland-client.h>

#include <linux/input-event-codes.h>
#include <sys/mman.h>
#include <unistd.h>

#include <algorithm>
#include <array>
#include <cerrno>
#include <cmath>
#include <string_view>
#include <utility>

#include "FloatingToolbarPainter.h"
#include "wlr-layer-shell-unstable-v1-client-protocol.h"

namespace msime::linux_host {
namespace {

struct Output {
  wl_output *object;
  uint32_t id;
  int width = 0;
  int height = 0;
  int scale = 1;
  int transform = WL_OUTPUT_TRANSFORM_NORMAL;

  int logical_width() const { return (transform % 2 ? height : width) / scale; }
  int logical_height() const { return (transform % 2 ? width : height) / scale; }
};

class WaylandToolbar final : public FloatingToolbarSurface {
 public:
  WaylandToolbar(OverlayLoop loop, Callbacks callbacks)
      : loop_(std::move(loop)), callbacks_(std::move(callbacks)) {}
  ~WaylandToolbar() override { disconnect(); }

  bool connect();
  void present(const Content &content, std::optional<WaveOverlayPosition> saved) override;
  void hide() override;
  bool visible() const override { return connected() && layer_ && !closed_; }
  bool connected() const override { return display_ && !wl_display_get_error(display_); }
  bool foreground_fullscreen() override { return false; }

 private:
  struct Buffer {
    WaylandToolbar *owner = nullptr;
    wl_buffer *object = nullptr;
    cairo_surface_t *image = nullptr;
    void *pixels = nullptr;
    std::size_t bytes = 0;
    int width = 0;
    int height = 0;
    bool busy = false;

    void clear();
    bool allocate(int w, int h);
  };

  void disconnect();
  void add_global(wl_registry *registry, uint32_t id, const char *name, uint32_t version);
  void remove_global(uint32_t id);
  Output &output(wl_output *object);
  void update_pointer(bool available);
  void change_pointer(double x, double y);
  void click(uint32_t button, bool down);
  void update_layout();
  void redraw();
  void open_menu(std::vector<FloatingToolbarMenuItem> items);
  void flush();

  OverlayLoop loop_;
  Callbacks callbacks_;
  OverlayLoop::Token watch_;
  OverlayLoop::Token flush_timer_;
  wl_display *display_ = nullptr;
  wl_registry *registry_ = nullptr;
  wl_compositor *compositor_ = nullptr;
  wl_shm *shm_ = nullptr;
  wl_seat *seat_ = nullptr;
  wl_pointer *pointer_ = nullptr;
  uint32_t seat_id_ = 0;
  zwlr_layer_shell_v1 *shell_ = nullptr;
  std::vector<Output> outputs_;
  wl_output *active_output_ = nullptr;
  wl_surface *surface_ = nullptr;
  zwlr_layer_surface_v1 *layer_ = nullptr;
  bool closed_ = false;
  bool configured_ = false;
  bool dirty_ = false;

  Content content_;
  FloatingToolbarLayout toolbar_;
  FloatingToolbarFrame frame_;
  FloatingToolbarMenuLayout menu_;
  std::vector<FloatingToolbarMenuItem> menu_items_;
  std::optional<std::size_t> menu_hovered_;
  bool mouse_inside_ = false;
  double x_ = 0;
  double y_ = 0;
  std::optional<std::pair<double, double>> drag_start_;
  WaveOverlayPosition position_{};
  int width_ = 0;
  int height_ = 0;
  int toolbar_x_ = 0;
  int toolbar_y_ = 0;
  int menu_x_ = 0;
  int menu_y_ = 0;
  std::array<Buffer, 2> buffers_;
};

void WaylandToolbar::Buffer::clear() {
  if (object) wl_buffer_destroy(object);
  if (image) cairo_surface_destroy(image);
  if (pixels) munmap(pixels, bytes);
  *this = {};
}

bool WaylandToolbar::Buffer::allocate(int w, int h) {
  if (object && w == width && h == height) return true;
  auto *toolbar = owner;
  clear();
  owner = toolbar;
  // 使用匿名文件传给合成器。同一缓冲只在收到 release 后才会重画。
  const int fd = memfd_create("msime-wayland-toolbar", MFD_CLOEXEC);
  if (fd < 0) return false;
  const int stride = cairo_format_stride_for_width(CAIRO_FORMAT_ARGB32, w);
  bytes = static_cast<std::size_t>(stride) * h;
  if (ftruncate(fd, bytes) == 0) {
    void *mapping = mmap(nullptr, bytes, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (mapping != MAP_FAILED) {
      pixels = mapping;
      auto *pool = wl_shm_create_pool(owner->shm_, fd, bytes);
      object = wl_shm_pool_create_buffer(pool, 0, w, h, stride, WL_SHM_FORMAT_ARGB8888);
      wl_shm_pool_destroy(pool);
      image = cairo_image_surface_create_for_data(static_cast<unsigned char *>(pixels), CAIRO_FORMAT_ARGB32,
                                                 w, h, stride);
      static const wl_buffer_listener listener{[](void *data, wl_buffer *) {
        auto &buffer = *static_cast<Buffer *>(data);
        buffer.busy = false;
        buffer.owner->redraw();
      }};
      wl_buffer_add_listener(object, &listener, this);
      width = w;
      height = h;
    }
  }
  close(fd);
  return object != nullptr;
}

bool WaylandToolbar::connect() {
  if (connected()) return true;
  disconnect();
  display_ = wl_display_connect(nullptr);
  if (!display_) return false;
  registry_ = wl_display_get_registry(display_);
  static const wl_registry_listener listener{
      [](void *data, wl_registry *registry, uint32_t id, const char *name, uint32_t version) {
        static_cast<WaylandToolbar *>(data)->add_global(registry, id, name, version);
      },
      [](void *data, wl_registry *, uint32_t id) { static_cast<WaylandToolbar *>(data)->remove_global(id); }};
  wl_registry_add_listener(registry_, &listener, this);
  if (wl_display_roundtrip(display_) < 0 || !compositor_ || !shm_ || !shell_) {
    disconnect();
    return false;
  }
  watch_ = loop_.watch(wl_display_get_fd(display_), [this] {
    if (wl_display_dispatch(display_) < 0) {
      disconnect();
      return;
    }
    flush();
  });
  return true;
}

void WaylandToolbar::add_global(wl_registry *registry, uint32_t id, const char *name, uint32_t version) {
  const std::string_view interface(name);
  if (interface == wl_compositor_interface.name && !compositor_ && version >= 3) {
    compositor_ = static_cast<wl_compositor *>(wl_registry_bind(registry, id, &wl_compositor_interface, 3));
  } else if (interface == wl_shm_interface.name && !shm_) {
    shm_ = static_cast<wl_shm *>(wl_registry_bind(registry, id, &wl_shm_interface, 1));
  } else if (interface == zwlr_layer_shell_v1_interface.name && !shell_) {
    shell_ = static_cast<zwlr_layer_shell_v1 *>(wl_registry_bind(registry, id, &zwlr_layer_shell_v1_interface, 1));
  } else if (interface == wl_seat_interface.name && !seat_) {
    seat_id_ = id;
    seat_ = static_cast<wl_seat *>(wl_registry_bind(registry, id, &wl_seat_interface, std::min(version, 3u)));
    static const wl_seat_listener listener{
        [](void *data, wl_seat *, uint32_t flags) {
          static_cast<WaylandToolbar *>(data)->update_pointer(flags & WL_SEAT_CAPABILITY_POINTER);
        },
        [](void *, wl_seat *, const char *) {}};
    wl_seat_add_listener(seat_, &listener, this);
  } else if (interface == wl_output_interface.name && version >= 2) {
    auto *object = static_cast<wl_output *>(wl_registry_bind(registry, id, &wl_output_interface, 2));
    outputs_.push_back({object, id});
    static const auto listener = [] {
      wl_output_listener events{};
      events.geometry = [](void *data, wl_output *object, int32_t, int32_t, int32_t, int32_t, int32_t,
                           const char *, const char *, int32_t transform) {
        static_cast<WaylandToolbar *>(data)->output(object).transform = transform;
      };
      events.mode = [](void *data, wl_output *object, uint32_t flags, int32_t w, int32_t h, int32_t) {
        if (!(flags & WL_OUTPUT_MODE_CURRENT)) return;
        auto &output = static_cast<WaylandToolbar *>(data)->output(object);
        output.width = w;
        output.height = h;
      };
      events.scale = [](void *data, wl_output *object, int32_t scale) {
        static_cast<WaylandToolbar *>(data)->output(object).scale = std::max(1, scale);
      };
      events.done = [](void *data, wl_output *object) {
        auto &self = *static_cast<WaylandToolbar *>(data);
        if (object == self.active_output_) self.update_layout();
      };
      return events;
    }();
    wl_output_add_listener(object, &listener, this);
  }
}

void WaylandToolbar::remove_global(uint32_t id) {
  if (id == seat_id_) {
    update_pointer(false);
    wl_seat_destroy(std::exchange(seat_, nullptr));
    seat_id_ = 0;
  }
  auto found = std::find_if(outputs_.begin(), outputs_.end(), [id](const Output &item) { return item.id == id; });
  if (found == outputs_.end()) return;
  if (found->object == active_output_) active_output_ = nullptr;
  wl_output_destroy(found->object);
  outputs_.erase(found);
}

Output &WaylandToolbar::output(wl_output *object) {
  return *std::find_if(outputs_.begin(), outputs_.end(), [object](const Output &item) { return item.object == object; });
}

void WaylandToolbar::update_pointer(bool available) {
  if (available == (pointer_ != nullptr)) return;
  if (!available) {
    auto *pointer = std::exchange(pointer_, nullptr);
    if (wl_pointer_get_version(pointer) >= WL_POINTER_RELEASE_SINCE_VERSION) wl_pointer_release(pointer);
    else wl_pointer_destroy(pointer);
    mouse_inside_ = false;
    drag_start_.reset();
    frame_.pressed.reset();
    frame_.hovered.reset();
    open_menu({});
    return;
  }
  pointer_ = wl_seat_get_pointer(seat_);
  static const auto listener = [] {
    wl_pointer_listener events{};
    events.enter = [](void *data, wl_pointer *, uint32_t, wl_surface *surface, wl_fixed_t x, wl_fixed_t y) {
      auto &self = *static_cast<WaylandToolbar *>(data);
      self.mouse_inside_ = surface == self.surface_;
      self.change_pointer(wl_fixed_to_double(x), wl_fixed_to_double(y));
    };
    events.leave = [](void *data, wl_pointer *, uint32_t, wl_surface *) {
      auto &self = *static_cast<WaylandToolbar *>(data);
      self.mouse_inside_ = false;
      self.change_pointer(0, 0);
    };
    events.motion = [](void *data, wl_pointer *, uint32_t, wl_fixed_t x, wl_fixed_t y) {
      static_cast<WaylandToolbar *>(data)->change_pointer(wl_fixed_to_double(x), wl_fixed_to_double(y));
    };
    events.button = [](void *data, wl_pointer *, uint32_t, uint32_t, uint32_t button, uint32_t state) {
      static_cast<WaylandToolbar *>(data)->click(button, state == WL_POINTER_BUTTON_STATE_PRESSED);
    };
    events.axis = [](void *, wl_pointer *, uint32_t, uint32_t, wl_fixed_t) {};
    return events;
  }();
  wl_pointer_add_listener(pointer_, &listener, this);
}

void WaylandToolbar::present(const Content &content, std::optional<WaveOverlayPosition> saved) {
  if (!connect()) return;
  if (closed_) hide();
  content_ = content;
  frame_ = {};
  frame_.show_logo = content.config.show_logo;
  frame_.icon_path = content.icon_path;
  for (auto button : content.config.buttons) {
    auto face = floating_toolbar_face(button, content.state, content.display_name);
    face.enabled = content.focus_actions || !floating_toolbar_button_needs_focus(button);
    frame_.faces.push_back(std::move(face));
  }
  toolbar_ = floating_toolbar_layout(content.config, true);
  if (!surface_) {
    position_ = saved.value_or(WaveOverlayPosition{10, 10});
    surface_ = wl_compositor_create_surface(compositor_);
    static const auto surface_events = [] {
      wl_surface_listener events{};
      events.enter = [](void *data, wl_surface *, wl_output *object) {
        auto &self = *static_cast<WaylandToolbar *>(data);
        self.active_output_ = object;
        self.update_layout();
      };
      events.leave = [](void *data, wl_surface *, wl_output *object) {
        auto &self = *static_cast<WaylandToolbar *>(data);
        if (self.active_output_ == object) self.active_output_ = nullptr;
      };
      return events;
    }();
    wl_surface_add_listener(surface_, &surface_events, this);
    // Top 层不占用工作区，不参与普通窗口布局，也不获取键盘焦点。
    layer_ = zwlr_layer_shell_v1_get_layer_surface(shell_, surface_, nullptr, ZWLR_LAYER_SHELL_V1_LAYER_TOP,
                                                  "msime-floating-toolbar");
    static const zwlr_layer_surface_v1_listener events{
        [](void *data, zwlr_layer_surface_v1 *layer, uint32_t serial, uint32_t, uint32_t) {
          auto &self = *static_cast<WaylandToolbar *>(data);
          zwlr_layer_surface_v1_ack_configure(layer, serial);
          self.configured_ = true;
          self.dirty_ = true;
          self.redraw();
        },
        [](void *data, zwlr_layer_surface_v1 *) { static_cast<WaylandToolbar *>(data)->closed_ = true; }};
    zwlr_layer_surface_v1_add_listener(layer_, &events, this);
    zwlr_layer_surface_v1_set_anchor(layer_, ZWLR_LAYER_SURFACE_V1_ANCHOR_RIGHT | ZWLR_LAYER_SURFACE_V1_ANCHOR_BOTTOM);
    zwlr_layer_surface_v1_set_keyboard_interactivity(layer_, ZWLR_LAYER_SURFACE_V1_KEYBOARD_INTERACTIVITY_NONE);
  }
  open_menu({});
}

void WaylandToolbar::update_layout() {
  if (!visible()) return;
  const bool has_menu = !menu_items_.empty();
  const int toolbar_width = std::ceil(toolbar_.width);
  const int toolbar_height = std::ceil(toolbar_.height);
  const int menu_width = has_menu ? std::ceil(menu_.width) : 0;
  const int menu_height = has_menu ? std::ceil(menu_.height) : 0;
  const int right_limit = active_output_ ? output(active_output_).logical_width() : 32767;
  const int bottom_limit = active_output_ ? output(active_output_).logical_height() : 32767;
  const int right = std::clamp(position_.x, 0, std::max(0, right_limit - toolbar_width));
  const int bottom = std::clamp(position_.y, 0, std::max(0, bottom_limit - toolbar_height));
  const bool menu_below = has_menu && toolbar_height + menu_height + bottom > bottom_limit && menu_height <= bottom;
  const int width = std::max(toolbar_width, menu_width);
  const int height = toolbar_height + menu_height;
  toolbar_x_ = width - toolbar_width;
  toolbar_y_ = menu_below ? 0 : menu_height;
  menu_x_ = width - menu_width;
  menu_y_ = menu_below ? toolbar_height : 0;
  zwlr_layer_surface_v1_set_margin(layer_, 0, right, bottom - (menu_below ? menu_height : 0), 0);
  // 映射前以一次无缓冲的提交请求 configure，收到确认后再挂像素。
  if (width != width_ || height != height_) {
    width_ = width;
    height_ = height;
    zwlr_layer_surface_v1_set_size(layer_, width_, height_);
  }
  dirty_ = true;
  if (configured_) {
    redraw();
  } else {
    wl_surface_commit(surface_);
    flush();
  }
}

void WaylandToolbar::redraw() {
  if (!dirty_ || !configured_ || !visible()) return;
  const auto available = std::find_if(buffers_.begin(), buffers_.end(), [](const Buffer &buffer) { return !buffer.busy; });
  const int scale = active_output_ ? std::clamp(output(active_output_).scale, 1, 4) : 1;
  if (available == buffers_.end()) return;
  available->owner = this;
  if (!available->allocate(width_ * scale, height_ * scale)) return;
  auto *cairo = cairo_create(available->image);
  cairo_set_operator(cairo, CAIRO_OPERATOR_CLEAR);
  cairo_paint(cairo);
  cairo_set_operator(cairo, CAIRO_OPERATOR_OVER);
  cairo_scale(cairo, scale, scale);
  cairo_save(cairo);
  cairo_translate(cairo, toolbar_x_, toolbar_y_);
  paint_floating_toolbar(cairo, toolbar_, frame_, content_.colors);
  cairo_restore(cairo);
  if (!menu_items_.empty()) {
    cairo_translate(cairo, menu_x_, menu_y_);
    paint_floating_toolbar_menu(cairo, menu_, menu_items_, menu_hovered_, content_.colors);
  }
  cairo_destroy(cairo);
  cairo_surface_flush(available->image);
  auto *region = wl_compositor_create_region(compositor_);
  const auto add = [region](const FloatingToolbarRect &plate, int x, int y) {
    wl_region_add(region, x + std::floor(plate.x), y + std::floor(plate.y),
                  std::ceil(plate.width), std::ceil(plate.height));
  };
  add(toolbar_.plate, toolbar_x_, toolbar_y_);
  if (!menu_items_.empty()) add(menu_.plate, menu_x_, menu_y_);
  wl_surface_set_input_region(surface_, region);
  wl_region_destroy(region);
  wl_surface_set_buffer_scale(surface_, scale);
  wl_surface_attach(surface_, available->object, 0, 0);
  wl_surface_damage(surface_, 0, 0, width_, height_);
  available->busy = true;
  dirty_ = false;
  wl_surface_commit(surface_);
  flush();
}

void WaylandToolbar::change_pointer(double x, double y) {
  x_ = x;
  y_ = y;
  auto hit = floating_toolbar_hit(toolbar_, x - toolbar_x_, y - toolbar_y_);
  auto hovered = mouse_inside_ ? hit.button : std::nullopt;
  auto menu_hover = mouse_inside_ ? floating_toolbar_menu_hit(menu_, menu_items_, x - menu_x_, y - menu_y_) : std::nullopt;
  if (frame_.hovered == hovered && menu_hovered_ == menu_hover) return;
  frame_.hovered = hovered;
  menu_hovered_ = menu_hover;
  dirty_ = true;
  redraw();
}

void WaylandToolbar::open_menu(std::vector<FloatingToolbarMenuItem> items) {
  menu_items_ = std::move(items);
  menu_hovered_.reset();
  if (!menu_items_.empty()) {
    for (auto &item : menu_items_)
      item.disabled = !content_.focus_actions && floating_toolbar_action_needs_focus(item.action);
    menu_ = floating_toolbar_menu_layout(menu_items_, content_.config.scale, true);
  }
  update_layout();
}

void WaylandToolbar::click(uint32_t button, bool down) {
  if (!visible() || (button != BTN_LEFT && button != BTN_RIGHT)) return;
  if (callbacks_.interacted) callbacks_.interacted();
  if (!down && button == BTN_LEFT && drag_start_) {
    // 松手前保持表面不动，隐式指针抓取使位移始终基于同一套表面坐标。
    const auto [start_x, start_y] = *std::exchange(drag_start_, std::nullopt);
    const int right_limit = active_output_ ? output(active_output_).logical_width() : 32767;
    const int bottom_limit = active_output_ ? output(active_output_).logical_height() : 32767;
    position_.x = std::clamp(position_.x - static_cast<int>(std::lround(x_ - start_x)), 0,
                             std::max(0, right_limit - width_));
    position_.y = std::clamp(position_.y - static_cast<int>(std::lround(y_ - start_y)), 0,
                             std::max(0, bottom_limit - height_));
    if (callbacks_.moved) callbacks_.moved(position_);
    update_layout();
    return;
  }
  if (!menu_items_.empty()) {
    if (!down && button == BTN_LEFT) {
      const auto index = floating_toolbar_menu_hit(menu_, menu_items_, x_ - menu_x_, y_ - menu_y_);
      const auto item = index && mouse_inside_ ? std::optional(menu_items_[*index]) : std::nullopt;
      open_menu({});
      if (item && callbacks_.action) callbacks_.action(item->action, item->argument);
    }
    return;
  }
  auto hit = floating_toolbar_hit(toolbar_, x_ - toolbar_x_, y_ - toolbar_y_);
  if (!mouse_inside_) hit = {};
  if (button == BTN_RIGHT) {
    if (!down && hit.button && content_.config.buttons[*hit.button] == FloatingToolbarButton::Settings)
      open_menu(floating_toolbar_utility_menu(content_.display_name));
    return;
  }
  if (down) {
    if (hit.handle) drag_start_ = std::pair{x_, y_};
    if (hit.button && frame_.faces[*hit.button].enabled) frame_.pressed = hit.button;
  } else {
    const auto pressed = std::exchange(frame_.pressed, std::nullopt);
    if (pressed && pressed == hit.button) {
      const auto control = content_.config.buttons[*pressed];
      if (auto action = floating_toolbar_button_action(control)) {
        if (callbacks_.action) callbacks_.action(*action, "");
      } else {
        open_menu(floating_toolbar_scheme_menu(content_.state));
      }
    }
  }
  dirty_ = true;
  redraw();
}

void WaylandToolbar::flush() {
  if (wl_display_flush(display_) < 0 && errno == EAGAIN && !flush_timer_) {
    flush_timer_ = loop_.timer(10, [this] {
      flush_timer_.reset();
      if (display_) flush();
    });
  }
}

void WaylandToolbar::hide() {
  if (layer_) zwlr_layer_surface_v1_destroy(std::exchange(layer_, nullptr));
  if (surface_) wl_surface_destroy(std::exchange(surface_, nullptr));
  closed_ = configured_ = dirty_ = mouse_inside_ = false;
  active_output_ = nullptr;
  drag_start_.reset();
  frame_.pressed.reset();
  frame_.hovered.reset();
  menu_items_.clear();
  menu_hovered_.reset();
  width_ = height_ = 0;
  if (connected()) flush();
}

void WaylandToolbar::disconnect() {
  watch_.reset();
  hide();
  flush_timer_.reset();
  for (auto &buffer : buffers_) buffer.clear();
  if (pointer_) wl_pointer_destroy(std::exchange(pointer_, nullptr));
  if (seat_) wl_seat_destroy(std::exchange(seat_, nullptr));
  seat_id_ = 0;
  for (const auto &output : outputs_) wl_output_destroy(output.object);
  outputs_.clear();
  if (shell_) wl_proxy_destroy(reinterpret_cast<wl_proxy *>(std::exchange(shell_, nullptr)));
  if (shm_) wl_shm_destroy(std::exchange(shm_, nullptr));
  if (compositor_) wl_compositor_destroy(std::exchange(compositor_, nullptr));
  if (registry_) wl_registry_destroy(std::exchange(registry_, nullptr));
  if (display_) wl_display_disconnect(std::exchange(display_, nullptr));
}

}  // namespace

std::unique_ptr<FloatingToolbarSurface> create_wayland_floating_toolbar(
    OverlayLoop loop, FloatingToolbarSurface::Callbacks callbacks) {
  auto surface = std::make_unique<WaylandToolbar>(std::move(loop), std::move(callbacks));
  return surface->connect() ? std::move(surface) : nullptr;
}

}  // namespace msime::linux_host

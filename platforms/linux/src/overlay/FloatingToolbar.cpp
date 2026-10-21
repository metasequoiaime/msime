#include "FloatingToolbar.h"

#include <fstream>
#include <iterator>
#include <system_error>
#include <utility>

#include "../core/AtomicFileWrite.h"

namespace msime::linux_host {

std::unique_ptr<FloatingToolbar> FloatingToolbar::create(OverlayLoop loop, Handler handler, Options options,
                                                         const SurfaceFactory &surfaces) {
  if (!surfaces) return nullptr;
  std::unique_ptr<FloatingToolbar> toolbar(new FloatingToolbar(std::move(loop), std::move(handler), std::move(options)));
  auto *self = toolbar.get();
  FloatingToolbarSurface::Callbacks callbacks;
  callbacks.action = [self](FloatingToolbarAction action, const std::string &argument) {
    // 属主已经放开（输入法换走了）时按钮后面没有人，什么也不做。
    if (self->owner_ && self->handler_) self->handler_(self->owner_, action, argument);
  };
  callbacks.interacted = [self] { self->interacted(); };
  callbacks.moved = [self](WaveOverlayPosition position) { self->moved(position); };
  callbacks.focus_changed = [self] {
    if (!self->active_) return;
    self->fullscreen_ = self->surface_->foreground_fullscreen();
    self->refresh();
  };
  toolbar->surface_ = surfaces(std::move(callbacks));
  if (!toolbar->surface_) return nullptr;
  if (toolbar->options_.position_file) {
    std::ifstream stream(*toolbar->options_.position_file);
    if (stream) {
      const std::string text((std::istreambuf_iterator<char>(stream)), std::istreambuf_iterator<char>());
      toolbar->saved_ = parse_floating_toolbar_position(text);
    }
  }
  return toolbar;
}

FloatingToolbar::FloatingToolbar(OverlayLoop loop, Handler handler, Options options)
    : loop_(std::move(loop)), handler_(std::move(handler)), options_(std::move(options)) {}

FloatingToolbar::~FloatingToolbar() {
  idle_timer_.reset();
  surface_.reset();
}

bool FloatingToolbar::visible() const { return surface_ && surface_->visible(); }

void FloatingToolbar::configure(const FloatingToolbarConfig &config, const FloatingToolbarColors &colors) {
  const bool newly_enabled = config.enabled && (!configured_ || !config_.enabled);
  config_ = config;
  colors_ = colors;
  configured_ = true;
  if (!config_.enabled) {
    idle_timer_.reset();
    idle_hidden_ = false;
  }
  // 打开开关算一次输入，与 macOS 的 setVisible:forDelegate: 相同。
  if (newly_enabled && active_ && owner_) {
    idle_hidden_ = false;
    restart_idle_timer();
  }
  refresh();
}

void FloatingToolbar::update(Owner owner, const FloatingToolbarState &state) {
  if (!owner || owner != owner_) return;
  state_ = state;
  refresh();
}

void FloatingToolbar::activate(Owner owner, const FloatingToolbarState &state) {
  if (!owner) return;
  const bool newly_active = !active_;
  const bool owner_changed = owner != owner_;
  owner_ = owner;
  state_ = state;
  active_ = true;
  focused_ = true;
  fullscreen_ = surface_->foreground_fullscreen();
  // 只有新的激活或换了属主才唤醒空闲隐藏的工具栏；同一个属主在几个输入框之间切换不算输入，与 macOS 的 activateForDelegate:visible: 相同。
  if ((newly_active || owner_changed) && config_.enabled) {
    idle_hidden_ = false;
    restart_idle_timer();
  }
  refresh();
}

void FloatingToolbar::note_input(Owner owner) {
  if (!owner || owner != owner_ || !active_ || !config_.enabled) return;
  idle_hidden_ = false;
  restart_idle_timer();
  refresh();
}

void FloatingToolbar::focus_out(Owner owner) {
  if (!owner || owner != owner_) return;
  focused_ = false;
  if (options_.hide_on_focus_out || options_.actions_need_focus) refresh();
}

void FloatingToolbar::deactivate(Owner owner) {
  if (!owner || owner != owner_) return;
  owner_ = nullptr;
  active_ = false;
  focused_ = false;
  idle_hidden_ = false;
  idle_timer_.reset();
  refresh();
}

void FloatingToolbar::restart_idle_timer() {
  idle_timer_ = loop_.timer(FloatingToolbarTiming::idle_ms, [this] {
    idle_hidden_ = true;
    refresh();
  });
}

void FloatingToolbar::interacted() {
  // 点击或拖动工具栏本身也算输入，按钮在用的时候不会到时隐藏，与 macOS 的 sendEvent: 相同。
  note_input(owner_);
}

void FloatingToolbar::moved(WaveOverlayPosition position) {
  saved_ = position;
  if (!options_.position_file) return;
  const auto &file = *options_.position_file;
  std::error_code error;
  std::filesystem::create_directories(file.parent_path(), error);
  if (error) return;
  // 写不进去只是下次回到默认位置，不影响输入。
  publish_file_atomically(file, serialize_floating_toolbar_position(position),
                          (file.parent_path() / ".floating-toolbar-position.XXXXXX").string());
}

void FloatingToolbar::refresh() {
  const bool focused = focused_ || !options_.hide_on_focus_out;
  const bool show = configured_ && owner_ &&
                    floating_toolbar_should_show(config_.enabled && !idle_hidden_ && focused, active_, fullscreen_);
  if (!show) {
    if (surface_->visible()) surface_->hide();
    presented_.reset();
    return;
  }
  FloatingToolbarSurface::Content content{config_, colors_, state_, options_.icon_path, options_.display_name,
                                          focused_ || !options_.actions_need_focus};
  if (surface_->visible() && presented_ && *presented_ == content) return;
  surface_->present(content, saved_);
  presented_ = std::move(content);
}

}  // namespace msime::linux_host

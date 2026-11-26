#include "CandidateMenuController.h"

#include "CandidateActionName.h"
#include "GnomePointer.h"

#ifdef MSIME_LINUX_FLOATING_PANELS
#include "CandidateMenuSurface.h"
#endif

#include <glib-unix.h>
#include <glib.h>

#include <atomic>
#include <utility>

namespace msime::linux_host {

namespace {

CandidateMenuController::ShellCalls production_shell_calls() {
  return {[](const std::string &token, const std::string &identity,
             const std::vector<ShellMenuEntry> &entries) {
            return shell_show_candidate_menu(token.c_str(), identity.c_str(), entries);
          },
          [](const std::string &token) { shell_hide_candidate_menu(token.c_str()); }};
}

// 扩展同一时间只有一份菜单；token 在进程内递增，几个输入上下文的收起不会互相误关。
std::atomic<std::uint64_t> next_shell_token{1};

}  // namespace

#ifdef MSIME_LINUX_FLOATING_PANELS
// 自绘浮层和它的显示连接监听（显示时才挂，收起时移除），照 sync_wave_overlay_sources 的做法。
struct CandidateMenuController::Overlay {
  CandidateMenuSurface surface;
  guint watch = 0;
  int watch_fd = -1;
  // show 时按条目下标存下的动作名；分隔线与禁用项不会成为命中。
  std::vector<std::string> actions;
};
#else
struct CandidateMenuController::Overlay {};
#endif

CandidateMenuController::CandidateMenuController() : CandidateMenuController(production_shell_calls()) {}

CandidateMenuController::CandidateMenuController(ShellCalls shell) : shell_(std::move(shell)) {}

CandidateMenuController::~CandidateMenuController() { release(); }

bool CandidateMenuController::show_in_shell(const std::vector<CandidateMenuItem> &items, Target target) {
  release();
  const auto entries = shell_menu_entries(items, target.session, target.generation, target.index);
  const auto token = std::to_string(next_shell_token.fetch_add(1, std::memory_order_relaxed));
  // session/generation/index 只作展示层一致性信息，信任根仍是 property_activate 的校验。
  const auto identity =
      candidate_action_name("", target.session, target.generation, target.index).substr(1);
  if (!shell_.show || !shell_.show(token, identity, entries))
    return false;
  shell_token_ = token;
  return true;
}

#ifdef MSIME_LINUX_FLOATING_PANELS
bool CandidateMenuController::show_overlay(const std::vector<CandidateMenuItem> &items, Target target,
                                           const FloatingToolbarColors &colors,
                                           CandidateMenuAnchor anchor, Activate activate) {
  release();
  if (!overlay_)
    overlay_ = std::make_unique<Overlay>();
  overlay_->actions.clear();
  for (const auto &entry : shell_menu_entries(items, target.session, target.generation, target.index))
    overlay_->actions.push_back(entry.action_name);
  const bool shown = overlay_->surface.show(
      items, colors, anchor,
      [this, activate = std::move(activate)](std::size_t item_index) {
        if (!overlay_ || item_index >= overlay_->actions.size() ||
            overlay_->actions[item_index].empty())
          return;
        // 先拷出来：activate 会经 property_activate 重绘，重绘又会 release，这期间 actions 可能被改写。
        const auto action = overlay_->actions[item_index];
        if (activate)
          activate(action);
      },
      // 收起后的通知（点外、选择或 release 里的 hide）：卸掉显示连接监听。
      [this] { sync_overlay_watch(); });
  if (!shown)
    return false;
  sync_overlay_watch();
  return true;
}

void CandidateMenuController::sync_overlay_watch() {
  if (!overlay_)
    return;
  const int fd = overlay_->surface.visible() ? overlay_->surface.event_fd() : -1;
  if (fd == overlay_->watch_fd)
    return;
  if (overlay_->watch)
    g_source_remove(overlay_->watch);
  overlay_->watch = 0;
  overlay_->watch_fd = fd;
  if (fd < 0)
    return;
  overlay_->watch = g_unix_fd_add_full(
      G_PRIORITY_DEFAULT, fd, G_IO_IN,
      +[](gint, GIOCondition condition, gpointer data) -> gboolean {
        auto *self = static_cast<CandidateMenuController *>(data);
        auto &overlay = *self->overlay_;
        if (!overlay.surface.visible() || (condition & (G_IO_ERR | G_IO_HUP | G_IO_NVAL))) {
          // 返回 G_SOURCE_REMOVE 由主循环移除这个监听，先清掉记录，免得 release 再移除一次。
          overlay.watch = 0;
          overlay.watch_fd = -1;
          self->release();
          return G_SOURCE_REMOVE;
        }
        overlay.surface.dispatch_events();
        return G_SOURCE_CONTINUE;
      },
      this, nullptr);
}

#else
bool CandidateMenuController::show_overlay(const std::vector<CandidateMenuItem> &, Target,
                                           const FloatingToolbarColors &, CandidateMenuAnchor,
                                           Activate) {
  return false;
}

void CandidateMenuController::sync_overlay_watch() {}
#endif

void CandidateMenuController::release() {
#ifdef MSIME_LINUX_FLOATING_PANELS
  if (overlay_) {
    if (overlay_->surface.visible())
      overlay_->surface.hide();
    if (overlay_->watch)
      g_source_remove(overlay_->watch);
    overlay_->watch = 0;
    overlay_->watch_fd = -1;
  }
#endif
  if (!shell_token_.empty()) {
    const auto token = std::move(shell_token_);
    shell_token_.clear();
    if (shell_.hide)
      shell_.hide(token);
  }
}

bool CandidateMenuController::consume_escape() {
#ifdef MSIME_LINUX_FLOATING_PANELS
  const bool overlay_visible = overlay_ && overlay_->surface.visible();
#else
  const bool overlay_visible = false;
#endif
  release();
  return overlay_visible;
}

}  // namespace msime::linux_host

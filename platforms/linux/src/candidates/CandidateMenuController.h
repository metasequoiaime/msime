// 一个 iBus 输入上下文的候选右键菜单：GNOME Shell 会话推给随装扩展画（D-Bus，按 token 收起），其余会话用宿主自绘的 X11 浮层（CandidateMenuSurface，显示时在 GLib 主循环里监听它的显示连接）。两条路的条目都来自 candidate_menu_items，点中后都回传完整动作名，由引擎的 property_activate 按会话、代次和身份核对后执行。菜单弹不出来时 show_* 返回 false，调用方改在辅助区域提示快捷键。
#pragma once

#include "../overlay/FloatingPanelColors.h"
#include "CandidateMenuItems.h"
#include "CandidateMenuPlacement.h"
#include "ShellCandidateMenu.h"

#include <cstddef>
#include <cstdint>
#include <functional>
#include <memory>
#include <string>
#include <vector>

namespace msime::linux_host {

class CandidateMenuController {
 public:
  // 点中条目时回传的完整动作名（CandidateActionName.h 的编码）。
  using Activate = std::function<void(const std::string &action_name)>;
  // GNOME 扩展的两条 D-Bus 调用；默认用 GnomePointer.h 的生产版本，测试注入假实现。
  struct ShellCalls {
    std::function<bool(const std::string &token, const std::string &identity,
                       const std::vector<ShellMenuEntry> &entries)>
        show;
    std::function<void(const std::string &token)> hide;
  };
  // 被右键的那一行：session/generation/index 编进每个条目的动作名。
  struct Target {
    std::uint64_t session = 0;
    std::uint64_t generation = 0;
    std::size_t index = 0;
  };

  CandidateMenuController();
  explicit CandidateMenuController(ShellCalls shell);
  ~CandidateMenuController();
  CandidateMenuController(const CandidateMenuController &) = delete;
  CandidateMenuController &operator=(const CandidateMenuController &) = delete;

  // GNOME Shell 会话：推给扩展画。扩展没启用、总线不可用或超时返回 false。
  bool show_in_shell(const std::vector<CandidateMenuItem> &items, Target target);
  // 其余会话：宿主自绘 X11 浮层。没有悬浮面板的构建、连不上显示或建不了窗口时返回 false。
  bool show_overlay(const std::vector<CandidateMenuItem> &items, Target target,
                    const FloatingToolbarColors &colors, CandidateMenuAnchor anchor,
                    Activate activate);
  // 收起菜单：自绘浮层直接隐藏并停掉显示连接监听；推给扩展的菜单按 token 收起，扩展只关匹配的那一轮，迟到的收起不误关新菜单。会话的每条出口（focus_out、reset、render）都调它。
  void release();
  // Esc：自绘浮层显示着时只收菜单、返回 true，Esc 不再取消组字（浮层不抢焦点，Esc 仍先到引擎）。否则返回 false，Esc 照常取消组字：GNOME 扩展的菜单自己在 Shell 里停掉 Esc，点外面或候选框收起时扩展关菜单并不通知宿主，所以留着的 token 不等于菜单还开着，这里只顺手收掉它。
  bool consume_escape();

 private:
  struct Overlay;
  void sync_overlay_watch();

  ShellCalls shell_;
  std::string shell_token_;
  std::unique_ptr<Overlay> overlay_;
};

}  // namespace msime::linux_host

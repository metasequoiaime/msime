#pragma once

#include <functional>
#include <optional>
#include <string>
#include <vector>

#include "FloatingPanelColors.h"
#include "FloatingToolbarModel.h"
#include "WaveOverlayPlacement.h"

namespace msime::linux_host {

// 悬浮工具栏的显示后端。FloatingToolbar 管什么时候显示、空闲隐藏和位置记忆，后端只管把工具栏、菜单和提示画出来并报告指针做了什么。现在只有 X11 后端（FloatingToolbarX11Surface），GNOME 的 Wayland 会话经 Xwayland 也走它；拿不到后端时宿主只有菜单里的「工具栏」子菜单。
class FloatingToolbarSurface {
 public:
  struct Callbacks {
    // 点了按钮或菜单项。
    std::function<void(FloatingToolbarAction, const std::string &argument)> action;
    // 指针在工具栏或它的菜单上按下、拖动：与 macOS 的 sendEvent: 一样算一次输入，重新计空闲时间。
    std::function<void()> interacted;
    // 拖完松手，工具栏本体左上角的新位置（根窗口的设备像素）。
    std::function<void(WaveOverlayPosition)> moved;
    // 前台窗口或它的状态变了，是否全屏需要重新判断。
    std::function<void()> focus_changed;
  };

  // 一次要显示的全部内容。
  struct Content {
    FloatingToolbarConfig config;
    FloatingToolbarColors colors;
    FloatingToolbarState state;
    std::string icon_path;
    std::string display_name;
    // 要输入框的按钮和菜单项（floating_toolbar_button_needs_focus）此刻能不能用；为假时它们画成淡色、点了不做事。
    bool focus_actions = true;
    bool operator==(const Content &other) const {
      return config == other.config && colors == other.colors && state == other.state &&
             icon_path == other.icon_path && display_name == other.display_name &&
             focus_actions == other.focus_actions;
    }
  };

  virtual ~FloatingToolbarSurface() = default;
  // 显示或更新。已经在屏幕上时位置不动，只按新尺寸收进工作区；从隐藏变成显示时按 saved（用户拖过的位置）或默认的右下角重新摆。
  virtual void present(const Content &content, std::optional<WaveOverlayPosition> saved) = 0;
  virtual void hide() = 0;
  virtual bool visible() const = 0;
  // 断线时旧的全屏判断不再有效；默认后端没有可断开的连接。
  virtual bool connected() const { return true; }
  // 前台窗口是否全屏（Linux 与 Windows 一样在全屏应用前收起工具栏）。查不到时回答 false。
  virtual bool foreground_fullscreen() = 0;
};

}  // namespace msime::linux_host

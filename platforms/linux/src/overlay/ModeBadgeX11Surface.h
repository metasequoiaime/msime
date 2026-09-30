#pragma once
#include <X11/Xlib.h>

#include <memory>
#include <string>

#include "ModeBadgeSurface.h"

namespace msime::linux_host {

// X11 会话下的徽章：一个 override-redirect 窗口，用 cairo-xlib 画，输入形状为空所以点击
// 穿透。画法与 Wayland 后端共用 ModeBadgePainter.h。
class ModeBadgeX11Surface final : public ModeBadgeSurface {
 public:
  ~ModeBadgeX11Surface() override;
  ModeBadgeX11Surface(const ModeBadgeX11Surface &) = delete;
  ModeBadgeX11Surface &operator=(const ModeBadgeX11Surface &) = delete;

  static std::unique_ptr<ModeBadgeX11Surface> create();

  bool show(const std::string &text, const std::string &icon_path, const ModeBadgeStyle &style) override;
  void hide() override;

 private:
  ModeBadgeX11Surface() = default;
  bool ensure_window();
  void destroy_window();

  Display *display_ = nullptr;
  Window window_ = 0;
  Visual *visual_ = nullptr;
  Colormap colormap_ = 0;
  // RandR 1.5 monitor enumeration; without it the badge sits in the EWMH work area or the root window.
  bool randr_monitors_ = false;
  bool visible_ = false;
};

}  // namespace msime::linux_host

#pragma once
#include <X11/Xlib.h>

#include <memory>
#include <optional>
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

  bool show(const std::string &text, const std::string &icon_path, const ModeBadgeStyle &style,
            std::optional<ModeBadgeCaret> caret) override;
  bool set_opacity(double alpha) override;
  void hide() override;
  bool follows_caret() const override { return true; }

 private:
  ModeBadgeX11Surface() = default;
  bool ensure_window();
  void destroy_window();
  // 有没有合成管理器（EWMH 的 _NET_WM_CM_S<屏幕号> 选区有主人）。没有时 ARGB 窗口的透明像素会画成黑色，于是不画阴影、不做淡出。
  bool compositing() const;
  void paint(double alpha);

  Display *display_ = nullptr;
  Window window_ = 0;
  Visual *visual_ = nullptr;
  Colormap colormap_ = 0;
  // RandR 1.5 monitor enumeration; without it the badge sits in the EWMH work area or the root window.
  bool randr_monitors_ = false;
  // 拿到了 32 位 ARGB visual；拿不到时窗口不透明，阴影和淡出都无从谈起。
  bool argb_ = false;
  bool visible_ = false;
  // 上一次 show 画的内容和窗口尺寸，set_opacity 按它重画。
  std::string text_;
  std::string icon_path_;
  FloatingSurfaceColors colors_;
  ModeBadgeLayout layout_;
  int window_width_ = 0;
  int window_height_ = 0;
};

}  // namespace msime::linux_host

#pragma once
#include <memory>
#include <optional>
#include <string>

#include "ModeBadgePlacement.h"
#include "ModeBadgeStyle.h"

namespace msime::linux_host {

// 中英文切换徽章的后端接口。Wayland 用 layer-shell，X11 用 override-redirect 窗口，画法
// 两边共用 ModeBadgePainter.h。宿主只管要一个，拿不到就回退到各自面板能表达的文字提示。
class ModeBadgeSurface {
 public:
  virtual ~ModeBadgeSurface() = default;
  // style 带缩放和解析出的主题颜色；每次 show 都重新测量，换主题在下一次切换时生效。caret 为空时徽章放在屏幕右下角；给了就按 mode_badge_caret_position 放在光标旁（光标矩形不可用时用 macOS 的回退位置）。不能跟随光标的后端忽略它。
  virtual bool show(const std::string &text, const std::string &icon_path, const ModeBadgeStyle &style,
                    std::optional<ModeBadgeCaret> caret) = 0;
  // 按 alpha（0 到 1）重画上一次 show 的内容，用于淡出。后端做不到半透明（没有合成器的 X11）时返回 false，宿主就直接隐藏。
  virtual bool set_opacity(double alpha) = 0;
  virtual void hide() = 0;
  // 能否把徽章放到根窗口坐标里的光标旁。layer-shell 只能锚在输出边上，所以 Wayland 后端是 false。
  virtual bool follows_caret() const = 0;

  // 先试当前会话类型的那个后端。两个都拿不到时返回空指针，不做无谓的重试——连接不上
  // 合成器或 X 服务器是环境决定的，不会在下一次切换时忽然变好。
  static std::unique_ptr<ModeBadgeSurface> create();
};

}  // namespace msime::linux_host

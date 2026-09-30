#pragma once
#include <memory>
#include <string>

#include "ModeBadgeStyle.h"

namespace msime::linux_host {

// 中英文切换徽章的后端接口。Wayland 用 layer-shell，X11 用 override-redirect 窗口，画法
// 两边共用 ModeBadgePainter.h。宿主只管要一个，拿不到就回退到各自面板能表达的文字提示。
class ModeBadgeSurface {
 public:
  virtual ~ModeBadgeSurface() = default;
  // style carries the size (floating_toolbar.font_size, scale_percent) and the resolved theme palette; each show re-measures, so a preference change applies at the next switch.
  virtual bool show(const std::string &text, const std::string &icon_path, const ModeBadgeStyle &style) = 0;
  virtual void hide() = 0;

  // 先试当前会话类型的那个后端。两个都拿不到时返回空指针，不做无谓的重试——连接不上
  // 合成器或 X 服务器是环境决定的，不会在下一次切换时忽然变好。
  static std::unique_ptr<ModeBadgeSurface> create();
};

}  // namespace msime::linux_host

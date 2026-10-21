#pragma once

#include "NativeCanvas.h"
#include <memory>
#include <optional>

namespace fcitx { class EventLoop; }
namespace msime::linux_host {

// 画布里候选卡片在根窗口里的矩形，设备像素；不含窗口四周的投影边距、装饰图和外留白。
struct NativeCanvasFrame {
  int x = 0, y = 0, width = 0, height = 0;
};

class NativeCanvasX11 {
public:
  explicit NativeCanvasX11(fcitx::EventLoop &loop);
  ~NativeCanvasX11();
  NativeCanvasX11(const NativeCanvasX11 &) = delete;
  NativeCanvasX11 &operator=(const NativeCanvasX11 &) = delete;
  bool show(const std::string &display, std::shared_ptr<NativeCanvasAssets> assets,
            NativeCanvasContent content, nlohmann::json palette,
            int cursor_x, int cursor_y, int cursor_height);
  // 工作区变化时重排；返回 false 时宿主需要恢复桌面候选。
  bool move(int cursor_x, int cursor_y, int cursor_height);
  // 收起候选窗。`animate` 为 true 且皮肤写了退场预设时，窗口在不接收鼠标的状态下播完退场再撤下；失焦、隐私输入这类收起传 false，立即撤下。
  void hide(bool animate = false);
  // 桌面的「减弱动态效果」：为真时不播进场、退场、选中和装饰图待机动效，APNG、GIF、WebP 停在首帧。
  void setReduceMotion(bool reduce);
  // 画布正在屏幕上时卡片的矩形（退场动画中算已收起）；双拼键位图据此让开候选窗，不用按字号估计。
  std::optional<NativeCanvasFrame> frame() const;
private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

} // msime::linux_host 命名空间

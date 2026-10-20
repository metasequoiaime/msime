#pragma once

#include "NativeCanvas.h"
#include <memory>

namespace fcitx { class EventLoop; }
namespace msime::linux_host {

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
  void hide();
private:
  struct Impl;
  std::unique_ptr<Impl> impl_;
};

} // msime::linux_host 命名空间

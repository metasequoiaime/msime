#pragma once
#include "WaveOverlayModel.h"
#include <functional>
namespace msime::linux_host {
class WaveOverlaySurface {
 public:
  using ActionHandler = std::function<void(WaveOverlayModel::Action)>;
  virtual ~WaveOverlaySurface() = default;
  virtual bool show(const WaveOverlayModel&) = 0;
  virtual void update(const WaveOverlayModel&) = 0;
  virtual void hide() = 0;
  // 显示中的浮层读取指针和 expose 事件所用的显示连接，没有则为 -1。浮层显示期间宿主在自己的事件循环上监听它，可读时调用 `dispatch_events()`，操作按钮上的点击因此立即处理，不等下一次重画。
  virtual int event_fd() const { return -1; }
  virtual void dispatch_events() {}
  // 显示中的浮层是否自己画波形，因而在模型播放动画时每 `kWaveOverlayFrameMilliseconds` 需要一帧；文本回退不需要。
  virtual bool animates() const { return false; }
};
}

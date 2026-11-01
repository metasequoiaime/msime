#pragma once
#include "CandidateWindow.h"
#include <memory>

namespace msime::windows {
// 主线程解析显示偏好与皮肤，只发布值快照。每个桌面的绘制对象均由自己的 UI 线程持有。
struct DesktopCandidateSettings {
  CandidateFontSettings fonts;
  CandidateLayoutSettings layout;
  CandidateWindowStyle style;
  CandidatePalette palette, keymap_palette;
  CandidateSkinDecoration decoration;
  CandidateSkinBackground background;
  CandidateSkinLayout skin_layout;
  CandidateSkinAnimation animation;
  CandidateSkinCanvas canvas;
  std::string skin_font;
  std::optional<float> skin_radius;
  double min_width = 0;
  bool follow_cursor = true;
  // 游戏兜底锚点放客户区上部（game_compatibility.overlay_anchor_top）。
  bool game_anchor_top = false;
  bool mouse_wheel = false;
  bool keymap_enabled = false;
  // 配色／皮肤值与图像缓存各有代次，普通轮询不会重建字体或重载图片。
  uint64_t appearance_revision = 1;
  uint64_t image_revision = 1;
};
struct DesktopCandidateCallbacks {
  CandidateWindow::Reader read;
  CandidateWindow::Click click;
  CandidateWindow::Page page;
  CandidateWindow::Rendered rendered;
  CandidateWindow::Expansion expansion;
  CandidateWindow::LeaseCheck lease_current;
};
// 单个 Server 的候选 UI 路由器；协调线程不建窗，各桌面线程先绑定桌面再创建任何 UI。
// stop() 必须先于 Server 与点击工作线程的停止；析构也执行相同收尾。
class DesktopCandidates final {
public:
  explicit DesktopCandidates(DesktopCandidateCallbacks callbacks);
  ~DesktopCandidates();
  DesktopCandidates(const DesktopCandidates &) = delete;
  DesktopCandidates &operator=(const DesktopCandidates &) = delete;
  void start(DesktopCandidateSettings settings);
  void publish(DesktopCandidateSettings settings);
  void stop();
  // 任何线程：请求协调线程立即读一次快照并叫醒各桌面线程绘制，不等 16 毫秒的轮询。只发信号，不阻塞、不回调；stop() 之后调用无效果。渲染屏障等回执时经 SessionController::set_candidate_render_request 调用（#9519）。
  void wake();
  // 只含固定标签和数值，主线程取出后统一写诊断日志；队列有界。
  std::vector<std::string> take_diagnostics();
  bool failed() const;

private:
  struct State;
  std::unique_ptr<State> state_;
};
} // namespace msime::windows

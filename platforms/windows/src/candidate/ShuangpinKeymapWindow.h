#pragma once
#include "CandidatePalette.h"
#include "ComponentFailure.h"
#include "ShuangpinKeymapLayout.h"
#include <optional>
#include <string>
// 先包含 windows.h：它的 DrawText 宏要在 Direct2D 的声明之前生效。
#include <windows.h>
#include <msimeui/DeviceResources.h>

namespace msime::windows {
class CandidateWindow;
// 一帧键位提示和它要跟随的光标锚点（TSF 报来的光标所在行左下角，物理像素）。
struct ShuangpinKeymapFrame {
  ShuangpinKeymapHint hint;
  long anchor_x = 0;
  long anchor_y = 0;
};

// 双拼键位提示浮窗，对应 macOS 的 MSIMEShuangpinKeymapPanel：双拼组字时贴着候选窗显示当前方案的三排键帽和零声母说明，高亮刚按下的键。不抢焦点、不接收鼠标，配色取候选窗的调色板，键位表经 host-api 取自 Engine。只在 Server 的界面线程上使用；画不出来时自己隐藏，Server 去掉这块浮窗继续运行。
class ShuangpinKeymapWindow final {
public:
  ShuangpinKeymapWindow();
  ~ShuangpinKeymapWindow();
  ShuangpinKeymapWindow(const ShuangpinKeymapWindow &) = delete;
  ShuangpinKeymapWindow &operator=(const ShuangpinKeymapWindow &) = delete;
  // 与候选窗同一份调色板，主题和皮肤换了跟着换。
  void set_palette(CandidatePalette palette);
  // 主循环每轮在候选窗 refresh 之后调用：`frame` 为空（开关关着、不在双拼组字）或候选窗不可见时隐藏，否则贴着候选卡片显示。贴的是卡片本身（CandidateWindow::card_on_screen），不是带透明阴影边距的窗口外框。
  void update(const std::optional<ShuangpinKeymapFrame> &frame, const CandidateWindow &candidates);
  void hide();
  bool failed() const { return failed_; }
  const std::optional<ComponentFailureSite> &failure_site() const { return failure_site_; }

private:
  static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM) noexcept;
  // 换了方案时重新取这个方案的键位和零声母说明。
  void load_profile(const std::string &profile);
  void paint();
  void fail(ComponentFailureSite site);
  msimeui::DeviceResources device_;
  CandidatePalette palette_;
  ShuangpinKeymapMetrics metrics_;
  HWND window_ = nullptr;
  std::string profile_;
  ShuangpinKeymapRows rows_;
  std::wstring title_;
  std::wstring zero_initials_;
  char highlighted_ = 0;
  // 上次摆放的窗口位置和尺寸，没变时不再挪动窗口。
  RECT placed_{};
  bool failed_ = false;
  std::optional<ComponentFailureSite> failure_site_;
};
} // namespace msime::windows

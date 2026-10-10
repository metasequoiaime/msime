#pragma once
#include "AccessibleElements.h"
#include <cstdint>
#include <memory>
#include <optional>
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>

namespace msime::windows {
// 读屏要求执行一个元素时投递给窗口的消息：wparam 是元素 id，lparam 是发出请求时那棵树的序号（用 AccessibleWindow::current 核对）。窗口在自己的 UI 线程上处理，序号对不上就丢掉，免得按旧树里的位置点到新树里的另一样东西。
inline constexpr UINT accessible_invoke_message = WM_APP + 0x5A;

struct AccessibleShared;

// 一个自绘窗口的 UI Automation 提供者：窗口本身是片段根（IRawElementProviderFragmentRoot），AccessibleTree 里的每个元素是它的一个子片段。窗口在 WM_GETOBJECT 里交给 answer()，每次画完把刚画好的树交给 publish()。
//
// 线程：publish、focus、menu_opened、menu_closed、disconnect 和析构只在窗口的 UI 线程上调用。提供者对象可能在 UI Automation 的线程上被调用，它们只在锁里读最近一次 publish 的树，执行请求一律 PostMessage 回窗口，不碰窗口的其他状态。
//
// 窗口销毁前必须 disconnect（析构也会做）：断开交出去的每个提供者并告诉 UI Automation 这个窗口的提供者都作废了；之后还被读屏拿着的提供者只回答「元素不可用」。
class AccessibleWindow final {
public:
  explicit AccessibleWindow(HWND window);
  ~AccessibleWindow();
  AccessibleWindow(const AccessibleWindow &) = delete;
  AccessibleWindow &operator=(const AccessibleWindow &) = delete;
  // WM_GETOBJECT：请求的是 UI Automation 的根时交出片段根并返回结果，其他请求（MSAA 的 OBJID_CLIENT 等）返回空，由 DefWindowProc 处理。
  std::optional<LRESULT> answer(WPARAM wparam, LPARAM lparam) noexcept;
  // 换上新的树。和上一棵完全相同时什么也不做；变了就让正在听的读屏重新取子元素。
  void publish(AccessibleTree tree) noexcept;
  // 执行请求带回的 lparam 是不是当前这棵树的序号。树每变一次序号加一，按旧树发出的请求就对不上。
  bool current(LPARAM token) const noexcept;
  // 报告键盘焦点移到了元素 `id` 上（托盘卡片的键盘导航）。元素须在当前树里并标着 focused。
  void focus(int id) noexcept;
  // 菜单打开、收起的事件，托盘卡片显示和隐藏时报告。
  void menu_opened() noexcept;
  void menu_closed() noexcept;
  void disconnect() noexcept;

private:
  HWND window_ = nullptr;
  std::shared_ptr<AccessibleShared> shared_;
};
} // namespace msime::windows

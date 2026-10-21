#pragma once

#include <functional>
#include <memory>

namespace msime::linux_host {

// 自绘浮窗要用的那一点事件循环：监视显示服务器连接可读，以及一次性定时器。IBus 宿主跑在 GLib 主循环里，Fcitx5 插件跑在 fcitx::EventLoop 里，浮窗本身不认识哪一个，由宿主各填一份。返回的令牌一析构就撤掉对应的监视或定时器，所以浮窗只要放下令牌就不会再被回调。回调一律在宿主的主线程上、在宿主自己的调用栈之外执行。
struct OverlayLoop {
  using Token = std::shared_ptr<void>;
  // fd 可读时调用 ready，直到令牌析构。
  std::function<Token(int fd, std::function<void()> ready)> watch;
  // milliseconds 毫秒后调用一次 fire；令牌在那之前析构就不调用。
  std::function<Token(int milliseconds, std::function<void()> fire)> timer;
};

}  // namespace msime::linux_host

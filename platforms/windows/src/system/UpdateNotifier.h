#pragma once
#include "UpdateNotification.h"
#include <functional>
#include <memory>
#include <string>

namespace msime::windows {

// Server 的后台更新检查和新版本提醒。一个工作线程在启动后 `update_first_check_delay` 第一次调用 `msime_client_update_background_check`，之后每 `update_check_interval` 调用一次；通道变了以后等 `update_channel_settle` 再调用，不在用户刚在「关于」页换完通道时又弹一条重复的通知。联网、节流和只提醒一次都在共享层，不在输入线程、也不在消息循环所在的线程上联网。共享层让提醒时，在创建它的线程（Server 的消息循环线程）上加一个临时的通知区域图标并弹出气泡（Windows 10/11 显示为通知）：系统正不接受通知（全屏、演示、锁屏、安静时间）时先留在内存里，等它接受了再弹，最多等 `update_notice_max_defer`。共享层返回时已经把这个构建记成提醒过了，所以气泡超时或被关掉以后图标仍然留着（Windows 10/11 删掉图标会连操作中心里的那条通知一起删掉），点气泡或图标打开设置窗口的「关于」页并删掉图标，右键图标只删掉图标，一天后自动删掉；不显示任何窗口，不抢焦点。
class UpdateNotifier {
public:
  struct Options {
    // 共享层的请求里除通道以外的部分：本用户的更新记录目录（UTF-8，common/UpdateRecordDirectory.h）、是不是 Windows 7 构建、本程序编译成的架构（只有 Windows 7 构建放进请求，见 update_background_request）、版本号、版本 id。
    std::string directory;
    bool windows7 = false;
    std::string architecture;
    std::string version;
    std::string edition;
    // 通知标题里的产品名（MSIME_EDITION_DISPLAY_NAME），几个版本同时安装时分得清是哪个。
    std::wstring product_name;
    // 偏好里现在的通道（stable、beta、nightly），在工作线程上调用，必须线程安全。
    std::function<std::string()> channel;
    // 点了气泡或图标：打开设置窗口的「关于」页，打不开时返回 false，改用浏览器打开发布页。在消息循环线程上调用。
    std::function<bool()> open_about;
    // 诊断日志的一行，可能在工作线程上调用；为空时不记。
    std::function<void(const std::string &)> log;
  };

  // 在调用线程上建一个不显示的窗口接收通知图标的消息，再起工作线程。建不出窗口时抛出 std::runtime_error，调用方记一条诊断后不做后台检查，不影响输入。
  explicit UpdateNotifier(Options options);
  ~UpdateNotifier();
  UpdateNotifier(const UpdateNotifier &) = delete;
  UpdateNotifier &operator=(const UpdateNotifier &) = delete;

  // 停下工作线程、删掉图标和窗口，可以重复调用。工作线程正在联网时最多等两秒，还没回来就放手让它自己结束：它只持有共享状态，回来后看见停止标志就退出，不再碰窗口；进程随后退出时操作系统收走它和共享层的文件锁。必须在创建它的线程上调用。
  void stop();

  struct State;

private:
  std::shared_ptr<State> state_;
};

} // namespace msime::windows

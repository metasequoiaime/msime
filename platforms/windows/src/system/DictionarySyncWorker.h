#pragma once
#include <functional>
#include <memory>
#include <nlohmann/json.hpp>
#include <string>

namespace msime::windows {

// Server 里「我的词条」经 WebDAV 网盘的自动同步（docs/dictionary-webdav-sync.md 的「宿主怎么接入」）。Server 是一直活着、持有输入会话的进程，所以自动触发都在这里：用户在候选窗口删了词之后以 local_change 记一次（只记录、不联网），之后按每次回答里的 `next_check_ms` 以 periodic 再调。打开设置窗口（opened）和「立即同步」（manual）由设置窗口自己调。调度规则在 DictionarySyncSchedule.h；要不要联网、额度、防抖和退避都由共享 Rust 决定。
//
// 所有调用都在这个类自己的工作线程上，不在输入线程、管道线程或消息循环上：除了 local_change 都会阻塞联网。Rust 写本机词库时每批取一次 Engine 的维护权、写完就放，从不跨网络请求持有；自动同步不请输入法让出词库，输入会话占着时这一轮的远端改动留到下一次（Rust 的 deferred），不打扰打字。
//
// 同步目录是本用户的 %LOCALAPPDATA%\<本版本的用户目录>\dictionary-sync（common/DictionarySyncDirectory.h），与设置窗口同一个，所以一个安装只有一份设备身份；两个进程第一次使用时同时生成身份由 Rust 的文件锁和「不覆盖另一个身份」挡住，仍然报 clipboard_keystore_conflict 时这里重试（common/DictionarySyncRetry.h）。
class DictionarySyncWorker {
public:
  struct Options {
    // Server 跑的 HostOptions 文档：偏好目录（读 dictionary_sync_webdav）和 Engine 词库路径都从它来，Server 运行期间不变。
    nlohmann::json host_options;
    // 本用户的同步目录，UTF-8。
    std::string directory;
    // 诊断日志的一行（只写错误码，不写网盘地址、账号或词条），在工作线程上调用；为空时不记。
    std::function<void(const std::string &)> log;
  };

  // 先建出来接住偏好和改动的通知，`start` 之前什么都不调用。`configured` 是启动时偏好里有没有配网盘。
  explicit DictionarySyncWorker(bool configured);
  ~DictionarySyncWorker();
  DictionarySyncWorker(const DictionarySyncWorker &) = delete;
  DictionarySyncWorker &operator=(const DictionarySyncWorker &) = delete;

  // 起工作线程，第一次 periodic 在两分钟后。起不了线程时抛出 std::system_error，调用方记一条诊断后不做自动同步，不影响输入。只调一次。
  void start(Options options);
  // 偏好发布时调用：网盘配上或清掉。线程安全，不阻塞。
  void configure(bool configured);
  // 候选窗口删了词之后调用。线程安全，只记一笔、唤醒工作线程，不等它。
  void note_local_change();
  // 停下工作线程，可以重复调用。正在联网时最多等两秒，还没回来就放手让它自己结束：它只持有共享状态，回来后看见停止标志就退出；进程随后退出时操作系统收走它和 Rust 的文件锁，下一次同步从写盘过的那一步接着做。
  void stop();

  struct State;

private:
  // stop 与在途通知可以重叠；状态指针在对象存活期间不变，停机只改锁内标志。
  const std::shared_ptr<State> state_;
};

} // namespace msime::windows

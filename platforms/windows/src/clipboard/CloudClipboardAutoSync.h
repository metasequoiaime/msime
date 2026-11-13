#pragma once

#include <condition_variable>
#include <filesystem>
#include <mutex>
#include <string>
#include <thread>

namespace msime::windows {

// Server 常驻期间同步系统剪贴板；网络和端到端加密仍由 Host API 负责。
class CloudClipboardAutoSync final {
public:
  explicit CloudClipboardAutoSync(std::filesystem::path directory);
  ~CloudClipboardAutoSync();
  CloudClipboardAutoSync(const CloudClipboardAutoSync &) = delete;
  void observe_local_text(std::string text);

private:
  void run();
  void upload(const std::string &text);
  void poll();
  bool request(const std::string &action, std::string *result);

  std::filesystem::path directory_;
  std::mutex mutex_;
  std::condition_variable wake_;
  std::thread worker_;
  bool stop_ = false;
  bool upload_pending_ = false;
  std::string pending_text_;
  std::string local_text_;
  std::string receiver_text_;
  std::string latest_remote_updated_;
  bool receiver_owned_ = false;
};

} // namespace msime::windows

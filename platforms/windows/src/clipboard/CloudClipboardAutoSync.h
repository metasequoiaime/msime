#pragma once

#include "CloudClipboardReceivePolicy.h"

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

private:
  void run();
  void poll();
  bool stopped();
  std::string current_binding();
  bool request(const std::string &action, const std::string &binding, std::string *result);

  std::filesystem::path directory_;
  std::mutex mutex_;
  std::condition_variable wake_;
  std::thread worker_;
  bool stop_ = false;
  bool initialized_ = false;
  std::uint32_t observed_sequence_ = 0;
  std::string binding_;
  CloudClipboardReceivePolicy policy_;
};

} // namespace msime::windows

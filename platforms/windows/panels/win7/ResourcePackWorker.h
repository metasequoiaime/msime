#pragma once

#include <nlohmann/json.hpp>
#include <condition_variable>
#include <cstdint>
#include <filesystem>
#include <functional>
#include <mutex>
#include <optional>
#include <string>
#include <thread>
#include <vector>

namespace msime::windows::win7 {

enum class ResourcePackAction { list, download, import_files, remove };

struct ResourcePackProgress {
  std::string phase;
  uint64_t done = 0;
  uint64_t total = 0;
};

struct ResourcePackResult {
  ResourcePackAction action;
  nlohmann::json response;
  nlohmann::json catalog;
};

struct ResourcePackUpdate {
  bool busy;
  bool cancelling;
  ResourcePackAction action;
  ResourcePackProgress progress;
  std::optional<ResourcePackResult> result;
};

// 下载独占工作线程，不占用笔划识别队列；通知只表示有新状态，数据由 take 取走。
class ResourcePackWorker final {
public:
  ResourcePackWorker(std::function<std::filesystem::path()> root, std::function<void()> notify);
  ~ResourcePackWorker();
  ResourcePackWorker(const ResourcePackWorker&) = delete;
  ResourcePackWorker& operator=(const ResourcePackWorker&) = delete;
  bool start(ResourcePackAction action, std::string pack,
      std::vector<std::filesystem::path> files = {});
  void cancel();
  ResourcePackUpdate take();

private:
  struct Job {
    ResourcePackAction action;
    std::string pack;
    std::vector<std::filesystem::path> files;
  };
  void run();
  void emit(bool notify) noexcept;
  bool changed();
  void cancel_locked();
  static void progress(void* context, const char* phase, uint64_t done, uint64_t total) noexcept;

  std::function<std::filesystem::path()> root_;
  std::function<void()> notify_;
  std::mutex mutex_;
  std::condition_variable wake_;
  bool stopping_ = false;
  bool busy_ = false;
  bool cancelling_ = false;
  bool notification_pending_ = false;
  ResourcePackAction action_ = ResourcePackAction::list;
  std::string pack_;
  std::optional<Job> pending_;
  ResourcePackProgress progress_;
  std::optional<ResourcePackResult> result_;
  std::thread thread_;
};

} // namespace msime::windows::win7

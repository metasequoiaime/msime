#include "ResourcePackWorker.h"
#include "msime_client.h"
#include "../../../common/HostApiString.h"

#include <utility>
#include <stdexcept>

namespace msime::windows::win7 {
namespace {
using Json = nlohmann::json;
const uint8_t* bytes(const std::string& text) { return reinterpret_cast<const uint8_t*>(text.data()); }
Json response(char* buffer) {
  auto owned = msime::host_api::own_string(buffer);
  return owned ? Json::parse(owned.get(), nullptr, false) : Json();
}
Json catalog(const std::filesystem::path& root) {
  const auto request = Json{{"state_root", root.u8string()}}.dump();
  return response(msime_client_resource_packs(bytes(request), request.size()));
}
Json saved_sources(const std::filesystem::path& root) {
  const auto directory = root.u8string();
  const auto snapshot = response(msime_client_load_preferences(bytes(directory), directory.size()));
  if (!snapshot.is_object() || !snapshot.value("ok", false)) throw std::runtime_error("preferences unavailable");
  const auto& preferences = snapshot.at("value").at("preferences");
  const auto mirror = preferences.at("voice_input").value("asr_model_mirror", "");
  return mirror.empty() ? Json::array() : Json::array({mirror});
}
} // namespace

ResourcePackWorker::ResourcePackWorker(std::function<std::filesystem::path()> root, std::function<void()> notify)
    : root_(std::move(root)), notify_(std::move(notify)), thread_([this] { run(); }) {}

ResourcePackWorker::~ResourcePackWorker() {
  {
    std::lock_guard<std::mutex> guard(mutex_);
    stopping_ = true;
    cancelling_ = true;
    pending_.reset();
    cancel_locked();
  }
  wake_.notify_one();
  thread_.join();
}

bool ResourcePackWorker::start(ResourcePackAction action, std::string pack, std::vector<std::filesystem::path> files) {
  {
    std::lock_guard<std::mutex> guard(mutex_);
    if (busy_ || stopping_) return false;
    action_ = action;
    pack_ = pack;
    busy_ = true;
    cancelling_ = false;
    progress_ = {};
    result_.reset();
    pending_ = Job{action, std::move(pack), std::move(files)};
  }
  wake_.notify_one();
  return true;
}

void ResourcePackWorker::cancel_locked() {
  if (busy_ && !pack_.empty() && (action_ == ResourcePackAction::download || action_ == ResourcePackAction::import_files))
    msime_client_resource_pack_cancel(bytes(pack_), pack_.size());
}

void ResourcePackWorker::cancel() {
  bool notify;
  {
    std::lock_guard<std::mutex> guard(mutex_);
    if (!busy_ || (action_ != ResourcePackAction::download && action_ != ResourcePackAction::import_files)) return;
    cancelling_ = true;
    // 与任务完成和下一次 start 串行，不能把旧取消发给下一次下载。
    cancel_locked();
    notify = changed();
  }
  emit(notify);
}

ResourcePackUpdate ResourcePackWorker::take() {
  std::lock_guard<std::mutex> guard(mutex_);
  notification_pending_ = false;
  return {busy_, cancelling_, action_, progress_, std::exchange(result_, std::nullopt)};
}

bool ResourcePackWorker::changed() {
  return !std::exchange(notification_pending_, true);
}

void ResourcePackWorker::emit(bool notify) noexcept {
  if (!notify || !notify_) return;
  try { notify_(); } catch (...) { /* 通知失败不展开到 C ABI，也不丢失可读取的结果。 */ }
}

void ResourcePackWorker::progress(void* context, const char* phase, uint64_t done, uint64_t total) noexcept {
  auto& worker = *static_cast<ResourcePackWorker*>(context);
  try {
    bool notify;
    {
      std::lock_guard<std::mutex> guard(worker.mutex_);
      worker.progress_ = {phase ? phase : "verify", done, total};
      // start 后、Rust 登记前的取消不能丢：首次回调已完成登记，尚未访问网络或文件。
      if (worker.cancelling_ || worker.stopping_) worker.cancel_locked();
      notify = worker.changed();
    }
    worker.emit(notify);
  } catch (...) {
    // C 回调不可抛出异常；内存不足时保留取消能力并让共享安装器安全退出。
    msime_client_resource_pack_cancel(bytes(worker.pack_), worker.pack_.size());
  }
}

void ResourcePackWorker::run() {
  for (;;) {
    std::unique_lock<std::mutex> guard(mutex_);
    wake_.wait(guard, [this] { return stopping_ || pending_.has_value(); });
    if (stopping_) return;
    auto job = std::move(*pending_);
    pending_.reset();
    guard.unlock();
    ResourcePackResult result{job.action, Json(), Json()};
    try {
      const auto root = root_();
      if (job.action == ResourcePackAction::list) {
        result.response = catalog(root);
        result.catalog = result.response;
      } else {
        Json request{{"state_root", root.u8string()}, {"pack", job.pack}};
        if (job.action == ResourcePackAction::remove) {
          const auto document = request.dump();
          result.response = response(msime_client_resource_pack_remove(bytes(document), document.size()));
        } else {
          if (job.action == ResourcePackAction::import_files) {
            request["files"] = Json::array();
            for (const auto& file : job.files) request["files"].push_back(file.u8string());
          } else request["sources"] = saved_sources(root);
          const auto document = request.dump();
          result.response = response(msime_client_resource_pack_install(bytes(document), document.size(), progress, this));
        }
        result.catalog = catalog(root);
      }
    } catch (...) {
      // 路径转换或响应错误不能让工作线程终止；界面显示固定错误，不回显本地路径。
      result.response = Json();
    }
    guard.lock();
    if (stopping_) return;
    result_ = std::move(result);
    busy_ = false;
    cancelling_ = false;
    const bool notify = changed();
    guard.unlock();
    emit(notify);
  }
}

} // namespace msime::windows::win7

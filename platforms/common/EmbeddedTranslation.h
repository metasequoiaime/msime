#pragma once

#include "HostApiString.h"
#include <atomic>
#include <chrono>
#include <functional>
#include <future>
#include <string>

namespace msime::translation {

inline std::string next_request_id() {
  static std::atomic<uint64_t> sequence{0};
  return "native-" + std::to_string(++sequence);
}

// 只在宿主后台线程运行。持续检查原宿主的失焦/代次标记，覆盖取消早于 Rust 登记请求的竞态。
inline host_api::OwnedString embedded(const std::string &request, const std::string &id,
                                     const std::function<bool()> &cancelled) noexcept try {
  if (cancelled()) return host_api::own_string(nullptr);
  auto future = std::async(std::launch::async, [request] {
    return host_api::own_string(msime_client_local_translation_query(
        reinterpret_cast<const uint8_t *>(request.data()), request.size()));
  });
  while (future.wait_for(std::chrono::milliseconds(20)) != std::future_status::ready) {
    if (cancelled())
      msime_client_local_translation_cancel(reinterpret_cast<const uint8_t *>(id.data()), id.size());
  }
  auto result = future.get();
  if (cancelled()) return host_api::own_string(nullptr);
  return result;
} catch (...) {
  return host_api::own_string(nullptr);
}

// 状态只由输入线程读写；工作线程仅持有取消标记，不引用宿主或客户端对象。
class EmbeddedState {
public:
  using Token = std::shared_ptr<std::atomic<bool>>;
  ~EmbeddedState() { cancel(); }
  void cancel() {
    if (token_) token_->store(true);
    token_.reset();
    identity_.clear();
  }
  void observe(const std::string &identity) {
    if (identity != identity_) cancel();
  }
  Token begin(const std::string &identity) {
    cancel();
    identity_ = identity;
    token_ = std::make_shared<std::atomic<bool>>(false);
    return token_;
  }
private:
  Token token_;
  std::string identity_;
};

} // namespace msime::translation

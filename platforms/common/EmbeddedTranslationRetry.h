#pragma once

#include "EmbeddedTranslation.h"
#include <algorithm>
#include <nlohmann/json.hpp>
#include <thread>

namespace msime::translation {

inline host_api::OwnedString embedded_with_retry(
    nlohmann::json query, const std::string &id,
    const std::function<bool()> &cancelled) noexcept try {
  using Clock = std::chrono::steady_clock;
  using Milliseconds = std::chrono::milliseconds;
  const auto timeout = query.value("timeout_ms", int64_t{30'000});
  // 非法预算仍由 Rust 边界拒绝，不在适配器里静默修正。
  if (timeout < 1 || timeout > 120'000) return embedded(query.dump(), id, cancelled);
  const auto deadline = Clock::now() + Milliseconds(timeout);
  auto result = host_api::own_string(nullptr);
  while (!cancelled()) {
    const auto remaining = std::chrono::duration_cast<Milliseconds>(deadline - Clock::now()).count();
    if (remaining <= 0) break;
    query["timeout_ms"] = remaining;
    result = embedded(query.dump(), id, cancelled);
    if (!result || cancelled()) return host_api::own_string(nullptr);
    const auto envelope = nlohmann::json::parse(result.get());
    if (envelope.value("ok", false) ||
        envelope.value("error", std::string{}) != "local_translation_busy") return result;
    // 旧请求取消尚未收尾时等待空位；退避和每次推理共用同一个截止时间。
    const auto retry = (std::min)(deadline, Clock::now() + Milliseconds(100));
    while (!cancelled() && Clock::now() < retry)
      std::this_thread::sleep_until((std::min)(retry, Clock::now() + Milliseconds(20)));
  }
  return cancelled() ? host_api::own_string(nullptr) : std::move(result);
} catch (...) {
  return host_api::own_string(nullptr);
}

} // namespace msime::translation

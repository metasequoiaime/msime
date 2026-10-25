#pragma once

#include <chrono>
#include <cstddef>
#include <cstdint>
#include <string_view>

namespace msime::dictionary_lease {

// 词库维护租约的格式，不碰文件系统，每个宿主都能包含：Linux 和 macOS 的读写在 DictionaryQuiesceLease.h，Windows TIP 的读取在 platforms/windows/common/DictionaryQuiesceLease.h。文件名、第一行上以 Unix 毫秒表示的过期时间（读者读到换行就停；每个写者都会在它后面再写一行 owner）以及这个上限，都与 crates/client-services/src/dictionary/quiesce.rs 共用。
inline constexpr std::string_view kDictionaryQuiesceLeaseName = ".msime-dictionary-quiesce";
// Rust writers use the same persistent advisory lock. It serializes the
// read-and-remove sequence with every publisher, so a writer cannot remove a
// newer writer's lease between checking its contents and unlinking it.
inline constexpr std::string_view kDictionaryQuiesceLeaseLockName = ".msime-dictionary-quiesce.lock";
inline constexpr std::int64_t kDictionaryQuiesceLeaseMaxMs = 30'000;
inline constexpr std::size_t kDictionaryQuiesceLeaseMaxBytes = 256;

inline bool dictionary_quiesce_lease_live(std::string_view contents, std::int64_t now_ms) {
  std::int64_t expiry = 0;
  bool digits = false;
  for (const char c : contents) {
    if (c == '\n') break;
    if (c < '0' || c > '9' || expiry > (INT64_MAX - 9) / 10) return false;
    expiry = expiry * 10 + (c - '0');
    digits = true;
  }
  return digits && expiry > now_ms && expiry - now_ms <= kDictionaryQuiesceLeaseMaxMs;
}

inline std::int64_t dictionary_quiesce_now_ms() {
  return std::chrono::duration_cast<std::chrono::milliseconds>(
             std::chrono::system_clock::now().time_since_epoch())
      .count();
}

}  // namespace msime::dictionary_lease

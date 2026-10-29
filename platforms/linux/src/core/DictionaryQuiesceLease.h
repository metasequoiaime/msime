#pragma once

#include "../../../common/DictionaryQuiesceLease.h"

#include <string>
#include <string_view>
#include <system_error>

namespace msime::linux_host {

// The lease itself is shared with the macOS input method (platforms/common/DictionaryQuiesceLease.h); the IBus and Fcitx5 hosts reach it under their own namespace.
using msime::dictionary_lease::dictionary_quiesce_lease_live;
using msime::dictionary_lease::dictionary_quiesce_now_ms;
using msime::dictionary_lease::dictionary_quiesced;
using msime::dictionary_lease::kDictionaryQuiesceLeaseMaxMs;
using msime::dictionary_lease::kDictionaryQuiesceLeaseName;

// A menu preference save writes into the state root beside `user_data`. Moving the data directory holds the lease on that user directory while it copies, so a save waits then rather than landing in the old root after it was copied; once the move has taken the old user directory away, a host still configured for it saves nothing, so it cannot put a preferences file back into the old root. No session can open on a missing user directory either.
inline bool preference_save_held(const std::string &user_data,
                                 std::int64_t now_ms = dictionary_quiesce_now_ms()) {
  if (user_data.empty() || user_data.front() != '/') return false;
  std::error_code error;
  return !std::filesystem::is_directory(user_data, error) || dictionary_quiesced(user_data, now_ms);
}

// Host API 在设置页持有独占词库锁时拒绝建会话给出的原因（crates/host-api/src/ffi/session.rs），与 client-services 的 `quiesce::BUSY` 是同一个字符串。
inline constexpr std::string_view kDictionaryMaintenanceBusy = "dictionary maintenance busy";

// `reply` 是 Host API 的整份回复（`{"ok":false,"error":...}`），说的是不是“词库正在维护”。租约还没挂上或已经过期、而设置页仍持着独占锁时，建会话会这样失败；这不是运行配置有错，宿主应当像看到租约一样把按键交给应用，等维护做完再建会话，而不是报“请检查运行配置”（#9918）。
template <class Document> bool dictionary_maintenance_busy(const Document &reply) {
  if (!reply.is_object()) return false;
  const auto ok = reply.find("ok");
  const auto error = reply.find("error");
  return ok != reply.end() && ok->is_boolean() && !ok->template get<bool>() &&
         error != reply.end() && error->is_string() &&
         error->template get_ref<const std::string &>() == kDictionaryMaintenanceBusy;
}

// 两个宿主建会话时对 `msime_client_create` 整份回复的同一个判断：词库正在维护时返回真，宿主不开会话、把按键交给应用；`noted` 记着这一段忙碌是否已经调过 `write`（写一条诊断日志），所以一段忙碌只写一条。会话建起来（`ok` 为真）时清掉记号，下一段忙碌再写一条；别的失败返回假、记号不动，由宿主照旧当成出错处理（#9918）。
template <class Document, class Write>
bool defer_session_for_dictionary_maintenance(const Document &reply, bool &noted, Write &&write) {
  if (dictionary_maintenance_busy(reply)) {
    if (!noted) write();
    noted = true;
    return true;
  }
  if (reply.is_object()) {
    const auto ok = reply.find("ok");
    if (ok != reply.end() && ok->is_boolean() && ok->template get<bool>()) noted = false;
  }
  return false;
}

}  // namespace msime::linux_host

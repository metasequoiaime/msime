#pragma once

#include "../../common/DictionaryQuiesceLeaseFormat.h"
#include "../src/system/StateRootLease.h"

#include <cstdint>
#include <filesystem>
#include <string>

namespace msime::windows {

// 词库维护租约的 Windows 读取，给各应用进程里的 TIP 用。设置窗口和 msime-mcp 维护词库时，除了经辅助管道让 Server 放开它的会话，也在 user_data 下写这份租约（crates/client-services/src/dictionary/quiesce.rs 的 `WindowsRelease`）；TIP 自己的宿主会话以共享方式持有同一把词库锁，Server 管不到它，只能由 TIP 看到租约后自己放开。读法与 Linux 和 macOS 宿主相同：租约路径上有符号链接或目录联接、文件不是单链接的普通文件、读不出或过期时都当作没有租约。只读第一行，不跟随重解析点，打开时带 FILE_SHARE_DELETE，写者随时能把新租约改名盖上来。
inline bool dictionary_quiesced(const std::filesystem::path &user_data,
                                std::int64_t now_ms = msime::dictionary_lease::dictionary_quiesce_now_ms()) {
  if (user_data.empty() || !user_data.is_absolute())
    return false;
  const auto lease = user_data / std::string(msime::dictionary_lease::kDictionaryQuiesceLeaseName);
  // 每个 TIP 线程每 500 毫秒问一次，绝大多数时候没有租约：先用一次属性查询排除，不必每次都逐级检查上级目录。
  if (GetFileAttributesW(lease.c_str()) == INVALID_FILE_ATTRIBUTES)
    return false;
  const auto contents = read_private_file(lease, msime::dictionary_lease::kDictionaryQuiesceLeaseMaxBytes);
  return contents && msime::dictionary_lease::dictionary_quiesce_lease_live(*contents, now_ms);
}

} // namespace msime::windows

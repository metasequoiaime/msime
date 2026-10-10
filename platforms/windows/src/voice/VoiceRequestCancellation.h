#pragma once

#include <algorithm>
#include <atomic>
#include <memory>
#include <vector>

namespace msime::windows {
// 只保留仍被识别任务持有的取消令牌；已完成任务的 future 被清理后，向量不能无限增长。
inline void prune_completed_voice_requests(
    std::vector<std::shared_ptr<std::atomic_bool>> &requests) {
  requests.erase(
      std::remove_if(requests.begin(), requests.end(),
                     [](const auto &request) { return request.use_count() == 1; }),
      requests.end());
}
} // namespace msime::windows

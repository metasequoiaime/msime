#pragma once

#include "../ipc/PipeTicket.h"

#include <cstdint>
#include <utility>
#include <vector>

namespace msime::windows {

// 记住每个已注册 TSF 管道是否收到当前配置；同一进程里的新宿主或重连会拿到新的代次。
class TsfConfigDeliveryTracker final {
public:
  bool needs_send(uint64_t revision,
                  const std::vector<PipeTicket> &current) const {
    if (revision != applied_revision_)
      return true;
    if (current.size() != delivered_.size())
      return true;
    for (size_t i = 0; i < current.size(); ++i)
      if (!same_ticket(current[i], delivered_[i]))
        return true;
    return false;
  }

  void mark_sent(uint64_t revision, std::vector<PipeTicket> current) {
    applied_revision_ = revision;
    delivered_ = std::move(current);
  }

private:
  uint64_t applied_revision_ = 0;
  std::vector<PipeTicket> delivered_;
};

} // namespace msime::windows

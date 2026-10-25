#pragma once
#include "../ipc/PipeTicket.h"

namespace msime::windows {
// 焦点身份的数据副本，供跨线程界面通知校验，不引入焦点门的操作接口。
struct FocusLease {
  PipeTicket transport;
  uint64_t epoch = 0;
  uint64_t token = 0;
};
} // namespace msime::windows

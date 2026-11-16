#include "CloudClipboardReceivePolicy.h"

#include <cassert>
#include <iostream>

using namespace msime::windows;

int main() {
  const auto first = clipboard_remote_revision("2026-11-13T00:00:00Z");
  const auto newer = clipboard_remote_revision("2026-11-13T00:00:01.123456Z");
  assert(first && newer && *newer > *first);
  assert(first == clipboard_remote_revision("2026-11-13T00:00:00.000000+00:00"));
  for (const auto *invalid : {"synthetic-time", "2026-02-30T00:00:00Z", "2026-11-13T99:00:00Z",
                              "2026-11-13T00:00:00.Z", "2026-11-13T00:00:00.1234567890Z"})
    assert(!clipboard_remote_revision(invalid));

  CloudClipboardReceivePolicy policy;
  policy.begin(7, true);
  assert(!policy.receive(first, 7));
  assert(policy.receive(newer, 7));
  assert(policy.receive(newer, 7)); // 写入失败不得消耗修订。
  policy.received(*newer, 8);
  assert(!policy.receive(newer, 8)); // 接收写入不触发回传。
  assert(!policy.receive(first, 8));
  const auto later = clipboard_remote_revision("2026-11-13T00:00:02Z");
  assert(!policy.receive(later, 9)); // 请求期间本机复制优先。
  const auto latest = clipboard_remote_revision("2026-11-13T00:00:03Z");
  assert(policy.receive(latest, 9));
  policy.begin(9, true); // 换账号或中继重置水位。
  assert(!policy.receive(std::nullopt, 9));
  assert(policy.receive(first, 9)); // 空列表后第一条复制可达。
  std::cout << "云剪贴板 UTC 修订、水位、写入失败、回声及并发复制通过\n";
}

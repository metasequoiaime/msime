#include "../../HostSessionMaintenance.h"
#include <cstdlib>

// 词库维护期间 TIP 宿主会话的去留（HostSessionMaintenance.h，#9434）。
int main() {
  using msime::tsf::host_session_maintenance_step;
  using msime::tsf::host_session_waits_for_maintenance;
  using Step = msime::tsf::HostSessionMaintenanceStep;
  int failures = 0;
  const auto expect = [&failures](bool condition) {
    if (!condition)
      ++failures;
  };

  // 租约有效：开着的会话放开，正在组字时等下一轮。
  expect(host_session_maintenance_step(true, true, false, false) == Step::Release);
  expect(host_session_maintenance_step(true, true, false, true) == Step::Keep);
  // 已经放开的会话在租约有效期间不重建，否则又拿回共享锁。
  expect(host_session_maintenance_step(true, false, true, false) == Step::Keep);
  expect(host_session_maintenance_step(true, false, false, false) == Step::Keep);

  // 租约撤掉：为维护放开的会话重建；会话不在时组合在旧组字流程里，正在组字时等这次组合结束，不在组字途中换成空的宿主会话。
  expect(host_session_maintenance_step(false, false, true, false) == Step::Restore);
  expect(host_session_maintenance_step(false, false, true, true) == Step::Keep);
  // 不是为维护放开的会话（建不起来、没有宿主选项）不在这里一轮一轮地重建；开着的会话不动。
  expect(host_session_maintenance_step(false, false, false, false) == Step::Keep);
  expect(host_session_maintenance_step(false, true, false, false) == Step::Keep);
  expect(host_session_maintenance_step(false, true, true, false) == Step::Keep);

  // 维护者还拿着独占锁时下一轮再试；别的失败不再等。
  expect(host_session_waits_for_maintenance("dictionary maintenance busy"));
  expect(!host_session_waits_for_maintenance("dictionary access unavailable"));
  expect(!host_session_waits_for_maintenance(""));

  return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

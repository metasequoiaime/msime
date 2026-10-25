#pragma once

#include <string_view>

namespace msime::tsf {

// TIP 的宿主会话在词库维护期间的去留。每个用着输入法的应用进程里，TIP 都从 runtime-options.json 建了一个宿主会话，它以共享方式持有词库锁；设置窗口或 msime-mcp 新增词条、导入、清除学习数据要独占这把锁，而 Server 的 DictionaryQuiesce 只放开 Server 自己的会话（#9434）。维护者在 user_data 下写租约（../common/DictionaryQuiesceLease.h），TIP 在偏好定时器上按这里的决定放开或重建：
// - 租约有效、会话开着、没有在组字：放开（Release）。正在组字时等下一轮，不丢掉屏幕上的组合；维护者在 2.5 秒内一直重试。
// - 租约已撤、会话是为维护放开的（或激活时因为维护没建起来）、没有在组字：重建（Restore）。会话不在时 TIP 用自己的旧组字流程（KeyHandler 只在宿主会话无效时走它），组合存在 `_keystrokeBuffer` 里；组字途中建起空的宿主会话，按键状态和后续按键就改读宿主那边，已输入的字母被丢下或换掉，退格和空格按空组合判定、漏进文档。所以和放开一样，等这次组合结束再重建。
// - 其余情况不动（Keep）：没有为维护放开过的会话，不论因为什么没建起来，都不在这里一轮一轮地重建。
enum class HostSessionMaintenanceStep { Keep, Release, Restore };

inline HostSessionMaintenanceStep host_session_maintenance_step(bool quiesced, bool session_open,
                                                                bool released_for_maintenance,
                                                                bool composing) {
  if (quiesced)
    return session_open && !composing ? HostSessionMaintenanceStep::Release
                                      : HostSessionMaintenanceStep::Keep;
  return !session_open && released_for_maintenance && !composing
             ? HostSessionMaintenanceStep::Restore
             : HostSessionMaintenanceStep::Keep;
}

// 建会话失败后是否还算「在等维护结束」：维护者撤了租约、还没放下独占锁的那一刻，宿主库答 `dictionary maintenance busy`（msime_client_create），下一轮再试。别的失败和激活时建不起来一样，不再重试。
inline bool host_session_waits_for_maintenance(std::string_view error) {
  return error == "dictionary maintenance busy";
}

} // namespace msime::tsf

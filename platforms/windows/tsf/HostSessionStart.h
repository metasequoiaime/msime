#pragma once

#include "EngineSessionAdapter.h"
#include <string>

namespace msime::tsf {

// TIP 新建宿主会话的唯一入口：先按 runtime-options.json 建会话，再立即套上用户当前的偏好快照。runtime-options.json 是 Server 首次准备状态目录时写下的，此后不再改写，里面的 preferences 是那时的版本默认值（方案是全拼）；用户选的方案只在偏好快照里（HostConfiguration 的 preferenceSnapshot，msime_client_load_preferences 读出的那份）。只建不套的会话就停在全拼，而候选窗由 Server 那份双拼会话画：小鹤双拼打 qkkl 选「情况」，TIP 这份按全拼只消耗 qk，上屏「情况kl」（#10867、#10887）。偏好套不上（快照为空或被拒）时会话仍然可用，偏好定时器之后还会再套，所以只把建会话本身的成败交给调用方。
inline bool create_host_session(EngineSessionAdapter &adapter, const std::string &options,
                                const std::string &preference_snapshot, std::string *error) {
  if (!adapter.create(options, error)) return false;
  if (!preference_snapshot.empty()) {
    std::string ignored, preference_error;
    (void)adapter.update_preferences(preference_snapshot, &ignored, &preference_error);
  }
  return true;
}

} // namespace msime::tsf

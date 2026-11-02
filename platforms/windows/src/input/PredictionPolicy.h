#pragma once
#include <nlohmann/json.hpp>
#include <string>

namespace msime::windows {
// 共享会话的 view 是不是上屏后联想（msime_client.h 的 View.prediction）：没有组字，候选是联想的后续文字。数字键选这一页对应的行，空格选高亮的行，点选选点中的行，回车和标点照常输入；Server 的候选窗在 TIP 把它显示到光标处之后才露出来，见 CandidateMailbox 和 FocusedSession::hide_candidate。
inline bool idle_prediction_view(const nlohmann::json &view) {
  if (!view.is_object() || !view.value("prediction", false))
    return false;
  const auto candidates = view.find("candidates");
  return view.value("editing_text", std::string{}).empty() && candidates != view.end() &&
         candidates->is_array() && !candidates->empty();
}
} // namespace msime::windows

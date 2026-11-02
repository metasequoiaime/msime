// 上屏后联想（view 的 prediction）进候选窗的投影：没有拼音也画；没有 prediction 标记的空组字照旧不画，UI-less 宿主、失焦时也不画。送达的按键和异步刷新两条投影规则相同。
#include "CandidatePresentation.h"
#include "PredictionPolicy.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Prediction presentation test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 5}, {"index", index}}},
                        {"text", text},
                        {"highlighted", index == 0},
                        {"annotation", ""},
                        {"translation", ""}};
}
// 选中「吃」之后的联想：editing_text、preedit 都为空，候选是后续文字。共享内核只在联想时写 prediction。
nlohmann::json view(bool prediction, bool focused = true) {
  auto value = nlohmann::json{{"session", 1},
                              {"generation", 5},
                              {"focused", focused},
                              {"scheme", 0},
                              {"local_mode", "none"},
                              {"preedit", ""},
                              {"editing_text", ""},
                              {"caret_position", 0},
                              {"candidates", {candidate(0, "饭"), candidate(1, "东西"), candidate(2, "完了")}}};
  if (prediction)
    value["prediction"] = true;
  return value;
}
CandidatePresentation delivered(const FocusLease &lease, const nlohmann::json &value,
                                const std::string &next_prefix = {}, uint32_t modifiers = 0) {
  PendingReply reply{};
  reply.source = {42, 1, 7, true, nlohmann::json{{"commit", "吃"}, {"view", value}}};
  reply.next_prefix = next_prefix;
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.request_id = 7;
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.modifiers_down = modifiers;
  return candidate_presentation(lease, reply, packet);
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};

    // 判据：标了 prediction、没有组字、有候选。
    require(idle_prediction_view(view(true)));
    require(!idle_prediction_view(view(false)));
    auto composing = view(true);
    composing["editing_text"] = "chi";
    require(!idle_prediction_view(composing));
    auto empty = view(true);
    empty["candidates"] = nlohmann::json::array();
    require(!idle_prediction_view(empty));
    require(!idle_prediction_view(nlohmann::json()));

    // 异步刷新（释义、云候选）的投影：联想照样画，拼音为空，不画插入点。
    const auto refreshed = candidate_presentation_from_view(lease, view(true), 10, 20, "");
    require(refreshed.visible && refreshed.prediction && refreshed.preedit.empty());
    require(refreshed.preedit_caret == std::string::npos);
    require(refreshed.candidates.size() == 3 && refreshed.candidates[0].text == "饭" &&
            refreshed.candidates[0].highlighted && refreshed.candidates[1].index == 1);
    require(!refreshed.shuangpin_keymap);

    // 没有 prediction 的空组字是组字结束，不画。
    const auto ended = candidate_presentation_from_view(lease, view(false), 0, 0, "");
    require(!ended.visible && !ended.prediction && ended.composition_ended);
    // 失焦时联想也不画。
    require(!candidate_presentation_from_view(lease, view(true, false), 0, 0, "").visible);

    // 送达按键的投影规则相同；上屏留下的前缀不再画成拼音。
    const auto shown = delivered(lease, view(true), "吃");
    require(shown.visible && shown.prediction && shown.preedit.empty() && shown.candidates.size() == 3);
    require(!delivered(lease, view(false)).visible);
    // UI-less 宿主自己画候选，Server 不画联想。
    const auto uiless = delivered(lease, view(true), {}, FanyImePipeFlags::UiLess);
    require(!uiless.visible && !uiless.prediction && !uiless.composition_ended);

    // 普通组字的投影不带 prediction。
    auto ordinary = view(false);
    ordinary["editing_text"] = "chi";
    ordinary["preedit"] = "chi";
    const auto typing = candidate_presentation_from_view(lease, ordinary, 0, 0, "");
    require(typing.visible && !typing.prediction && typing.preedit == "chi");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

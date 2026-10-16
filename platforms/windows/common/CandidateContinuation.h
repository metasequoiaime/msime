#pragma once
#include <nlohmann/json.hpp>

namespace msime::windows {
// 两进程的 session/generation、候选排序和异步释义各自变化，只有组字语义参与前后置核对。
inline nlohmann::json candidate_continuation_state(const nlohmann::json &view) {
    return {{"editing_text", view.at("editing_text")}, {"preedit", view.at("preedit")},
            {"caret_position", view.at("caret_position")}, {"scheme", view.at("scheme")},
            {"local_mode", view.at("local_mode")}, {"dedicated_english", view.at("dedicated_english")},
            {"reading", view.value("reading", std::string{})},
            {"conversion", view.value("conversion", std::string{})},
            {"conversion_focus_start", view.value("conversion_focus_start", std::size_t{})},
            {"conversion_focus_end", view.value("conversion_focus_end", std::size_t{})},
            {"phrase_prefix", view.value("phrase_prefix", std::string{})}};
}
inline std::string candidate_continuation_request(const nlohmann::json &before, const std::string &candidate,
                                                   const nlohmann::json &after) {
    return nlohmann::json{{"version", 1}, {"before", candidate_continuation_state(before)},
                          {"candidate", candidate}, {"after", candidate_continuation_state(after)}}
        .dump(-1, ' ', true);
}
} // namespace msime::windows

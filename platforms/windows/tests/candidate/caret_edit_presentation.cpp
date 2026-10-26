// 光标后编辑（偏好 `caret_edit_suffix`）进候选窗的投影：view 的 caret_edit_suffix 和 caret_edit_prefix 两条投影都读；首行右侧只在生效时有提示；首选开头画淡的那一段只落在 Engine 下标为 0、而且确实以固定那段开头的一行上。模式没有生效时投影和以前一样。
#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Caret edit presentation test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 2}, {"index", index}}},
                        {"text", text},
                        {"highlighted", index == 0},
                        {"code", "chihuoguo"},
                        {"annotation", ""},
                        {"translation", ""}};
}
// 光标后编辑生效时的一页：首选是整句，开头是光标前那段的转换「我们」；后面几行只是光标后那段的候选，第二行恰好也以「我们」开头，用来确认只有首选画淡。没有生效时两个键都不出现，和共享内核的 JSON 一样。
nlohmann::json view(bool active) {
  auto value = nlohmann::json{{"session", 1},
                              {"generation", 2},
                              {"focused", true},
                              {"scheme", 0},
                              {"local_mode", "none"},
                              {"preedit", "women chihuoguo"},
                              {"editing_text", "womenchihuoguo"},
                              {"candidates",
                               {candidate(0, "我们吃火锅"), candidate(1, "我们吃"), candidate(2, "吃火锅")}}};
  if (active) {
    value["caret_edit_suffix"] = true;
    value["caret_edit_prefix"] = "我们";
  }
  return value;
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    // 生效时两项都抄进投影，首行右侧有提示，首选开头的「我们」画淡，别的行即使也以它开头也不画淡。
    const auto active = candidate_presentation_from_view(lease, view(true), 0, 0, "");
    require(active.visible && active.candidates.size() == 3);
    require(active.caret_edit_suffix && active.caret_edit_prefix == "我们");
    require(candidate_header_hint(active) == "候选编辑：光标后");
    require(candidate_caret_edit_dim(active, active.candidates[0]) == std::string("我们").size());
    require(candidate_caret_edit_dim(active, active.candidates[1]) == 0);
    require(candidate_caret_edit_dim(active, active.candidates[2]) == 0);
    // 首选的文字仍是整句，画淡只是画法，上屏、读屏和悬停提示都拿全文。
    require(active.candidates[0].text == "我们吃火锅");
    require(candidate_tooltip_text(active.candidates[0]) == "我们吃火锅");

    // 没有生效：两项都是空的，没有提示，也没有画淡的一段。
    const auto plain = candidate_presentation_from_view(lease, view(false), 0, 0, "");
    require(!plain.caret_edit_suffix && plain.caret_edit_prefix.empty());
    require(candidate_header_hint(plain).empty());
    require(candidate_caret_edit_dim(plain, plain.candidates[0]) == 0);

    // 生效但固定那段为空（光标前还没有转换出东西）：有提示，不画淡。
    auto empty_prefix = view(true);
    empty_prefix.erase("caret_edit_prefix");
    const auto unprefixed = candidate_presentation_from_view(lease, empty_prefix, 0, 0, "");
    require(unprefixed.caret_edit_suffix && unprefixed.caret_edit_prefix.empty());
    require(!candidate_header_hint(unprefixed).empty());
    require(candidate_caret_edit_dim(unprefixed, unprefixed.candidates[0]) == 0);

    // 首选不以固定那段开头时宁可不画淡。
    auto mismatched = view(true);
    mismatched["caret_edit_prefix"] = "你们";
    const auto other = candidate_presentation_from_view(lease, mismatched, 0, 0, "");
    require(other.caret_edit_prefix == "你们");
    require(candidate_caret_edit_dim(other, other.candidates[0]) == 0);

    // 繁体输出：固定那段和候选文字按同样的写法换算，画淡的仍是首选开头那两个字。
    const auto traditional = candidate_presentation_from_view(lease, view(true), 0, 0, "", true);
    require(traditional.caret_edit_prefix == "我們");
    require(traditional.candidates[0].text.compare(0, std::string("我們").size(), "我們") == 0);
    require(candidate_caret_edit_dim(traditional, traditional.candidates[0]) ==
            std::string("我們").size());

    // Ctrl+Enter 释义页上的义项不是光标后那段的候选：不带提示，不画淡。
    auto glossary = view(true);
    glossary["translation_page"] = 3;
    const auto page = candidate_presentation_from_view(lease, glossary, 0, 0, "");
    require(page.translation_page == 3 && !page.caret_edit_suffix && page.caret_edit_prefix.empty());

    // 组字结束（候选窗收起）时不带这两项。
    auto ended = view(true);
    ended["editing_text"] = "";
    require(!candidate_presentation_from_view(lease, ended, 0, 0, "").caret_edit_suffix);

    // 送达按键时的投影（Server 画候选窗用的那一条）读同样的两个键。
    PendingReply reply{};
    reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", view(true)}}};
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.request_id = 7;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    const auto delivered = candidate_presentation(lease, reply, packet);
    require(delivered.caret_edit_suffix && delivered.caret_edit_prefix == "我们");
    require(candidate_caret_edit_dim(delivered, delivered.candidates[0]) == std::string("我们").size());
    reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", view(false)}}};
    const auto delivered_plain = candidate_presentation(lease, reply, packet);
    require(!delivered_plain.caret_edit_suffix && delivered_plain.caret_edit_prefix.empty());
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

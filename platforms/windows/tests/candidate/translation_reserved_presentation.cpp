// view 候选的 translation_reserved（候选词翻译范围是「仅当前高亮」时的高亮行）进候选窗的投影：竖排候选窗只凭它给高亮行一直留出释义行，读漏了，移动高亮时窗口就会一会儿高一行一会儿矮一行。两条投影路径（按键送达时的 candidate_presentation、释义或云候选到达时的 candidate_presentation_from_view）都要读它，键缺省时为假。
#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Translation reserved presentation test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text, bool highlighted) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 2}, {"index", index}}},
                        {"text", text},
                        {"highlighted", highlighted},
                        {"annotation", ""}};
}
// 高亮在第二行：只有它带 translation_reserved，其他行没有这个键（Rust 为假时不写）。第一行带着离线释义。
nlohmann::json scoped_view() {
  auto first = candidate(0, "你好", false);
  first["translation"] = "hello";
  auto second = candidate(1, "拟好", true);
  second["translation_reserved"] = true;
  return nlohmann::json{{"session", 1},
                        {"generation", 2},
                        {"focused", true},
                        {"scheme", 0},
                        {"local_mode", "none"},
                        {"preedit", "nihao"},
                        {"editing_text", "nihao"},
                        {"candidates", {first, second, candidate(2, "你号", false)}}};
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    const auto projected = candidate_presentation_from_view(lease, scoped_view(), 0, 0, "");
    require(projected.visible && projected.candidates.size() == 3);
    require(!projected.candidates[0].translation_reserved);
    require(projected.candidates[1].translation_reserved && projected.candidates[1].highlighted);
    require(!projected.candidates[2].translation_reserved);
    // 留着释义行的那一行照样只画视图给的 translation：译文还没到时是空的，别的行的离线释义原样画。
    require(projected.candidates[1].translation.empty());
    require(projected.candidates[0].translation == "hello");

    PendingReply reply{};
    reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", scoped_view()}}};
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.request_id = 7;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    const auto delivered = candidate_presentation(lease, reply, packet);
    require(delivered.candidates.size() == 3);
    require(!delivered.candidates[0].translation_reserved);
    require(delivered.candidates[1].translation_reserved);
    require(!delivered.candidates[2].translation_reserved);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

// 云占位行（来源 15，文字「☁」）不进候选窗的投影（#9273），高亮也就只能在看得见的行上。运行时移动高亮时越过这一行（#10665），这里钉住两头：高亮在看得见的行上时投影照常生成、编号连续；高亮落在占位行上时投影仍然拒绝，这条不变式不放宽，否则候选窗会画出一页没有高亮的候选。
#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Cloud pending presentation test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text, unsigned source, size_t highlighted) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 2}, {"index", index}}},
                        {"text", text},
                        {"source", source},
                        {"highlighted", index == highlighted},
                        {"annotation", ""},
                        {"translation", ""}};
}
// 引擎在第二格放云占位行，运行时把高亮放在第 highlighted 行。
nlohmann::json cloud_pending_view(size_t highlighted) {
  return nlohmann::json{{"session", 1},
                        {"generation", 2},
                        {"focused", true},
                        {"scheme", 0},
                        {"local_mode", "none"},
                        {"preedit", "a"},
                        {"editing_text", "a"},
                        {"candidates",
                         {candidate(0, "甲", 0, highlighted),
                          candidate(1, "☁", candidate_source_cloud_pending, highlighted),
                          candidate(2, "乙", 0, highlighted), candidate(3, "丙", 0, highlighted)}}};
}
bool rejected(const nlohmann::json &view) {
  const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
  try {
    (void)candidate_presentation_from_view(lease, view, 0, 0, "");
  } catch (const std::invalid_argument &) {
    return true;
  }
  return false;
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    // ↓ 越过占位行后高亮在「乙」上：占位行不画，其余三行照常，唯一的高亮就是「乙」，它仍带自己的引擎下标。
    const auto moved = candidate_presentation_from_view(lease, cloud_pending_view(2), 0, 0, "");
    require(moved.visible && moved.candidates.size() == 3);
    require(moved.candidates[0].text == "甲" && !moved.candidates[0].highlighted);
    require(moved.candidates[1].text == "乙" && moved.candidates[1].highlighted &&
            moved.candidates[1].index == 2);
    require(moved.candidates[2].text == "丙" && !moved.candidates[2].highlighted);

    // 送达按键时的投影（Server 在输入队列里画候选窗用的那一条，#10665 就抛在这里）走同一条规则。
    PendingReply reply{};
    reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", cloud_pending_view(2)}}};
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.request_id = 7;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    const auto delivered = candidate_presentation(lease, reply, packet);
    require(delivered.visible && delivered.candidates.size() == 3);
    require(delivered.candidates[1].text == "乙" && delivered.candidates[1].highlighted);

    // 高亮落在占位行上时，看得见的行里一个高亮都没有，投影拒绝。运行时不能把高亮放在这里。
    require(!rejected(cloud_pending_view(0)));
    require(rejected(cloud_pending_view(1)));
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Cloud pending presentation checks passed\n";
  return 0;
}

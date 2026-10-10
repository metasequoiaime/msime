// Whether the candidate window takes the mouse for a list. A Zhuyin list is driven from the keyboard only, because its conversion composes in the TIP's own host session, which a row picked or a page turned in the Server's window would leave behind; every other list, the Korean Hanja list included, stays clickable.
#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Candidate pointer input test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json view(unsigned scheme_number) {
  return nlohmann::json{{"session", 1},
                        {"generation", 2},
                        {"focused", true},
                        {"scheme", scheme_number},
                        {"local_mode", "none"},
                        {"dedicated_english", false},
                        {"preedit", "你"},
                        {"editing_text", "su3"},
                        {"page", 0},
                        {"page_count", 2},
                        {"candidates",
                         {{{"id", {{"session", 1}, {"generation", 2}, {"index", 0}}}, {"text", "你"}, {"highlighted", true}},
                          {{"id", {{"session", 1}, {"generation", 2}, {"index", 1}}}, {"text", "妳"}, {"highlighted", false}}}}};
}
} // namespace

int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    for (unsigned number = 0; number <= 9; ++number) {
      const bool keyboard_only = number == static_cast<unsigned>(scheme::Zhuyin);
      // The projection the Server draws from a view it reads itself.
      const auto projected = candidate_presentation_from_view(lease, view(number), 0, 0, "");
      require(projected.visible && projected.candidates.size() == 2 && projected.page_count == 2);
      require(projected.pointer_input == !keyboard_only);

      // The projection it draws from a delivered key reply follows the same rule.
      PendingReply reply{};
      reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", view(number)}}};
      FanyImeNamedpipeData packet{};
      packet.client_id = 42;
      packet.request_id = 7;
      packet.event_type = FanyImePipeEventType::KeyEvent;
      const auto delivered = candidate_presentation(lease, reply, packet);
      require(delivered.visible && delivered.pointer_input == !keyboard_only);
      // 释义只在全拼、双拼、五笔和韩文里请求，横排候选窗只为这些方案预留释义行。
      const bool glossed = number == static_cast<unsigned>(scheme::Quanpin) ||
                           number == static_cast<unsigned>(scheme::Shuangpin) ||
                           number == static_cast<unsigned>(scheme::Wubi) ||
                           number == static_cast<unsigned>(scheme::Korean);
      require(projected.shows_glosses == glossed && delivered.shows_glosses == glossed);
    }
    // 临时日文组字和网址模式不请求释义，和 host-api 的翻译查询一样。
    for (const char *mode : {"temporary_japanese", "url"}) {
      auto local = view(static_cast<unsigned>(scheme::Quanpin));
      local["local_mode"] = mode;
      require(!candidate_presentation_from_view(lease, local, 0, 0, "").shows_glosses);
    }
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Candidate pointer input checks passed\n";
  return 0;
}

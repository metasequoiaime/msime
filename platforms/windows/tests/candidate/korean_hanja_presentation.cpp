// A Korean Hanja row carries its 훈음 as the Engine's annotation. The projection moves it off the Hanja's line onto the smaller secondary run, above any translation, and keeps it out of `translation`, which is the only text a translation shortcut may commit.
#include "CandidatePresentation.h"
#include <iostream>
#include <optional>
#include <stdexcept>
#include <string>
#include <utility>
#include <vector>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Korean Hanja presentation test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text, const std::string &annotation,
                         const std::string &translation = {}) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 2}, {"index", index}}},
                        {"text", text},
                        {"highlighted", index == 0},
                        {"code", "gks"},
                        {"annotation", annotation},
                        {"translation", translation}};
}
nlohmann::json view() {
  return nlohmann::json{{"session", 1},
                        {"generation", 2},
                        {"focused", true},
                        {"scheme", 4},
                        {"local_mode", "none"},
                        {"dedicated_english", false},
                        {"preedit", "한"},
                        {"editing_text", "gks"},
                        {"candidates",
                         {candidate(0, "韓", "나라 이름 한, 한나라 한", "Korea"), candidate(1, "漢", "한수 한"),
                          candidate(2, "寒", ""), candidate(3, "閑", "", "leisure")}}};
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    const auto projected = candidate_presentation_from_view(lease, view(), 0, 0, "");
    require(projected.visible && projected.candidates.size() == 4);
    const auto &korea = projected.candidates[0];
    // The main run is the Hanja alone; the 훈음 goes on the secondary run with the translation on the line under it.
    require(korea.text == "韓" && korea.annotation.empty());
    require(korea.gloss == "나라 이름 한, 한나라 한" && korea.translation == "Korea");
    require(candidate_secondary_text(korea) == "나라 이름 한, 한나라 한\nKorea");
    require(candidate_secondary_lines(korea) == 2);
    // Without a translation the 훈음 still shows, on one line.
    const auto &han = projected.candidates[1];
    require(han.annotation.empty() && han.gloss == "한수 한" && han.translation.empty());
    require(candidate_secondary_text(han) == "한수 한" && candidate_secondary_lines(han) == 1);
    // A Hanja without a 훈음 shows only what translation it has, or nothing.
    require(candidate_secondary_text(projected.candidates[2]).empty());
    require(candidate_secondary_text(projected.candidates[3]) == "leisure");
    require(candidate_secondary_lines(projected.candidates[3]) == 1);
    // Hanja are never converted to Traditional by the output toggle, which they already are, and the 훈음 is not Chinese text to convert.
    const auto traditional = candidate_presentation_from_view(lease, view(), 0, 0, "", true);
    require(traditional.candidates[0].gloss == "나라 이름 한, 한나라 한");

    // Outside a Hanja list the annotation stays where it was: other schemes, Korean's dedicated English mode and its local modes.
    for (const auto &[field, value] : {std::pair<const char *, nlohmann::json>{"scheme", 0},
                                       {"dedicated_english", true},
                                       {"local_mode", "unicode"}}) {
      auto other = view();
      other[field] = value;
      const auto kept = candidate_presentation_from_view(lease, other, 0, 0, "");
      require(kept.candidates[0].annotation == "나라 이름 한, 한나라 한");
      require(kept.candidates[0].gloss.empty() && kept.candidates[0].translation == "Korea");
      require(candidate_secondary_text(kept.candidates[0]) == "Korea");
    }

    // The reply projection, which the Server draws from the delivered key, applies the same rule.
    PendingReply reply{};
    reply.source = {42, 1, 7, false, nlohmann::json{{"commit", nullptr}, {"view", view()}}};
    FanyImeNamedpipeData packet{};
    packet.client_id = 42;
    packet.request_id = 7;
    packet.event_type = FanyImePipeEventType::KeyEvent;
    const auto delivered = candidate_presentation(lease, reply, packet);
    require(delivered.candidates[0].annotation.empty() && delivered.candidates[0].gloss == "나라 이름 한, 한나라 한" &&
            delivered.candidates[0].translation == "Korea");

    // 两种目标语言的释义每种一行（会话里用 U+2028 分行），读音在各自那一行下面单独一行；훈음 仍在最上面。
    auto both = korea;
    both.translation = "Korea\xE2\x80\xA8" "韓国";
    both.pronunciation = "/kəˈriːə/";
    require(candidate_secondary_text(both) == "나라 이름 한, 한나라 한\nKorea\n/kəˈriːə/\n韓国");
    require(candidate_secondary_lines(both) == 4);
    const auto both_run = candidate_secondary_run(both);
    require(both_run.readings.size() == 1 &&
            both_run.readings[0].first == std::string("나라 이름 한, 한나라 한\nKorea\n").size());
    require(both_run.translation_line_starts ==
            (std::vector<size_t>{std::string("나라 이름 한, 한나라 한\n").size(),
                                 std::string("나라 이름 한, 한나라 한\nKorea\n/kəˈriːə/\n").size()}));
    // Tab 预选的释义列按每行释义报出来的起点画下划线：第一列有读音时，第二列在读音那一行之后，不是第二行。候选带 훈음 时不接释义列。
    PresentationCandidate columns{};
    columns.text = "你好";
    columns.translation = "hello\xE2\x80\xA8" "こんにちは";
    columns.pronunciation = "/həˈləʊ/\nkonnichiwa";
    require(candidate_secondary_text(columns) == "hello\n/həˈləʊ/\nこんにちは\nkonnichiwa");
    require(candidate_secondary_lines(columns) == 4);
    require(candidate_gloss_column_range(columns, 1) == std::make_optional(std::make_pair(size_t{0}, size_t{5})));
    require(candidate_gloss_column_range(columns, 2) ==
            std::make_optional(std::make_pair(std::string("hello\n/həˈləʊ/\n").size(),
                                              std::string("こんにちは").size())));
    require(!candidate_gloss_column_range(columns, 3) && !candidate_gloss_column_range(columns, 0));
    require(!candidate_gloss_column_range(both, 1));
    // 没有读音时第二列就在第二行。
    columns.pronunciation.clear();
    require(candidate_gloss_column_range(columns, 2) ==
            std::make_optional(std::make_pair(size_t{6}, std::string("こんにちは").size())));
    PresentationCandidate sentence{};
    sentence.text = "我喜欢你";
    sentence.translation = "I like you";
    require(candidate_secondary_text(sentence) == "I like you" && candidate_secondary_lines(sentence) == 1);
    // 悬停提示是候选全文，下一行起是释义；没有释义时只有候选。
    require(candidate_tooltip_text(sentence) == "我喜欢你\nI like you");
    PresentationCandidate bare{};
    bare.text = "你好";
    bare.badge = " ☁️";
    require(candidate_tooltip_text(bare) == "你好 ☁️");
    // 正好 4096 字节时保留读音（释义、换行和读音）；超过上限才只画释义，不让整个候选窗失败。
    PresentationCandidate long_gloss{};
    long_gloss.text = "长";
    long_gloss.translation = std::string(4091, 'a');
    long_gloss.pronunciation = "/ə/";
    require(candidate_secondary_text(long_gloss).size() == 4096);
    long_gloss.translation = std::string(4092, 'a');
    require(candidate_secondary_text(long_gloss) == long_gloss.translation);
    require(candidate_secondary_run(long_gloss).readings.empty() && candidate_secondary_lines(long_gloss) == 1);
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  std::cout << "Korean Hanja presentation checks passed\n";
  return 0;
}

#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Wubi code hint test failed at line " + std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json candidate(size_t index, const std::string &text, const std::string &code,
                         const std::string &annotation = {}) {
  return nlohmann::json{{"id", {{"session", 1}, {"generation", 2}, {"index", index}}},
                        {"text", text},
                        {"highlighted", index == 0},
                        {"code", code},
                        {"annotation", annotation}};
}
nlohmann::json view() {
  return nlohmann::json{{"session", 1},
                        {"generation", 2},
                        {"focused", true},
                        {"scheme", 2},
                        {"local_mode", "none"},
                        {"answered_by_pinyin_fallback", false},
                        {"preedit", "gg"},
                        {"editing_text", "gg"},
                        {"candidates",
                         {candidate(0, "王", "gg"), candidate(1, "五", "gghy"),
                          candidate(2, "一", "ggll", "engine")}}};
}

nlohmann::json corrected_view() {
  auto result = view();
  result["candidates"][1]["corrected"] = true;
  return result;
}
} // namespace
int main() {
  try {
    // The pure rule: only a strict prefix of a Wubi code, and never for fallback or local-mode rows.
    require(wubi_code_hint("gghy", "gg", 2, "none", false) == "hy");
    require(wubi_code_hint("gg", "gg", 2, "none", false).empty());
    require(wubi_code_hint("gghy", "gh", 2, "none", false).empty());
    require(wubi_code_hint("gghy", "", 2, "none", false).empty());
    require(wubi_code_hint("gghy", "gg", 0, "none", false).empty());
    require(wubi_code_hint("gghy", "gg", 2, "quick_phrase", false).empty());
    require(wubi_code_hint("gghy", "gg", 2, "none", true).empty());
    require(wubi_code_hint(std::string(65, 'g'), "gg", 2, "none", false).empty());

    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};
    const auto projected = candidate_presentation_from_view(lease, view(), 0, 0, "");
    require(projected.candidates.size() == 3);
    require(projected.candidates[0].wubi_code_hint.empty());
    require(projected.candidates[1].wubi_code_hint == "hy");
    require(projected.candidates[2].wubi_code_hint == "ll");
    // The projection alone leaves the annotation as the Engine sent it; the preference decides.
    require(projected.candidates[1].annotation.empty());

    const auto shown = with_wubi_code_hints(projected, true);
    require(shown.candidates[0].annotation.empty());
    require(shown.candidates[1].annotation == "(hy)");
    require(shown.candidates[2].annotation == "(ll)");
    const auto hidden = with_wubi_code_hints(projected, false);
    require(hidden.candidates[1].annotation.empty());
    require(hidden.candidates[2].annotation == "engine");

    // A Pinyin scheme carries codes too, and must not advertise them as Wubi keys.
    auto pinyin = view();
    pinyin["scheme"] = 0;
    require(candidate_presentation_from_view(lease, pinyin, 0, 0, "").candidates[1].wubi_code_hint.empty());
    auto fallback = view();
    fallback["answered_by_pinyin_fallback"] = true;
    require(candidate_presentation_from_view(lease, fallback, 0, 0, "").candidates[1].wubi_code_hint.empty());
    auto local = view();
    local["local_mode"] = "unicode";
    require(candidate_presentation_from_view(lease, local, 0, 0, "").candidates[1].wubi_code_hint.empty());

    const auto corrected = candidate_presentation_from_view(lease, corrected_view(), 0, 0, "");
    require(corrected.candidates[1].corrected);
    require(!corrected.candidates[0].corrected);
    require(candidate_primary_text(corrected.candidates[1]) == "五*");
    auto corrected_cloud = corrected;
    corrected_cloud.candidates[1].badge = " ☁️";
    require(candidate_primary_text(corrected_cloud.candidates[1]) == "五* ☁️");
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
  return 0;
}

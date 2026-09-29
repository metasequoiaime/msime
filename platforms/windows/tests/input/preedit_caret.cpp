#include "CandidatePresentation.h"
#include <iostream>
#include <stdexcept>
#include <string>

using namespace msime::windows;
namespace {
[[noreturn]] void require_failed(int line) {
  throw std::runtime_error("Preedit caret test failed at line " +
                           std::to_string(line));
}
#define require(value)                                                         \
  do {                                                                         \
    if (!(value))                                                              \
      require_failed(__LINE__);                                                \
  } while (false)
nlohmann::json view(const std::string &preedit, const std::string &editing,
                    int caret) {
  return nlohmann::json{
      {"session", 1},
      {"generation", 2},
      {"focused", true},
      {"preedit", preedit},
      {"editing_text", editing},
      {"caret_position", caret},
      {"candidates", nlohmann::json::array()}};
}
} // namespace
int main() {
  try {
    const FocusLease lease{{42, {1, 2, 3}}, 1, 1};

    // The ordinary case: the preedit is the editing text, so the byte offset
    // transfers directly.
    {
      const auto out =
          candidate_presentation_from_view(lease, view("nihao", "nihao", 2), 0, 0, "");
      require(out.preedit == "nihao");
      require(out.preedit_caret == 2);
    }
    // The host's prefix shifts the caret by its own length; without that the
    // caret would sit that many characters too far left.
    {
      const auto out = candidate_presentation_from_view(
          lease, view("nihao", "nihao", 2), 0, 0, "U");
      require(out.preedit == "Unihao");
      require(out.preedit_caret == 3);
    }
    // Caret at either end is valid and must be kept.
    {
      require(candidate_presentation_from_view(lease, view("ni", "ni", 0), 0, 0, "")
                  .preedit_caret == 0);
      require(candidate_presentation_from_view(lease, view("ni", "ni", 2), 0, 0, "")
                  .preedit_caret == 2);
    }
    // The Engine reports the caret against editing_text. When the drawn preedit
    // is a different string the offset means nothing there, and a caret at the
    // wrong character is worse than none, so it is dropped.
    {
      const auto out = candidate_presentation_from_view(
          lease, view("ni'hao", "nihao", 2), 0, 0, "");
      require(out.preedit == "ni'hao");
      require(out.preedit_caret == std::string::npos);
    }
    // An offset past the end is refused rather than clamped: it means the two
    // sides disagree, and guessing would put the caret somewhere arbitrary.
    {
      require(candidate_presentation_from_view(lease, view("ni", "ni", 9), 0, 0, "")
                  .preedit_caret == std::string::npos);
    }
    // A view with no caret at all still works; it simply lands at the end.
    {
      auto without = view("nihao", "nihao", 0);
      without.erase("caret_position");
      require(candidate_presentation_from_view(lease, without, 0, 0, "")
                  .preedit_caret == 5);
    }
    // An unfocused view carries no preedit and therefore no caret.
    {
      auto blurred = view("nihao", "nihao", 2);
      blurred["focused"] = false;
      require(candidate_presentation_from_view(lease, blurred, 0, 0, "")
                  .preedit_caret == std::string::npos);
    }
    // The preedit row carries the pager: the view's page position is kept when it names a page of the count, and dropped otherwise rather than drawing "4 / 3".
    {
      auto paged = view("nihao", "nihao", 2);
      paged["page"] = 1;
      paged["page_count"] = 3;
      const auto out = candidate_presentation_from_view(lease, paged, 0, 0, "");
      require(out.page == 1 && out.page_count == 3);
      paged["page"] = 3;
      const auto past = candidate_presentation_from_view(lease, paged, 0, 0, "");
      require(past.page == 0 && past.page_count == 0);
      const auto unpaged =
          candidate_presentation_from_view(lease, view("ni", "ni", 2), 0, 0, "");
      require(unpaged.page == 0 && unpaged.page_count == 0);
    }

    std::cout << "Preedit caret: placed only where the offset really means it\n";
  } catch (const std::exception &failure) {
    std::cerr << failure.what() << '\n';
    return 1;
  } catch (...) {
    std::cerr << "Preedit caret test failed with an unknown error\n";
    return 1;
  }
}

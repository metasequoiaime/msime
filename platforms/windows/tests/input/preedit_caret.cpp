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
    // Quanpin preedit is segmented by the Engine (`ni'hao`) while editing_text and caret_position stay on the raw keys (`nihao`). The caret is carried through the apostrophes the Engine inserted, the same mapping as macOS MSIMEPreeditCaretPosition: a raw caret at a syllable boundary lands before the inserted apostrophe, and both ends stay at the ends.
    {
      const size_t expected[] = {0, 1, 2, 4, 5, 6};
      for (int raw = 0; raw <= 5; ++raw) {
        const auto out = candidate_presentation_from_view(
            lease, view("ni'hao", "nihao", raw), 0, 0, "");
        require(out.preedit == "ni'hao");
        require(out.preedit_caret == expected[raw]);
      }
      require(candidate_presentation_from_view(
                  lease, view("pin'yin", "pinyin", 3), 0, 0, "")
                  .preedit_caret == 3);
      require(candidate_presentation_from_view(
                  lease, view("pin'yin", "pinyin", 4), 0, 0, "")
                  .preedit_caret == 5);
    }
    // The host's prefix shifts a mapped caret just as it shifts an unmapped one.
    {
      const auto out = candidate_presentation_from_view(
          lease, view("ni'hao", "nihao", 3), 0, 0, "U");
      require(out.preedit == "Uni'hao");
      require(out.preedit_caret == 5);
    }
    // An apostrophe the user typed is in editing_text too. A caret right after it stays after it and a caret right before it stays before it, while the apostrophes the Engine added elsewhere are still skipped.
    {
      const auto caret_of = [&](int raw) {
        return candidate_presentation_from_view(
                   lease, view("pin'yin'shu'ru'fa", "pinyin'shurufa", raw), 0, 0, "")
            .preedit_caret;
      };
      require(caret_of(6) == 7);
      require(caret_of(7) == 8);
      require(caret_of(10) == 11);
      require(caret_of(11) == 13);
      require(caret_of(14) == 17);
      // The Engine's display keeps a trailing typed apostrophe.
      require(candidate_presentation_from_view(
                  lease, view("ni'hao'", "nihao'", 5), 0, 0, "")
                  .preedit_caret == 6);
      require(candidate_presentation_from_view(
                  lease, view("ni'hao'", "nihao'", 6), 0, 0, "")
                  .preedit_caret == 7);
    }
    // A view with no caret lands at the end of the segmented preedit, not at editing_text's length inside it.
    {
      auto without = view("ni'hao", "nihao", 0);
      without.erase("caret_position");
      require(candidate_presentation_from_view(lease, without, 0, 0, "")
                  .preedit_caret == 6);
    }
    // When the preedit is not the raw letters plus separators the offset has no map into it, and a caret at the wrong character is worse than none, so it is still dropped: expanded shuangpin, Korean Hangul, stroke glyphs, Japanese kana.
    {
      require(candidate_presentation_from_view(lease, view("ni'hao", "nihc", 2), 0, 0, "")
                  .preedit_caret == std::string::npos);
      require(candidate_presentation_from_view(lease, view("\xED\x95\x9C", "gks", 1), 0, 0, "")
                  .preedit_caret == std::string::npos);
      require(candidate_presentation_from_view(
                  lease, view("\xE4\xB8\x80\xE4\xB8\xA8", "hs", 1), 0, 0, "")
                  .preedit_caret == std::string::npos);
      require(candidate_presentation_from_view(
                  lease, view("\xE3\x81\xAB\xE3\x81\xBB\xE3\x82\x93", "nihon", 2), 0, 0, "")
                  .preedit_caret == std::string::npos);
      // A segmented preedit whose caret is past the raw keys is refused like an unsegmented one.
      require(candidate_presentation_from_view(lease, view("ni'hao", "nihao", 6), 0, 0, "")
                  .preedit_caret == std::string::npos);
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

#include "CandidatePresentation.h"
#include <iostream>
#include <limits>
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
// 引擎给出 View.preedit_caret（光标在 preedit 里的位置，Unicode 标量）的视图，和 host-api 实际交给 Server 的一样。
nlohmann::json engine_view(const std::string &preedit, const std::string &editing, int caret,
                           int preedit_caret) {
  auto value = view(preedit, editing, caret);
  value["preedit_caret"] = preedit_caret;
  return value;
}
// 按键送达那条路径（ReplyComposer 的回复确认之后）的投影：每次打字、移光标，候选窗画的都是它。
CandidatePresentation delivered(const FocusLease &lease, const nlohmann::json &value,
                                const std::string &next_prefix = {}) {
  PendingReply reply{};
  reply.source = {42, 1, 7, true, nlohmann::json{{"view", value}}};
  reply.next_prefix = next_prefix;
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.request_id = 7;
  packet.event_type = FanyImePipeEventType::KeyEvent;
  return candidate_presentation(lease, reply, packet);
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

    // 按键送达的投影和异步刷新的投影放同一个插入点。以前只有刷新那条路径带光标，每次按键送达的投影都把它丢掉，候选窗拼音行打字、左右移光标时从来看不到插入点，只有释义、云候选这类异步结果回来重画时才偶尔出现。
    {
      const auto out = delivered(lease, view("nihao", "nihao", 2));
      require(out.visible && out.preedit == "nihao");
      require(out.preedit_caret == 2);
      require(delivered(lease, view("nihao", "nihao", 5)).preedit_caret == 5);
      // 已选词段的前缀画在拼音前面，光标跟着后移。
      const auto prefixed = delivered(lease, view("hao", "hao", 1), "你");
      require(prefixed.preedit == "你hao");
      require(prefixed.preedit_caret == std::string("你").size() + 1);
    }
    // 引擎给了 View.preedit_caret 时按它放，两条投影一样。双拼分隔（ni'mf，按键 nimf）：光标在末尾、按左键后在 mf 中间、在音节边界上落在隔音符前面、在开头。
    {
      const int raw[] = {4, 3, 2, 0};
      const int display[] = {5, 4, 2, 0};
      for (int index = 0; index < 4; ++index) {
        const auto value = engine_view("ni'mf", "nimf", raw[index], display[index]);
        require(delivered(lease, value).preedit_caret == static_cast<size_t>(display[index]));
        require(candidate_presentation_from_view(lease, value, 0, 0, "").preedit_caret ==
                static_cast<size_t>(display[index]));
      }
    }
    // 双拼「拼音分词」展开的全拼（按键 uiui，显示 shi'shi）字母数和按键对不上，宿主自己数字母只能放弃；引擎按音节换好的位置可以直接画：光标在第一个双拼单位后面，落在第一个音节后、隔音符前。
    {
      const auto value = engine_view("shi'shi", "uiui", 2, 3);
      require(delivered(lease, value).preedit_caret == 3);
      require(candidate_presentation_from_view(lease, value, 0, 0, "").preedit_caret == 3);
      require(delivered(lease, engine_view("shi'shi", "uiui", 4, 7)).preedit_caret == 7);
    }
    // 引擎的位置按 Unicode 标量计，宿主按 UTF-8 字节画：韩文音节组字时光标在末尾，是三个字节之后。
    {
      const auto value = engine_view("\xED\x95\x9C", "gks", 3, 1);
      require(delivered(lease, value).preedit_caret == 3);
      require(candidate_presentation_from_view(lease, value, 0, 0, "").preedit_caret == 3);
    }
    // 引擎的位置越过 preedit 时两边对不上，不猜，不画。
    {
      require(delivered(lease, engine_view("ni", "ni", 2, 3)).preedit_caret == std::string::npos);
      require(candidate_presentation_from_view(lease, engine_view("ni", "ni", 2, 3), 0, 0, "")
                  .preedit_caret == std::string::npos);
    }

    // 只有缺字段才兼容旧协议；明确给出的非法位置不能猜成原始按键的光标。
    {
      const nlohmann::json invalid_positions[] = {
          nullptr, false, true, 1.5, 1.0, "1", -1,
          nlohmann::json::array(), nlohmann::json::object(),
          uint64_t{1} << 32, uint64_t{1} << 63,
          std::numeric_limits<uint64_t>::max()};
      for (const auto &position : invalid_positions) {
        auto value = view("ni", "ni", 1);
        value["preedit_caret"] = position;
        require(delivered(lease, value).preedit_caret == std::string::npos);
        require(candidate_presentation_from_view(lease, value, 0, 0, "").preedit_caret ==
                std::string::npos);
      }
      // 合法无符号位置继续按 Unicode 标量转换，不能与非法大整数一起拒绝。
      for (uint64_t position = 0; position <= 2; ++position) {
        auto value = view("ni", "ni", 1);
        value["preedit_caret"] = position;
        require(delivered(lease, value).preedit_caret == position);
        require(candidate_presentation_from_view(lease, value, 0, 0, "").preedit_caret ==
                position);
      }
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

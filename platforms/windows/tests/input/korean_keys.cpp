#include "../../src/ipc/ReplyComposer.h"
#include "../core/TestHostOptions.h"
#include <cassert>
#include <chrono>
using namespace msime::windows;

namespace {
FanyImeNamedpipeData key(uint64_t request, uint32_t code, char16_t text, uint32_t modifiers = 0) {
  FanyImeNamedpipeData packet{};
  packet.client_id = 42;
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.request_id = request;
  packet.keycode = code;
  packet.wch = text;
  packet.modifiers_down = modifiers;
  return packet;
}

// The Server side of the Korean scheme, run against a real Engine session. The TIP inserts every syllable from its own host session; what the Server has to get right is staying in step with it, never failing a reply because a letter carried a commit, and counting what was written.
struct Fixture {
  ServerSession session;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  // Minus/equals and comma/period paging on, so the test shows Korean ignores them.
  NavigationBindings paging{true, true, true, true, true, true, false};

  explicit Fixture(const std::string &options) : session(42, options) { session.activate(1); }

  std::optional<PendingReply> press(uint32_t code, char16_t text, uint32_t modifiers = 0,
                                    TsfPreeditStyle style = TsfPreeditStyle::Local,
                                    WordCharacterBinding binding = WordCharacterBinding::Disabled) {
    const auto packet = key(++request, code, text, modifiers);
    auto reply = composer.configured_key(session, packet, 1, style, paging, std::nullopt, binding);
    if (reply)
      composer.confirm_delivery(42, 1, packet.request_id);
    return reply;
  }
  void type(const char *letters) {
    for (const char *letter = letters; *letter; ++letter) {
      const bool upper = *letter >= 'A' && *letter <= 'Z';
      const auto code = static_cast<uint32_t>(upper ? *letter : *letter - 'a' + 'A');
      assert(press(code, static_cast<char16_t>(*letter), upper ? 1u : 0u));
    }
  }
  std::string editing() const { return session.view().at("editing_text").get<std::string>(); }
  std::string reading() const { return session.view().at("reading").get<std::string>(); }
  nlohmann::json candidates() const { return session.view().at("candidates"); }
  std::string candidate(size_t slot) const { return candidates().at(slot).at("text").get<std::string>(); }
  std::string highlighted() const {
    for (const auto &item : candidates())
      if (item.at("highlighted").get<bool>())
        return item.at("text").get<std::string>();
    return {};
  }
};
} // namespace

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-korean-keys-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() {
      std::error_code ec;
      std::filesystem::remove_all(root, ec);
    }
  } cleanup{root};
  auto options = test_host_options(root);
  options["preferences"]["scheme"] = "korean";
  options["preferences"]["traditional_chinese_output"] = true;
  const auto serialized = options.dump();

  // A letter that starts a new syllable commits the finished one in the same reply. With the raw style the TIP draws the composition itself, so no frame goes back, but the commit is still counted.
  {
    Fixture korean(serialized);
    assert(korean.session.view().value("scheme", 0u) == 4u);
    korean.type("dks");
    assert(korean.editing() == "dks" && korean.reading() == "안");
    const auto next = korean.press('S', u's');
    assert(next && next->committed_text && *next->committed_text == "안");
    assert(!next->encoded && !next->traditional_output);
    assert(korean.editing() == "s" && korean.reading() == "ㄴ");
    // Traditional output is a Chinese projection; the toggle survives the Korean scheme untouched.
    assert(korean.session.traditional_output());
  }

  // The pinyin style reads the composition from the Server, so the reply is the next syllable alone. The syllable already has a final ㄴ, and ㄴ plus ㄱ is not a compound final, so the ㄱ starts a new syllable instead of joining this one.
  {
    Fixture korean(serialized);
    korean.type("rks");
    const auto next = korean.press('R', u'r', 0, TsfPreeditStyle::Pinyin);
    assert(next && next->committed_text && *next->committed_text == "간");
    assert(next->encoded && *next->encoded &&
           next->encoded->packet.msg_type == FanyImeReplyType::Preedit);
    assert(korean.reading() == "ㄱ");
  }

  // Shift gives the tense consonant.
  {
    Fixture korean(serialized);
    korean.type("R");
    assert(korean.editing() == "R" && korean.reading() == "ㄲ");
  }

  // Backspace takes one jamo off the open syllable.
  {
    Fixture korean(serialized);
    korean.type("ekfr");
    assert(korean.reading() == "닭");
    assert(korean.press(0x08, u'\b'));
    assert(korean.reading() == "달");
  }

  // Space and digits end the syllable; their own character is the TIP's to insert.
  for (const auto &[code, text] : {std::pair<uint32_t, char16_t>{0x20, u' '}, {'1', u'1'}, {0x61, u'1'}}) {
    Fixture korean(serialized);
    korean.type("rk");
    const auto ended = korean.press(code, text);
    assert(ended && ended->committed_text && *ended->committed_text == "가");
    assert(korean.editing().empty());
  }

  // Punctuation follows the syllable half-width, and '-', '=' and ',' do not page even though paging is bound to them.
  for (const auto &[code, text, expected] :
       {std::tuple<uint32_t, char16_t, const char *>{0xBE, u'.', "가."},
        {0xBC, u',', "가,"},
        {0xBD, u'-', "가-"},
        {0xBB, u'=', "가="},
        {0xDB, u'[', "가["},
        {0xBF, u'/', "가/"}}) {
    Fixture korean(serialized);
    korean.type("rk");
    const auto ended = korean.press(code, text);
    assert(ended && ended->committed_text && *ended->committed_text == expected);
    assert(korean.editing().empty());
  }

  // Word-to-character has no candidate to take a character from, so its keys stay punctuation.
  {
    Fixture korean(serialized);
    korean.type("rk");
    const auto ended = korean.press(0xBD, u'-', 0, TsfPreeditStyle::Local, WordCharacterBinding::MinusEqual);
    assert(ended && ended->committed_text && *ended->committed_text == "가-");
  }

  // Caret and editing keys reach the Server only when the TIP queued them behind earlier keys; they end the syllable and nothing is sent back.
  for (const uint32_t code : {0x0Du, 0x09u, 0x25u, 0x27u, 0x26u, 0x28u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu}) {
    Fixture korean(serialized);
    korean.type("rk");
    const auto ended = korean.press(code, 0);
    assert(ended && ended->committed_text && *ended->committed_text == "가");
    assert(!ended->encoded);
    assert(korean.editing().empty());
  }

  // Escape discards the syllable.
  {
    Fixture korean(serialized);
    korean.type("rk");
    const auto cancelled = korean.press(0x1B, 0);
    assert(cancelled && !cancelled->committed_text);
    assert(korean.editing().empty());
  }

  // The Hanja key lists the composing syllable's Hanja and sends nothing back: the TIP converts in its own host session, and the candidate window draws the delivered view.
  {
    Fixture korean(serialized);
    korean.type("gks");
    const auto opened = korean.press(0x19, 0);
    assert(opened && !opened->encoded && !opened->committed_text);
    assert(korean.editing() == "gks" && korean.reading() == "한");
    assert(korean.candidates().size() > 1 && korean.candidate(0) == "韓");
    // Again closes it and keeps the syllable.
    const auto closed = korean.press(0x19, 0);
    assert(closed && !closed->encoded && !closed->committed_text);
    assert(korean.candidates().empty() && korean.editing() == "gks");
  }

  // A digit chooses from the visible page, the row's or the numpad's, and only the chosen Hanja is committed.
  for (const uint32_t code : {uint32_t{'2'}, 0x62u}) {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto second = korean.candidate(1);
    const auto chosen = korean.press(code, u'2');
    assert(chosen && !chosen->encoded && chosen->committed_text && *chosen->committed_text == second);
    assert(korean.editing().empty() && korean.candidates().empty());
  }

  // Space and Enter choose the highlighted Hanja, which the arrows move.
  for (const auto &[code, text] : {std::pair<uint32_t, char16_t>{0x20, u' '}, {0x0D, u'\r'}}) {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto second = korean.candidate(1);
    assert(korean.press(0x28, 0) && korean.highlighted() == second);
    assert(korean.press(0x26, 0) && korean.highlighted() == korean.candidate(0));
    assert(korean.press(0x27, 0) && korean.highlighted() == second);
    const auto chosen = korean.press(code, text);
    assert(chosen && chosen->committed_text && *chosen->committed_text == second);
    assert(korean.editing().empty());
  }

  // Page Down turns the page and Page Up turns it back, whatever the paging bindings say.
  {
    Fixture korean(serialized);
    korean.paging = NavigationBindings{};
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto first = korean.candidate(0);
    assert(korean.press(0x22, 0) && korean.candidate(0) != first && korean.editing() == "gks");
    assert(korean.press(0x21, 0) && korean.candidate(0) == first);
  }

  // Escape and Backspace only close the list; the syllable keeps composing, and a mark then commits the Hangul.
  for (const auto &[code, text] : {std::pair<uint32_t, char16_t>{0x1B, 0}, {0x08, u'\b'}}) {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto closed = korean.press(code, text);
    assert(closed && !closed->committed_text);
    assert(korean.candidates().empty() && korean.editing() == "gks" && korean.reading() == "한");
    const auto ended = korean.press(0xBE, u'.');
    assert(ended && ended->committed_text && *ended->committed_text == "한.");
  }

  // With the list open a mark stays punctuation and commits the Hangul, never a Hanja, even with '-' bound to paging and to word-to-character.
  for (const auto binding : {WordCharacterBinding::Disabled, WordCharacterBinding::MinusEqual}) {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto ended = korean.press(0xBD, u'-', 0, TsfPreeditStyle::Local, binding);
    assert(ended && ended->committed_text && *ended->committed_text == "한-");
    assert(korean.editing().empty());
  }

  // A letter closes the list and composes as usual: ㄴ plus ㄱ is not a compound final, so 한 is committed and ㄱ starts the next syllable.
  {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto next = korean.press('R', u'r');
    assert(next && next->committed_text && *next->committed_text == "한");
    assert(korean.candidates().empty() && korean.reading() == "ㄱ");
  }

  // A digit past the visible page chooses nothing and leaves the list open.
  {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    const auto page = korean.candidates().size();
    if (page < 9) {
      const auto past = korean.press('9', u'9');
      assert(past && !past->committed_text);
      assert(korean.candidates().size() == page && korean.editing() == "gks");
    }
  }

  // A lone jamo has no Hanja, and the Hanja key with nothing composing is spent; neither fails the reply.
  {
    Fixture korean(serialized);
    korean.type("r");
    const auto jamo = korean.press(0x19, 0);
    assert(jamo && !jamo->committed_text && korean.candidates().empty() && korean.reading() == "ㄱ");
    assert(korean.press(0x1B, 0));
    assert(korean.editing().empty());
    const auto idle = korean.press(0x19, 0);
    assert(idle && !idle->encoded && !idle->committed_text);
  }

  // The routed clear a terminated composition sends discards the syllable even with its list open, where one MSIME_CANCEL only closes the list.
  {
    Fixture korean(serialized);
    korean.type("gks");
    assert(korean.press(0x19, 0));
    korean.session.cancel_composition(1);
    assert(korean.editing().empty() && korean.candidates().empty());
  }
}

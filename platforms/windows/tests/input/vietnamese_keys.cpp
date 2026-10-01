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

// The Server side of the Vietnamese scheme, run against a real Engine session. Like Korean, the TIP composes each word in its own host session and writes it itself; the Server has to stay in step with it, never fail a reply because a key carried a commit, and count what was written.
struct Fixture {
  ServerSession session;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  // Minus/equals and comma/period paging on, so the test shows Vietnamese ignores them.
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
  // Letters and the digit row, as a US layout types them.
  void type(const char *keys) {
    for (const char *text = keys; *text; ++text) {
      const bool upper = *text >= 'A' && *text <= 'Z';
      const bool lower = *text >= 'a' && *text <= 'z';
      const auto code = static_cast<uint32_t>(lower ? *text - 'a' + 'A' : *text);
      assert(press(code, static_cast<char16_t>(*text), upper ? 1u : 0u));
    }
  }
  std::string editing() const { return session.view().at("editing_text").get<std::string>(); }
};
} // namespace

int main() {
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-vietnamese-keys-" +
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
  options["preferences"]["scheme"] = "vietnamese";
  options["preferences"]["vietnamese"]["input_method"] = "telex";
  options["preferences"]["traditional_chinese_output"] = true;
  const auto serialized = options.dump();
  auto vni_options = options;
  vni_options["preferences"]["vietnamese"]["input_method"] = "vni";
  const auto vni = vni_options.dump();

  // Telex letters build the word in place, case kept, with no candidates. Traditional output is a Chinese projection: Vietnamese is never converted, and the switch survives the scheme untouched.
  {
    Fixture vietnamese(serialized);
    assert(vietnamese.session.view().value("scheme", 0u) == 7u);
    vietnamese.type("Vieejt");
    assert(vietnamese.editing() == "Việt");
    assert(vietnamese.session.view().at("candidates").empty());
    assert(vietnamese.session.traditional_output());
  }

  // Space and a digit end the word; their own character is the TIP's to insert.
  for (const auto &[code, text] : {std::pair<uint32_t, char16_t>{0x20, u' '}, {'1', u'1'}, {0x61, u'1'}}) {
    Fixture vietnamese(serialized);
    vietnamese.type("hoaf");
    const auto ended = vietnamese.press(code, text);
    assert(ended && ended->committed_text && *ended->committed_text == "hoà");
    assert(!ended->traditional_output && vietnamese.editing().empty());
  }

  // Punctuation follows the word half-width, and '-', '=' and ',' do not page even though paging is bound to them.
  for (const auto &[code, text, expected] :
       {std::tuple<uint32_t, char16_t, const char *>{0xBE, u'.', "hoà."},
        {0xBC, u',', "hoà,"},
        {0xBD, u'-', "hoà-"},
        {0xBB, u'=', "hoà="},
        {0xBF, u'/', "hoà/"}}) {
    Fixture vietnamese(serialized);
    vietnamese.type("hoaf");
    const auto ended = vietnamese.press(code, text);
    assert(ended && ended->committed_text && *ended->committed_text == expected);
    assert(vietnamese.editing().empty());
  }

  // Word-to-character has no candidate to take a character from, so its keys stay punctuation.
  {
    Fixture vietnamese(serialized);
    vietnamese.type("hoaf");
    const auto ended =
        vietnamese.press(0xBD, u'-', 0, TsfPreeditStyle::Local, WordCharacterBinding::MinusEqual);
    assert(ended && ended->committed_text && *ended->committed_text == "hoà-");
  }

  // Caret and editing keys reach the Server only when the TIP queued them behind earlier keys; they end the word and nothing is sent back.
  for (const uint32_t code : {0x0Du, 0x09u, 0x25u, 0x27u, 0x26u, 0x28u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu}) {
    Fixture vietnamese(serialized);
    vietnamese.type("hoaf");
    const auto ended = vietnamese.press(code, 0);
    assert(ended && ended->committed_text && *ended->committed_text == "hoà");
    assert(!ended->encoded && vietnamese.editing().empty());
  }

  // Backspace takes one keystroke off the word.
  {
    Fixture vietnamese(serialized);
    vietnamese.type("hoaf");
    assert(vietnamese.press(0x08, u'\b'));
    assert(vietnamese.editing() == "hoa");
  }

  // The first Escape shows the keys typed for the word again and the word keeps composing, as the TIP keeps it in its own host session; nothing is sent back for it. A second Escape discards the word. The routed clear a terminated composition sends discards it in one go.
  for (const auto &[method, keys] : {std::pair<const std::string *, const char *>{&serialized, "hoaf"}, {&vni, "tieng5"}}) {
    Fixture vietnamese(*method);
    vietnamese.type(keys);
    assert(vietnamese.editing() != keys);
    const auto restored = vietnamese.press(0x1B, 0);
    assert(restored && !restored->committed_text && !restored->encoded);
    assert(vietnamese.editing() == keys);
    assert(vietnamese.session.view().at("preedit").get<std::string>() == keys);
    const auto cancelled = vietnamese.press(0x1B, 0);
    assert(cancelled && !cancelled->committed_text);
    assert(vietnamese.editing().empty());
    vietnamese.type(keys);
    vietnamese.session.cancel_composition(1);
    assert(vietnamese.editing().empty());
  }

  // VNI spells tones with the digit row while a word composes; an idle digit is text the TIP leaves to the application, never a candidate pick.
  {
    Fixture vietnamese(vni);
    vietnamese.type("viet65");
    assert(vietnamese.editing() == "việt");
    const auto ended = vietnamese.press(0x20, u' ');
    assert(ended && ended->committed_text && *ended->committed_text == "việt");
    const auto idle = vietnamese.press('6', u'6');
    assert(!idle || !idle->committed_text);
    assert(vietnamese.editing().empty());
  }

  // The pinyin style reads the composition from the Server, so a letter is answered with the word so far.
  {
    Fixture vietnamese(serialized);
    const auto reply = vietnamese.press('A', u'a', 0, TsfPreeditStyle::Pinyin);
    assert(reply && reply->encoded && *reply->encoded &&
           reply->encoded->packet.msg_type == FanyImeReplyType::Preedit);
  }
  return 0;
}

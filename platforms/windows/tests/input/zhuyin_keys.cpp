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

// The US-layout key that types an ASCII character, and whether it takes Shift.
std::pair<uint32_t, bool> us_key(char text) {
  if (text >= 'a' && text <= 'z')
    return {static_cast<uint32_t>(text - 'a' + 'A'), false};
  if (text >= '0' && text <= '9')
    return {static_cast<uint32_t>(text), false};
  switch (text) {
  case ' ':
    return {0x20, false};
  case ',':
    return {0xBC, false};
  case '<':
    return {0xBC, true};
  case '.':
    return {0xBE, false};
  case '/':
    return {0xBF, false};
  case '?':
    return {0xBF, true};
  case ';':
    return {0xBA, false};
  case '-':
    return {0xBD, false};
  case '[':
    return {0xDB, false};
  case '!':
    return {'1', true};
  }
  assert(false && "no US key for this character");
  return {0, false};
}

// The Server side of the Zhuyin (Dachen) scheme, run against a real Engine session and the dictionary of the engine's golden Zhuyin scenarios. The TIP composes in its own host session and writes every commit itself; the Server has to stay in step with it key for key, never fail a reply because a key carried a commit, and count what was written.
struct Fixture {
  ServerSession session;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  // Minus/equals, comma/period and bracket paging on, so the test shows Zhuyin spells with those keys or keeps them as punctuation instead.
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
  std::optional<PendingReply> press_text(char text, TsfPreeditStyle style = TsfPreeditStyle::Local) {
    const auto [code, shift] = us_key(text);
    return press(code, static_cast<char16_t>(text), shift ? 1u : 0u, style);
  }
  void type(const char *keys) {
    for (const char *text = keys; *text; ++text)
      assert(press_text(*text));
  }
  std::string editing() const { return session.view().at("editing_text").get<std::string>(); }
  std::string preedit() const { return session.view().at("preedit").get<std::string>(); }
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

// argv[1] is tests/input/fixtures/zhuyin.db: the dictionary of crates/engine/tests/golden/scenarios/zh_bpmf_space_opens_list.json, written by Python's sqlite3 from that scenario's SQL with a 512-byte page size. The Windows build links no SQLite of its own, so the file is checked in rather than built here.
int main(int argc, char **argv) {
  assert(argc == 2);
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-zhuyin-keys-" +
                     std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
  std::filesystem::create_directory(root);
  struct Cleanup {
    std::filesystem::path root;
    ~Cleanup() {
      std::error_code ec;
      std::filesystem::remove_all(root, ec);
    }
  } cleanup{root};
  // A copy beside the resources, where the packaged dictionary goes, so the checked-in file is never opened for writing.
  const auto dictionaries = root / "language-dictionaries";
  std::filesystem::create_directory(dictionaries);
  std::filesystem::copy_file(std::filesystem::u8path(argv[1]), dictionaries / "zhuyin.db");
  auto options = test_host_options(root);
  options["language_dictionaries"] = dictionaries.u8string();
  options["preferences"]["scheme"] = "zhuyin";
  options["preferences"]["traditional_chinese_output"] = true;
  const auto serialized = options.dump();

  // Without its dictionary the scheme cannot run and the Engine answers with quanpin, the scheme the tray would then show.
  {
    auto missing = test_host_options(root / "missing");
    missing["preferences"]["scheme"] = "zhuyin";
    Fixture fallback(missing.dump());
    assert(fallback.session.view().value("scheme", 0u) == 0u);
  }

  // Letters, digits and the Dachen punctuation keys spell bopomofo; the conversion shows in the preedit and no list opens.
  {
    Fixture zhuyin(serialized);
    assert(zhuyin.session.view().value("scheme", 0u) == 6u);
    zhuyin.type("su3");
    assert(zhuyin.editing() == "su3" && zhuyin.preedit() == "你");
    assert(zhuyin.candidates().empty());
    zhuyin.type("cl3");
    assert(zhuyin.preedit() == "你好");
    // ',' is ㄝ, not paging or punctuation, even with paging bound to it.
    assert(zhuyin.press_text(','));
    assert(zhuyin.editing() == "su3cl3," && zhuyin.candidates().empty());
  }

  // Idle, the keys Dachen starts a syllable with are input too, digits and punctuation keys included.
  for (const char start : {'1', '5', ',', '/', ';', '-'}) {
    Fixture zhuyin(serialized);
    assert(zhuyin.press_text(start));
    assert(zhuyin.editing() == std::string(1, start));
  }

  // Space on a finished syllable opens the list; it is the first tone otherwise, so it reaches the Engine as that key and never picks a row. A second Space fixes the highlighted reading without committing.
  {
    Fixture zhuyin(serialized);
    zhuyin.type("su3");
    const auto opened = zhuyin.press_text(' ');
    assert(opened && !opened->committed_text && !opened->encoded);
    assert(zhuyin.candidates().size() == 2 && zhuyin.candidate(0) == "你");
    const auto fixed = zhuyin.press_text(' ');
    assert(fixed && !fixed->committed_text);
    assert(zhuyin.candidates().empty() && zhuyin.editing() == "su3" && zhuyin.preedit() == "你");
  }

  // Down opens the list on a composing conversion; with it open Down moves the highlight, and a digit fixes that reading and keeps composing. Commits are Traditional as stored, whatever the Traditional output switch says, and never converted again.
  {
    Fixture zhuyin(serialized);
    zhuyin.type("su3cl3");
    const auto opened = zhuyin.press(0x28, 0);
    assert(opened && !opened->committed_text && !opened->encoded);
    assert(zhuyin.candidates().size() == 3);
    const auto first = zhuyin.highlighted();
    assert(zhuyin.press(0x28, 0) && zhuyin.highlighted() != first);
    const auto third = zhuyin.candidate(2);
    const auto chosen = zhuyin.press_text('3');
    assert(chosen && !chosen->committed_text && !chosen->encoded);
    assert(zhuyin.candidates().empty() && zhuyin.editing() == "su3cl3" && zhuyin.preedit() == "你" + third);
    const auto committed = zhuyin.press(0x0D, u'\r');
    assert(committed && committed->committed_text && *committed->committed_text == "你" + third);
    assert(!committed->encoded && !committed->traditional_output);
    assert(zhuyin.editing().empty());
  }

  // Down with nothing composing is the application's: it is not routed as a list key.
  {
    Fixture zhuyin(serialized);
    const auto idle = zhuyin.press(0x28, 0);
    assert(!idle || !idle->committed_text);
    assert(zhuyin.editing().empty() && zhuyin.candidates().empty());
  }

  // Enter commits the converted text and drops a pending initial, as libchewing does.
  {
    Fixture zhuyin(serialized);
    zhuyin.type("su3c");
    const auto committed = zhuyin.press(0x0D, u'\r');
    assert(committed && committed->committed_text && *committed->committed_text == "你");
    assert(zhuyin.editing().empty());
  }

  // The other caret keys end the composition the same way: the TIP committed it and let the key go on.
  for (const uint32_t code : {0x09u, 0x25u, 0x27u, 0x26u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu}) {
    Fixture zhuyin(serialized);
    zhuyin.type("su3");
    const auto ended = zhuyin.press(code, 0);
    assert(ended && ended->committed_text && *ended->committed_text == "你");
    assert(!ended->encoded && zhuyin.editing().empty());
  }

  // Escape and Backspace close an open list and keep the composition; Escape then discards it.
  for (const auto &[code, text] : {std::pair<uint32_t, char16_t>{0x1B, 0}, {0x08, u'\b'}}) {
    Fixture zhuyin(serialized);
    zhuyin.type("su3");
    assert(zhuyin.press(0x28, 0) && !zhuyin.candidates().empty());
    const auto closed = zhuyin.press(code, text);
    assert(closed && !closed->committed_text);
    assert(zhuyin.candidates().empty() && zhuyin.editing() == "su3");
    const auto cancelled = zhuyin.press(0x1B, 0);
    assert(cancelled && !cancelled->committed_text);
    assert(zhuyin.editing().empty());
  }

  // The routed clear a terminated composition sends discards it even with its list open.
  {
    Fixture zhuyin(serialized);
    zhuyin.type("su3");
    assert(zhuyin.press(0x28, 0) && !zhuyin.candidates().empty());
    zhuyin.session.cancel_composition(1);
    assert(zhuyin.editing().empty() && zhuyin.candidates().empty());
  }

  // Shifted punctuation commits the conversion and its full-width mark, idle or composing; a mark outside the Dachen keys commits through the Chinese table.
  for (const auto &[keys, mark, expected] :
       {std::tuple<const char *, char, const char *>{"su3", '<', "你，"},
        {"", '?', "？"},
        {"cl3", '[', "好「"},
        {"su3", '!', "你！"}}) {
    Fixture zhuyin(serialized);
    zhuyin.type(keys);
    const auto ended = zhuyin.press_text(mark);
    assert(ended && ended->committed_text && *ended->committed_text == expected);
    assert(!ended->traditional_output && zhuyin.editing().empty());
  }

  // Word-to-character has no candidate to take a character from: '-' is ㄦ while composing, whatever the binding.
  {
    Fixture zhuyin(serialized);
    zhuyin.type("su3");
    const auto spelled = zhuyin.press(0xBD, u'-', 0, TsfPreeditStyle::Local, WordCharacterBinding::MinusEqual);
    assert(spelled && !spelled->committed_text);
    assert(zhuyin.editing() == "su3-");
  }

  // The pinyin style reads the composition from the Server, so a spelled key is answered with the conversion.
  {
    Fixture zhuyin(serialized);
    const auto reply = zhuyin.press_text('s', TsfPreeditStyle::Pinyin);
    assert(reply && reply->encoded && *reply->encoded &&
           reply->encoded->packet.msg_type == FanyImeReplyType::Preedit);
  }
  return 0;
}

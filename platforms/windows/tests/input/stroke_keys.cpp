#include "../../src/candidate/CandidatePresentation.h"
#include "../../src/input/TypingStatistics.h"
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

// The US-layout key that types an ASCII character.
uint32_t us_key(char text) {
  if (text >= 'a' && text <= 'z')
    return static_cast<uint32_t>(text - 'a' + 'A');
  if (text >= '0' && text <= '9')
    return static_cast<uint32_t>(text);
  switch (text) {
  case ' ':
    return 0x20;
  case ',':
    return 0xBC;
  case '.':
    return 0xBE;
  case '\'':
    return 0xDE;
  }
  assert(false && "no US key for this character");
  return 0;
}

// The Server side of the Stroke scheme, run against a real Engine session and a synthetic stroke.db. Stroke composes in the Server's candidate window like Cantonese: the TIP keeps its own composition in step key for key, so every key has to leave the Server's session where the TIP's host session leaves its own.
struct Fixture {
  ServerSession session;
  ReplyComposer composer{42, 1};
  uint64_t request = 0;
  NavigationBindings paging{};

  explicit Fixture(const std::string &options) : session(42, options) { session.activate(1); }

  std::optional<PendingReply> press(uint32_t code, char16_t text, uint32_t modifiers = 0,
                                    TsfPreeditStyle style = TsfPreeditStyle::Local,
                                    std::optional<std::string> local_text = std::nullopt) {
    const auto packet = key(++request, code, text, modifiers);
    auto reply = composer.configured_key(session, packet, 1, style, paging, std::move(local_text));
    if (reply)
      composer.confirm_delivery(42, 1, packet.request_id);
    return reply;
  }
  std::optional<PendingReply> press_text(char text, TsfPreeditStyle style = TsfPreeditStyle::Local) {
    return press(us_key(text), static_cast<char16_t>(text), 0, style);
  }
  void type(const char *keys) {
    for (const char *text = keys; *text; ++text)
      assert(press_text(*text));
  }
  nlohmann::json view() const { return session.view(); }
  std::string editing() const { return view().at("editing_text").get<std::string>(); }
  std::string preedit() const { return view().at("preedit").get<std::string>(); }
  nlohmann::json candidates() const { return view().at("candidates"); }
  std::string candidate(size_t slot) const { return candidates().at(slot).at("text").get<std::string>(); }
};
} // namespace

// argv[1] is tests/input/fixtures/stroke.db: the synthetic rows of the Engine's stroke fixture (crates/engine/src/stroke/mod.rs, made-up weights) in the language dictionary schema, written by Python's sqlite3 with a 512-byte page size. The Windows build links no SQLite of its own, so the file is checked in rather than built here.
int main(int argc, char **argv) {
  assert(argc == 2);
  const auto root = std::filesystem::temp_directory_path() /
                    ("msime-stroke-keys-" +
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
  std::filesystem::copy_file(std::filesystem::path(argv[1]), dictionaries / "stroke.db");
  auto options = test_host_options(root);
  options["language_dictionaries"] = dictionaries.u8string();
  options["preferences"]["scheme"] = "stroke";
  options["preferences"]["traditional_chinese_output"] = true;
  const auto serialized = options.dump();

  // Without its dictionary the scheme cannot run and the Engine answers with quanpin, the scheme the tray and the TIP then key (scheme::effective_scheme).
  {
    auto missing = test_host_options(root / "missing");
    missing["preferences"]["scheme"] = "stroke";
    Fixture fallback(missing.dump());
    assert(fallback.view().value("scheme", 0u) == 0u);
  }

  // h s p n z type the strokes: the preedit draws their glyphs, editing_text keeps the letters one per glyph, and the exact code ranks ahead of the longer codes it starts.
  {
    Fixture stroke(serialized);
    assert(stroke.view().value("scheme", 0u) == 8u);
    stroke.type("hs");
    assert(stroke.editing() == "hs" && stroke.preedit() == "一丨");
    assert(stroke.candidate(0) == "十" && stroke.candidate(1) == "土");
    // A character with two codes is listed once.
    size_t earth = 0;
    for (const auto &item : stroke.candidates())
      earth += item.at("text").get<std::string>() == "土" ? 1 : 0;
    assert(earth == 1);
    // Rows come from the read-only stroke.db: no 置顶, 固定 or 删除 is offered for them.
    const auto presented = candidate_presentation_from_view(FocusLease{{42, {1, 2, 3}}, 1, 1}, stroke.view(), 0, 0, "");
    assert(presented.visible && presented.preedit == "一丨" && !presented.candidates.empty());
    for (const auto &item : presented.candidates)
      assert(!item.actions_available);
    stroke.type("pnz");
    assert(stroke.preedit() == "一丨丿丶乛");
  }

  // x is the wildcard while composing: it matches any one stroke, and codes of the typed length come first.
  {
    Fixture stroke(serialized);
    stroke.type("hx");
    assert(stroke.editing() == "hx" && stroke.preedit() == "一＊");
    assert(stroke.candidate(0) == "二" && stroke.candidate(1) == "十");
  }

  // With nothing composing x and the other letters are not the scheme's: the Engine hands them back and nothing composes, which is why the TIP leaves them to the application (scheme::LetterPassesWhileIdle).
  for (const char other : {'x', 'a', 'q'}) {
    Fixture stroke(serialized);
    const auto reply = stroke.press_text(other);
    assert(!reply || !reply->committed_text);
    assert(stroke.editing().empty() && stroke.candidates().empty());
  }

  // While composing every other letter is swallowed: the composition stays as it was on both sides and nothing is written.
  for (const char other : {'a', 'q', 'v'}) {
    Fixture stroke(serialized);
    stroke.type("hs");
    const auto reply = stroke.press_text(other);
    assert(reply && !reply->committed_text);
    assert(stroke.editing() == "hs" && stroke.preedit() == "一丨");
  }

  // A digit picks the row in its slot; Stroke never learns, and its characters are written as stored whatever the Traditional output switch says.
  {
    Fixture stroke(serialized);
    stroke.type("hs");
    const auto second = stroke.candidate(1);
    const auto picked = stroke.press_text('2');
    assert(picked && picked->committed_text && *picked->committed_text == second);
    assert(!picked->traditional_output && stroke.editing().empty());
  }

  // Space commits the highlighted row; 干 stays 干 with Traditional output on.
  {
    Fixture stroke(serialized);
    stroke.type("hhs");
    assert(stroke.candidate(0) == "干");
    const auto committed = stroke.press_text(' ');
    assert(committed && committed->committed_text && *committed->committed_text == "干");
    assert(!committed->traditional_output && stroke.editing().empty());
  }

  // Enter commits the typed letters, which is what the TIP has already written from its own buffer.
  {
    Fixture stroke(serialized);
    stroke.type("hsp");
    const auto committed = stroke.press(0x0D, u'\r', 0, TsfPreeditStyle::Local, std::string("hsp"));
    assert(committed && committed->committed_text && *committed->committed_text == "hsp");
    assert(stroke.editing().empty());
  }

  // Backspace removes the last stroke; Escape discards the composition.
  {
    Fixture stroke(serialized);
    stroke.type("hhs");
    assert(stroke.press(0x08, u'\b'));
    assert(stroke.editing() == "hh" && stroke.preedit() == "一一" && stroke.candidate(0) == "二");
    const auto cancelled = stroke.press(0x1B, 0);
    assert(!cancelled || !cancelled->committed_text);
    assert(stroke.editing().empty() && stroke.candidates().empty());
  }

  // Punctuation while composing commits the highlighted row and then the Chinese mark.
  {
    Fixture stroke(serialized);
    stroke.type("hs");
    const auto ended = stroke.press_text(',');
    assert(ended && ended->committed_text && *ended->committed_text == "十，");
    assert(stroke.editing().empty());
  }

  // The apostrophe is no syllable separator under Stroke (the Engine refuses it): it is punctuation like the comma, so the highlighted row is committed followed by the mark, as on Linux and macOS, instead of the key being dropped with the composition left open.
  {
    Fixture stroke(serialized);
    stroke.type("hs");
    const auto ended = stroke.press_text('\'');
    assert(ended && ended->committed_text);
    const std::string written = *ended->committed_text;
    assert(written.rfind("十", 0) == 0 && written.size() > std::string("十").size());
    assert(stroke.editing().empty());
  }

  // The pinyin style reads the composition from the Server, so a stroke is answered with the glyphs it draws.
  {
    Fixture stroke(serialized);
    const auto reply = stroke.press_text('h', TsfPreeditStyle::Pinyin);
    assert(reply && reply->encoded && *reply->encoded &&
           reply->encoded->packet.msg_type == FanyImeReplyType::Preedit);
    assert(reply->encoded->packet.candidate_string[0] == static_cast<FanyImeWireChar>(u'一') &&
           reply->encoded->packet.candidate_string[1] == 0);
  }

  // Commits count under Stroke's own typing statistics source.
  assert(resolve_typing_source(8, false, false, "none", "xiaohe") == TypingSource::Stroke);
  return 0;
}

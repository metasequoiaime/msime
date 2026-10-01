#include "../src/core/KoreanHanja.h"

#include <ibus.h>

#include <cassert>
#include <initializer_list>

int main() {
  using msime::linux_host::korean_composition;
  using msime::linux_host::korean_hanja_key;
  using msime::linux_host::korean_hanja_list_open;
  using msime::linux_host::korean_hanja_punctuation_key;
  using msime::linux_host::korean_rules;
  using Json = nlohmann::json;

  // The keysyms are X's, so they are the IBus key values (and the Fcitx5 symbols, checked where FcitxEngine.cpp uses them).
  static_assert(msime::linux_host::kKeysymHangulHanja == static_cast<uint32_t>(IBUS_Hangul_Hanja));
  static_assert(msime::linux_host::kKeysymF9 == static_cast<uint32_t>(IBUS_F9));
  assert(korean_hanja_key(IBUS_Hangul_Hanja));
  assert(korean_hanja_key(IBUS_F9));
  for (uint32_t other : std::initializer_list<uint32_t>{IBUS_F8, IBUS_F10, IBUS_Hangul, IBUS_Return, IBUS_space, 'h'})
    assert(!korean_hanja_key(other));

  // A syllable composing under the Korean rules, then the same syllable with its Hanja list open.
  const Json composing = {{"scheme", 4}, {"dedicated_english", false}, {"local_mode", "none"},
                          {"editing_text", "gks"}, {"candidates", Json::array()}};
  Json open = composing;
  open["candidates"].push_back(Json{{"text", "韓"}});
  assert(korean_rules(composing) && korean_composition(composing) && !korean_hanja_list_open(composing));
  assert(korean_hanja_list_open(open));
  // The view the host parses carries unsigned numbers; a literal in a test carries signed ones.
  assert(korean_hanja_list_open(Json::parse(open.dump())));

  // Idle Korean is the Korean scheme with nothing to convert.
  Json idle = composing;
  idle["editing_text"] = "";
  assert(korean_rules(idle) && !korean_composition(idle) && !korean_hanja_list_open(idle));

  // The dedicated English mode and the local modes keep their own rules in the Korean scheme, candidates included.
  Json english = open;
  english["dedicated_english"] = true;
  assert(!korean_rules(english) && !korean_composition(english) && !korean_hanja_list_open(english));
  Json emoji = open;
  emoji["local_mode"] = "emoji";
  assert(!korean_rules(emoji) && !korean_hanja_list_open(emoji));

  // Every other scheme's candidates are not a Hanja list.
  for (int scheme : {0, 1, 2, 3}) {
    Json other = open;
    other["scheme"] = scheme;
    assert(!korean_rules(other) && !korean_hanja_list_open(other));
  }
  // A view without the fields (null before the first render) is not Korean.
  assert(!korean_rules(Json(nullptr)) && !korean_hanja_list_open(Json(nullptr)));
  assert(!korean_rules(Json::object()));
  assert(korean_rules(Json{{"scheme", 4}}));

  // The marks among the paging and word-to-character keys stay punctuation; the paging keys that are not marks do not.
  for (uint32_t mark : std::initializer_list<uint32_t>{IBUS_minus, IBUS_equal, IBUS_bracketleft, IBUS_bracketright, IBUS_comma, IBUS_period})
    assert(korean_hanja_punctuation_key(mark));
  for (uint32_t key : std::initializer_list<uint32_t>{IBUS_Page_Up, IBUS_Page_Down, IBUS_Tab, IBUS_Up, IBUS_Down, '1',
                       IBUS_semicolon, IBUS_apostrophe})
    assert(!korean_hanja_punctuation_key(key));
}

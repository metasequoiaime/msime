#include "PunctuationPolicy.h"

#include <iostream>
#include <stdexcept>
#include <string>
#include <string_view>

using namespace msime::windows;
namespace {
void require(bool value, int line) {
  if (!value)
    throw std::runtime_error("punctuation policy failed at line " + std::to_string(line));
}
#define REQUIRE(value) require((value), __LINE__)

FanyImeNamedpipeData key(unsigned code, wchar_t character) {
  FanyImeNamedpipeData packet{};
  packet.event_type = FanyImePipeEventType::KeyEvent;
  packet.keycode = code;
  packet.wch = static_cast<FanyImeWireChar>(character);
  return packet;
}

// The keycode a character arrives on does not matter to this predicate except
// for the six it names explicitly, so unrelated characters use a code that is
// none of them.
constexpr unsigned unrelated_key = 0xC0;
constexpr NavigationBindings no_paging{};
} // namespace

int main() {
  try {
    // The set, character by character, as the reference lists it in
    // IsCommitWithHighlightedCandidatePunctuationInCandidateMode. A character
    // missing here is a punctuation mark that stops ending the composition with
    // the highlighted candidate - visible while typing and nowhere else.
    constexpr std::string_view expected = "`!@#$%^&*()[]\\;:'\",<.>?";
    for (const char character : expected)
      REQUIRE(candidate_punctuation(key(unrelated_key, character), no_paging));
    REQUIRE(expected.size() == 23);

    // Characters that are punctuation but are not on the reference's list.
    for (const char character : "+-=_~|/{}")
      if (character != '\0')
        REQUIRE(!candidate_punctuation(key(unrelated_key, character), no_paging));
    // Letters and digits are not punctuation.
    REQUIRE(!candidate_punctuation(key(unrelated_key, 'a'), no_paging));
    REQUIRE(!candidate_punctuation(key(unrelated_key, '7'), no_paging));
    REQUIRE(!candidate_punctuation(key(unrelated_key, ' '), no_paging));

    // Minus, plus and their numpad twins never qualify whatever they carry.
    // The reference excludes them by keycode before it looks at the character.
    for (const unsigned code : {0xBDu, 0xBBu, 0x6Du, 0x6Bu}) {
      REQUIRE(!candidate_punctuation(key(code, '-'), no_paging));
      REQUIRE(!candidate_punctuation(key(code, '!'), no_paging));
    }

    // The Japanese scheme never pages on minus/equals, so their shifted and unshifted punctuation commits the highlighted candidate there. A bare '-' is the long-vowel mark and the numpad keys are unchanged.
    REQUIRE(candidate_punctuation(key(0xBB, '='), no_paging, true));
    REQUIRE(candidate_punctuation(key(0xBB, '+'), no_paging, true));
    REQUIRE(candidate_punctuation(key(0xBD, '_'), no_paging, true));
    REQUIRE(!candidate_punctuation(key(0xBD, '-'), no_paging, true));
    REQUIRE(!candidate_punctuation(key(0xBB, '!'), no_paging, true));
    REQUIRE(!candidate_punctuation(key(0x6B, '+'), no_paging, true));
    REQUIRE(!candidate_punctuation(key(0xBB, '='), no_paging));

    // Korean never pages on punctuation, even with a Hanja list open, so every ASCII mark is punctuation there, whatever the paging bindings say, and '-' and '=' among them. Letters, digits and marks above ASCII are not.
    {
      NavigationBindings paging = no_paging;
      paging.comma_period = true;
      paging.brackets = true;
      paging.minus_equal = true;
      for (const char character : "`~!@#$%^&*()-_=+[]{}\\|;:'\",<.>/?")
        if (character != '\0')
          REQUIRE(candidate_punctuation(key(unrelated_key, character), paging, false, true));
      REQUIRE(candidate_punctuation(key(0xBD, '-'), paging, false, true));
      REQUIRE(candidate_punctuation(key(0xBB, '='), paging, false, true));
      REQUIRE(candidate_punctuation(key(0xBC, ','), paging, false, true));
      REQUIRE(candidate_punctuation(key(0x6B, '+'), paging, false, true));
      REQUIRE(!candidate_punctuation(key('A', 'a'), paging, false, true));
      REQUIRE(!candidate_punctuation(key('7', '7'), paging, false, true));
      REQUIRE(!candidate_punctuation(key(0x20, ' '), paging, false, true));
      REQUIRE(!candidate_punctuation(key(unrelated_key, L'。'), paging, false, true));
    }

    // Comma and period are on the list, until they are bound as paging keys.
    NavigationBindings comma_period = no_paging;
    comma_period.comma_period = true;
    REQUIRE(candidate_punctuation(key(0xBC, ','), no_paging));
    REQUIRE(candidate_punctuation(key(0xBE, '.'), no_paging));
    REQUIRE(!candidate_punctuation(key(0xBC, ','), comma_period));
    REQUIRE(!candidate_punctuation(key(0xBE, '.'), comma_period));
    // Binding them must not disturb the rest of the list.
    REQUIRE(candidate_punctuation(key(unrelated_key, '?'), comma_period));
    REQUIRE(candidate_punctuation(key(0xDB, '['), comma_period));

    // Brackets, the same, under their own binding.
    NavigationBindings brackets = no_paging;
    brackets.brackets = true;
    REQUIRE(candidate_punctuation(key(0xDB, '['), no_paging));
    REQUIRE(candidate_punctuation(key(0xDD, ']'), no_paging));
    REQUIRE(!candidate_punctuation(key(0xDB, '['), brackets));
    REQUIRE(!candidate_punctuation(key(0xDD, ']'), brackets));
    REQUIRE(candidate_punctuation(key(0xBC, ','), brackets));

    // Each binding gates only its own pair.
    NavigationBindings both = no_paging;
    both.comma_period = true;
    both.brackets = true;
    REQUIRE(!candidate_punctuation(key(0xBC, ','), both));
    REQUIRE(!candidate_punctuation(key(0xDB, '['), both));
    REQUIRE(candidate_punctuation(key(unrelated_key, ';'), both));

    // No character above ASCII is one of these, including the ones whose low
    // byte is a listed mark - narrowing U+2021 would otherwise land on '!' and
    // a double dagger would end the composition as an exclamation mark.
    //
    // The `wch <= 127` bound inside the policy is not what stops them: the key
    // translator only calls 0x21..0x7E a character at all, so these never reach
    // it. The bound is a second guarantee that the narrowing cast below can
    // never see a value it cannot represent, and these assertions pin the
    // behaviour rather than that particular line.
    REQUIRE(!candidate_punctuation(key(unrelated_key, L'！'), no_paging)); // U+FF01
    REQUIRE(!candidate_punctuation(key(unrelated_key, L'。'), no_paging)); // U+3002
    REQUIRE(!candidate_punctuation(key(unrelated_key, L'\u2021'), no_paging)); // low byte '!'
    REQUIRE(!candidate_punctuation(key(unrelated_key, L'\u203F'), no_paging)); // low byte '?'


    // Anything the key translator does not call a character - Tab among them,
    // which the reference excludes by name - is not punctuation here either.
    REQUIRE(!candidate_punctuation(key(0x09, '\t'), no_paging));
    REQUIRE(!candidate_punctuation(key(0x0D, '\r'), no_paging));
    REQUIRE(!candidate_punctuation(key(0x1B, '\x1b'), no_paging));

    // The numpad arithmetic keys and '/' commit the highlighted candidate followed by the literal ASCII mark, never the Chinese punctuation the Engine would translate them to (reference 1d2431ad).
    REQUIRE(literal_candidate_punctuation(key(0x6B, '+')) == '+');
    REQUIRE(literal_candidate_punctuation(key(0x6D, '-')) == '-');
    REQUIRE(literal_candidate_punctuation(key(0x6E, '.')) == '.');
    REQUIRE(literal_candidate_punctuation(key(0x6F, '/')) == '/');
    // The numpad decimal is '.' whatever the layout reports for it.
    REQUIRE(literal_candidate_punctuation(key(0x6E, ',')) == '.');
    REQUIRE(literal_candidate_punctuation(key(0xBF, '/')) == '/');
    // The main-row minus and plus stay paging keys, and ordinary marks are translated.
    REQUIRE(literal_candidate_punctuation(key(0xBD, '-')) == 0);
    REQUIRE(literal_candidate_punctuation(key(0xBB, '=')) == 0);
    REQUIRE(literal_candidate_punctuation(key(0xBC, ',')) == 0);
    REQUIRE(literal_candidate_punctuation(key(0xBE, '.')) == 0);
    REQUIRE(literal_candidate_punctuation(key(0x41, 'a')) == 0);
    // A chord is the host's, not a mark.
    FanyImeNamedpipeData chord = key(0x6B, '+');
    chord.modifiers_down = 2;
    REQUIRE(literal_candidate_punctuation(chord) == 0);

    std::cout << "Windows candidate punctuation checks passed\n";
    return 0;
  } catch (const std::exception &error) {
    std::cerr << error.what() << '\n';
    return 1;
  }
}

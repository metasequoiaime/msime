#include "../../HostKoreanKey.h"
#include <cstdio>
#include <cstdlib>
#include <initializer_list>
#include <utility>

using msime::tsf::host_composed_key_action;
using msime::tsf::KoreanKeyAction;
namespace scheme = msime::windows::scheme;

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}

constexpr std::string_view idle = msime::tsf::kZhuyinIdleSymbols;
constexpr std::string_view dachen = msime::tsf::kZhuyinComposingSymbols;
constexpr std::string_view listOpen = msime::tsf::kZhuyinListOpenSymbols;
constexpr std::string_view vni = msime::tsf::kVietnameseVniDigits;
constexpr std::string_view tibetanIdle = msime::tsf::kTibetanIdleSymbols;
constexpr std::string_view tibetanComposing = msime::tsf::kTibetanComposingSymbols;

KoreanKeyAction tibetan(unsigned vk, wchar_t wch, bool composing) {
    return host_composed_key_action(scheme::Tibetan, vk, wch, composing, false,
                                    composing ? tibetanComposing : tibetanIdle);
}

KoreanKeyAction zhuyin(unsigned vk, wchar_t wch, bool composing, bool open = false) {
    return host_composed_key_action(scheme::Zhuyin, vk, wch, composing, open,
                                    !composing ? idle : open ? listOpen : dachen);
}
} // namespace

// The keys of the schemes the TIP composes in its own host session, as the immediate and the deferred classifiers decide them.
int main() {
    // Korean keeps its own rules.
    check(host_composed_key_action(scheme::Korean, 'R', L'r', false, false, {}) == KoreanKeyAction::Compose,
          "Korean letters compose");
    check(host_composed_key_action(scheme::Korean, 0xBE, L'.', true, false, {}) ==
              msime::tsf::korean_key_action(0xBE, L'.', true),
          "Korean punctuation follows korean_key_action");

    // Zhuyin: lowercase letters and the Dachen symbols spell, from idle too.
    check(zhuyin('S', L's', false) == KoreanKeyAction::Compose, "a letter starts a syllable");
    check(zhuyin('U', L'u', true) == KoreanKeyAction::Compose, "a letter extends the conversion");
    for (const auto &[vk, wch] : {std::pair<unsigned, wchar_t>{'1', L'1'}, {'5', L'5'}, {0xBC, L','}, {0xBF, L'/'},
                                  {0xBA, L';'}, {0xBD, L'-'}})
        check(zhuyin(vk, wch, false) == KoreanKeyAction::Compose, "an idle Dachen symbol starts a syllable");
    // Tone keys need a syllable: idle they keep the Chinese rules.
    check(zhuyin('3', L'3', false) == KoreanKeyAction::Default, "an idle tone digit is ordinary");
    check(zhuyin(0x20, L' ', false) == KoreanKeyAction::Default, "an idle Space is ordinary");
    // Composing, the tones and Space spell too: Space is the first tone, or opens the list in the Engine.
    for (const auto &[vk, wch] : {std::pair<unsigned, wchar_t>{'3', L'3'}, {'7', L'7'}, {0x20, L' '}, {0xBE, L'.'}})
        check(zhuyin(vk, wch, true) == KoreanKeyAction::Compose, "a composing Dachen key spells");
    // Shift letters are not phonetic: they end the conversion and follow it, or go to the application.
    check(zhuyin('A', L'A', true) == KoreanKeyAction::CommitWithText, "Shift+A ends the conversion");
    check(zhuyin('A', L'A', false) == KoreanKeyAction::Pass, "an idle Shift+A is the application's");
    // Shifted punctuation commits through the Chinese table; idle it keeps the Chinese rules.
    check(zhuyin(0xBC, L'<', true) == KoreanKeyAction::CommitWithText, "Shift+, commits with its mark");
    check(zhuyin(0xBF, L'?', false) == KoreanKeyAction::Default, "an idle Shift+/ is ordinary punctuation");
    // Down opens a closed list and is the list's own once it is open.
    check(zhuyin(0x28, 0, true) == KoreanKeyAction::ConvertHanja, "Down opens the list");
    check(zhuyin(0x28, 0, true, true) == KoreanKeyAction::HanjaList, "Down moves in the open list");
    check(zhuyin(0x28, 0, false) == KoreanKeyAction::Default, "an idle Down is the application's");
    // The open list takes its digits, Space and Enter; '0' and the punctuation keys still spell.
    for (const auto &[vk, wch] : {std::pair<unsigned, wchar_t>{'1', L'1'}, {0x69, L'9'}, {0x20, L' '}, {0x0D, L'\r'},
                                  {0x1B, 0x1B}, {0x08, L'\b'}, {0x22, 0}})
        check(zhuyin(vk, wch, true, true) == KoreanKeyAction::HanjaList, "the open list takes the key");
    check(zhuyin('0', L'0', true, true) == KoreanKeyAction::Compose, "0 spells with the list open");
    check(zhuyin(0xBC, L',', true, true) == KoreanKeyAction::Compose, "comma spells with the list open");
    check(zhuyin('S', L's', true, true) == KoreanKeyAction::Compose, "a letter closes the list and spells");
    // Enter commits; the caret keys commit and pass; Backspace and Escape edit.
    check(zhuyin(0x0D, L'\r', true) == KoreanKeyAction::CommitWithText, "Enter commits the conversion");
    for (const unsigned vk : {0x09u, 0x25u, 0x27u, 0x26u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu})
        check(zhuyin(vk, vk == 0x09 ? L'\t' : L'\0', true) == KoreanKeyAction::CommitAndPass,
              "a caret key commits and passes");
    check(zhuyin(0x08, L'\b', true) == KoreanKeyAction::Default, "Backspace edits the conversion");
    check(zhuyin(0x1B, 0x1B, true) == KoreanKeyAction::Default, "Escape cancels the conversion");

    // Vietnamese: letters compose with their case; VNI digits compose while a word is open.
    check(host_composed_key_action(scheme::Vietnamese, 'V', L'V', false, false, {}) == KoreanKeyAction::Compose,
          "an uppercase letter starts a word");
    check(host_composed_key_action(scheme::Vietnamese, '6', L'6', true, false, vni) == KoreanKeyAction::Compose,
          "a VNI digit spells");
    check(host_composed_key_action(scheme::Vietnamese, '6', L'6', true, false, {}) ==
              KoreanKeyAction::CommitWithText,
          "a Telex digit ends the word");
    check(host_composed_key_action(scheme::Vietnamese, '6', L'6', false, false, vni) == KoreanKeyAction::Pass,
          "an idle digit is the application's");
    check(host_composed_key_action(scheme::Vietnamese, 0xBE, L'.', true, false, {}) ==
              KoreanKeyAction::CommitWithText,
          "a period follows the word");
    check(host_composed_key_action(scheme::Vietnamese, 0x28, 0, true, false, {}) == KoreanKeyAction::CommitAndPass,
          "Down commits and passes: Vietnamese has no list");
    check(host_composed_key_action(scheme::Vietnamese, 0x0D, L'\r', true, false, {}) ==
              KoreanKeyAction::CommitAndPass,
          "Enter commits and passes");
    check(host_composed_key_action(scheme::Vietnamese, 0x08, L'\b', true, false, {}) == KoreanKeyAction::Default,
          "Backspace edits the word");

    // 藏文：字母连同大小写组字；`'` 随时组字，`+` `.` `-` 只在组字时组字；`/` 随时送进宿主会话（组字时带垂符上屏，空闲时单独输出垂符）。
    check(tibetan('B', L'b', false) == KoreanKeyAction::Compose, "a letter starts a Tibetan run");
    check(tibetan('T', L'T', true) == KoreanKeyAction::Compose, "an uppercase Wylie letter composes");
    check(tibetan(0xDE, L'\'', false) == KoreanKeyAction::Compose, "an idle apostrophe starts an achung syllable");
    check(tibetan(0xBF, L'/', false) == KoreanKeyAction::Compose, "an idle slash reaches the host session for a shad");
    check(tibetan(0xBF, L'/', true) == KoreanKeyAction::Compose, "a composing slash ends the run with a shad");
    for (const auto &[vk, wch] : {std::pair<unsigned, wchar_t>{0xBB, L'+'}, {0xBE, L'.'}, {0xBD, L'-'}})
        check(tibetan(vk, wch, true) == KoreanKeyAction::Compose, "a Wylie spelling symbol composes");
    check(tibetan(0xBE, L'.', false) == KoreanKeyAction::Pass, "an idle period is the application's");
    // 组字时空格送进宿主会话，由引擎带音节点上屏；空闲的空格属于应用。
    check(tibetan(0x20, L' ', true) == KoreanKeyAction::Compose, "a composing Space ends the run with a tsheg");
    check(tibetan(0x20, L' ', false) == KoreanKeyAction::Pass, "an idle Space is the application's");
    // 回车只上屏藏文并吃掉按键；数字和其他标点结束组字并跟在后面；光标键上屏后交给应用。
    check(tibetan(0x0D, L'\r', true) == KoreanKeyAction::CommitWithText, "Enter commits the run and is spent");
    check(tibetan('1', L'1', true) == KoreanKeyAction::CommitWithText, "a digit ends the run and follows it");
    check(tibetan('1', L'1', false) == KoreanKeyAction::Pass, "an idle digit is the application's");
    check(tibetan(0xBC, L',', true) == KoreanKeyAction::CommitWithText, "a comma follows the run");
    for (const unsigned vk : {0x09u, 0x25u, 0x27u, 0x26u, 0x28u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu})
        check(tibetan(vk, vk == 0x09 ? L'\t' : L'\0', true) == KoreanKeyAction::CommitAndPass,
              "a caret key commits the run and passes");
    check(tibetan(0x08, L'\b', true) == KoreanKeyAction::Default, "Backspace edits the raw Wylie");
    check(tibetan(0x1B, 0x1B, true) == KoreanKeyAction::Default, "Escape restores or cancels the run");
    // 实时视图判断：藏文组字时空格算组字接收的键，空闲时不算；其他方案的空格只看视图的拼写符号。
    using msime::tsf::host_composition_takes_key;
    check(host_composition_takes_key(scheme::Tibetan, tibetanComposing, L' ', true), "the live view takes a composing Space");
    check(!host_composition_takes_key(scheme::Tibetan, tibetanIdle, L' ', false), "the live view leaves an idle Space");
    check(host_composition_takes_key(scheme::Tibetan, tibetanIdle, L'/', false), "the live view takes an idle slash");
    check(!host_composition_takes_key(scheme::Tibetan, tibetanComposing, L',', true), "a comma ends the run");
    check(!host_composition_takes_key(scheme::Vietnamese, {}, L' ', true), "a Vietnamese Space ends the word");

    // Every other scheme keeps the ordinary classification.
    check(host_composed_key_action(scheme::Cantonese, 'S', L's', false, false, {}) == KoreanKeyAction::Default,
          "Cantonese keys are ordinary");
    check(host_composed_key_action(scheme::Stroke, 'H', L'h', true, false, {}) == KoreanKeyAction::Default &&
              host_composed_key_action(scheme::Stroke, 'X', L'x', false, false, {}) == KoreanKeyAction::Default,
          "Stroke keys take the ordinary classification, which LetterPassesWhileIdle narrows");

    // The projected Zhuyin list: Down opens it, a choice fixes a reading and keeps composing, Escape closes it.
    using msime::tsf::project_korean_hanja_key;
    const auto projects = [](unsigned vk, wchar_t wch, bool open, bool listOpen, bool ends) {
        const auto projected = project_korean_hanja_key(scheme::Zhuyin, vk, wch, open);
        return projected.listOpen == listOpen && projected.syllableEnds == ends;
    };
    check(projects(0x28, 0, false, true, false), "a queued Down opens the projected list");
    check(projects(0x28, 0, true, true, false), "a queued Down moves in the open list");
    check(projects('3', L'3', true, false, false), "a choice closes the list and keeps composing");
    check(projects(0x20, L' ', true, false, false), "Space fixes the highlighted reading and keeps composing");
    check(projects(0x1B, 0x1B, true, false, false), "Escape closes the projected list");
    check(projects(msime::tsf::kVirtualKeyHanja, 0, false, false, false), "the Hanja key is not Zhuyin's");

    if (failures)
        return EXIT_FAILURE;
    std::puts("Host-composed keys: Zhuyin spells and opens its list, Vietnamese and Tibetan compose in place, Korean is unchanged");
    return EXIT_SUCCESS;
}

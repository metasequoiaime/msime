#include "../../HostKoreanKey.h"
#include <cstdio>
#include <cstdlib>
#include <initializer_list>
#include <utility>

using msime::tsf::KoreanKeyAction;
using msime::tsf::korean_key_action;
using msime::tsf::korean_letter;

namespace {
int failures = 0;

void check(bool condition, const char *what) {
    if (!condition) {
        std::fprintf(stderr, "FAIL: %s\n", what);
        ++failures;
    }
}
} // namespace

int main() {
    // Every letter composes, composing or not, whatever case the layout produced.
    check(korean_key_action('R', L'r', false) == KoreanKeyAction::Compose, "r starts a syllable");
    check(korean_key_action('R', L'R', true) == KoreanKeyAction::Compose, "Shift+R composes");
    check(korean_key_action('Q', L'q', true) == KoreanKeyAction::Compose, "q composes inside a syllable");

    // Shift decides the case, so Shift+Q is the tense ㅃ and Caps Lock alone is not.
    check(korean_letter(L'Q', false) == L'q', "Caps Lock Q is plain q");
    check(korean_letter(L'q', true) == L'Q', "Shift q is Q");
    check(korean_letter(L'Q', true) == L'Q', "Shift with Caps Lock still Q");
    check(korean_letter(L'k', false) == L'k', "plain k stays k");

    // With a syllable open, printable keys commit it and carry their own character; space and digits included.
    check(korean_key_action(0x20, L' ', true) == KoreanKeyAction::CommitWithText, "space ends the syllable");
    check(korean_key_action('1', L'1', true) == KoreanKeyAction::CommitWithText, "digit ends the syllable");
    check(korean_key_action(0x61, L'1', true) == KoreanKeyAction::CommitWithText, "numpad digit ends the syllable");
    check(korean_key_action(0xBE, L'.', true) == KoreanKeyAction::CommitWithText, "period follows the syllable");
    check(korean_key_action(0xBD, L'-', true) == KoreanKeyAction::CommitWithText, "minus is punctuation, not paging");
    check(korean_key_action(0xBB, L'=', true) == KoreanKeyAction::CommitWithText, "equals is punctuation, not paging");
    check(korean_key_action(0x31, L'!', true) == KoreanKeyAction::CommitWithText, "Shift+1 is punctuation");

    // Caret and editing keys commit and still reach the application.
    for (const unsigned vk : {0x0Du, 0x09u, 0x25u, 0x26u, 0x27u, 0x28u, 0x24u, 0x23u, 0x21u, 0x22u, 0x2Eu, 0x2Du})
        check(korean_key_action(vk, vk == 0x0D ? L'\r' : vk == 0x09 ? L'\t' : L'\0', true) ==
                  KoreanKeyAction::CommitAndPass,
              "caret or editing key commits and passes");

    // Backspace and Escape keep their composition meaning: one jamo off, or the syllable discarded.
    check(korean_key_action(0x08, L'\b', true) == KoreanKeyAction::Default, "Backspace edits the syllable");
    check(korean_key_action(0x1B, 0x1B, true) == KoreanKeyAction::Default, "Escape cancels the syllable");

    // Nothing composing: punctuation and digits go to the application as half-width ASCII.
    check(korean_key_action(0xBE, L'.', false) == KoreanKeyAction::Pass, "idle period passes");
    check(korean_key_action('1', L'1', false) == KoreanKeyAction::Pass, "idle digit passes");
    check(korean_key_action(0x20, L' ', false) == KoreanKeyAction::Pass, "idle space passes");
    check(korean_key_action(0x0D, L'\r', false) == KoreanKeyAction::Default, "idle Enter is ordinary");

    // The Hanja key converts the open syllable and closes an open list; with nothing composing it is the application's.
    check(korean_key_action(msime::tsf::kVirtualKeyHanja, L'\0', true) == KoreanKeyAction::ConvertHanja,
          "Hanja converts the syllable");
    check(korean_key_action(msime::tsf::kVirtualKeyHanja, L'\0', true, true) == KoreanKeyAction::ConvertHanja,
          "Hanja closes an open list");
    check(korean_key_action(msime::tsf::kVirtualKeyHanja, L'\0', false) == KoreanKeyAction::Pass, "idle Hanja passes");

    // With the Hanja list open, the list takes digits, Space, Enter, the arrows, paging, Home/End, Escape and Backspace.
    for (const auto &[vk, wch] : {std::pair<unsigned, wchar_t>{'1', L'1'}, {'9', L'9'}, {0x61, L'1'}, {0x20, L' '},
                                  {0x0D, L'\r'}, {0x25, L'\0'}, {0x26, L'\0'}, {0x27, L'\0'}, {0x28, L'\0'},
                                  {0x21, L'\0'}, {0x22, L'\0'}, {0x24, L'\0'}, {0x23, L'\0'}, {0x1B, 0x1B},
                                  {0x08, L'\b'}})
        check(korean_key_action(vk, wch, true, true) == KoreanKeyAction::HanjaList, "the open list takes the key");
    // Every other key keeps its meaning: a letter composes, punctuation, '0' and Shift+1 end the syllable, Tab and Delete commit and pass.
    check(korean_key_action('R', L'r', true, true) == KoreanKeyAction::Compose, "a letter closes the list and composes");
    check(korean_key_action(0xBE, L'.', true, true) == KoreanKeyAction::CommitWithText, "a mark stays punctuation");
    check(korean_key_action(0xBD, L'-', true, true) == KoreanKeyAction::CommitWithText, "minus does not page the list");
    check(korean_key_action('0', L'0', true, true) == KoreanKeyAction::CommitWithText, "zero selects nothing");
    check(korean_key_action(0x31, L'!', true, true) == KoreanKeyAction::CommitWithText, "Shift+1 is punctuation");
    check(korean_key_action(0x09, L'\t', true, true) == KoreanKeyAction::CommitAndPass, "Tab commits and passes");
    check(korean_key_action(0x2E, L'\0', true, true) == KoreanKeyAction::CommitAndPass, "Delete commits and passes");
    check(korean_key_action(0x70, L'\0', true, true) == KoreanKeyAction::Default, "F1 is ordinary with the list open");

    // The list maps its keys to the same commands on the TIP and the Server.
    using msime::windows::KoreanHanjaKeyKind;
    using msime::windows::korean_hanja_key;
    check(korean_hanja_key(msime::tsf::kVirtualKeyHanja, 0).kind == KoreanHanjaKeyKind::Command &&
              korean_hanja_key(msime::tsf::kVirtualKeyHanja, 0).value == MSIME_CONVERT_HANJA,
          "Hanja sends MSIME_CONVERT_HANJA");
    check(korean_hanja_key('3', L'3').kind == KoreanHanjaKeyKind::Select && korean_hanja_key('3', L'3').value == 2,
          "3 chooses the third slot");
    check(korean_hanja_key(0x63, L'3').value == 2, "numpad 3 chooses the third slot");
    check(korean_hanja_key(0x0D, L'\r').value == MSIME_COMMIT_CANDIDATE, "Enter chooses the highlighted Hanja");
    check(korean_hanja_key(0x20, L' ').value == MSIME_COMMIT_CANDIDATE, "Space chooses the highlighted Hanja");
    check(korean_hanja_key(0x1B, 0x1B).value == MSIME_CANCEL, "Escape closes the list");
    check(korean_hanja_key(0x08, L'\b').value == MSIME_BACKSPACE, "Backspace closes the list");
    check(korean_hanja_key(0x28, 0).value == MSIME_NEXT_CANDIDATE, "Down moves the highlight");
    check(korean_hanja_key(0x22, 0).value == MSIME_NEXT_PAGE, "Page Down turns the page");
    check(korean_hanja_key(0xBC, L',').kind == KoreanHanjaKeyKind::None, "comma is not a list key");

    // Behind the deferred-key barrier the list is projected through the queue. The Hanja key opens a closed list and closes an open one, and the syllable keeps composing either way.
    using msime::tsf::project_korean_hanja_key;
    const auto projects = [](unsigned vk, wchar_t wch, bool open, bool listOpen, bool syllableEnds) {
        const auto projected = project_korean_hanja_key(msime::windows::scheme::Korean, vk, wch, open);
        return projected.listOpen == listOpen && projected.syllableEnds == syllableEnds;
    };
    const unsigned hanja = msime::tsf::kVirtualKeyHanja;
    check(projects(hanja, L'\0', false, true, false), "a queued Hanja key opens the projected list");
    check(projects(hanja, L'\0', true, false, false), "a second Hanja key closes it and keeps the syllable");
    // Escape and Backspace queued behind the Hanja key close the list and keep the syllable, as the Server's session does with them, rather than cancelling or editing it.
    check(projects(0x1B, 0x1B, true, false, false), "Escape closes the projected list and keeps the syllable");
    check(projects(0x08, L'\b', true, false, false), "Backspace closes the projected list and keeps the syllable");
    // Moving in the list keeps it open and the syllable composing.
    for (const unsigned vk : {0x25u, 0x26u, 0x27u, 0x28u, 0x21u, 0x22u, 0x24u, 0x23u})
        check(projects(vk, L'\0', true, true, false), "a move keeps the projected list open");
    // Space, Enter and a digit from either row choose and end the syllable.
    for (const auto &[vk, wch] : {std::pair<unsigned, wchar_t>{0x20, L' '}, {0x0D, L'\r'}, {'1', L'1'}, {0x69, L'9'}})
        check(projects(vk, wch, true, false, true), "a choice ends the syllable");
    // With the list projected closed only the Hanja key is queued as a list key; every other key keeps its action, so an arrow still commits the syllable and a Backspace after it is the application's again.
    check(korean_key_action(0x25, L'\0', true, false) == KoreanKeyAction::CommitAndPass,
          "an arrow with the list projected closed commits and passes");
    check(korean_key_action(0x1B, 0x1B, true, false) == KoreanKeyAction::Default,
          "Escape with the list projected closed cancels the syllable");

    // Keys with no character and no editing role are left to the ordinary classification.
    check(korean_key_action(0x70, L'\0', true) == KoreanKeyAction::Default, "F1 is ordinary");

    if (failures)
        return EXIT_FAILURE;
    std::puts("Korean key policy: letters compose, the Hanja list takes its keys, other keys end the syllable");
    return EXIT_SUCCESS;
}

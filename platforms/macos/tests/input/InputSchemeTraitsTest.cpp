#include "../../src/input/InputSchemeTraits.h"

#include <cassert>
#include <initializer_list>

using namespace msime::mac::scheme;

namespace
{
// One row per scheme ordinal: the Engine predicates the view does not publish, mirrored from crates/engine/src/types.rs, and the host-only traits. Index 9 is an ordinal no Engine knows. Rows 0-4 are the rules this host applied before the traits existed: the smart punctuation gestures were off for Japanese and Korean (japaneseSchemeActive, koreanSchemeActive), glosses were off for Japanese and on for Korean's Hanja rows (the gloss gates), and every Korean-only branch (MSIMEKoreanComposition) answered for scheme 4 alone. Rows 5-7 follow the `SchemeType` predicates; scheme 6 (Zhuyin) follows the design table until the Engine has the variant. Row 8 (Stroke) copies Cantonese, as the Engine's `SchemeType` predicates do.
struct Row
{
    bool commitsOnBlur, locksCaret, usesChinesePunctuation, hostSmartPunctuation, widensFullWidth, showsGlosses;
    bool foldsLetterCase, capsLockBypassExempt, letterComposition, opensCandidateList, alwaysInlinePreedit;
};

constexpr Row Expected[] = {
    // commits locks chinese smart  wide   gloss  fold   caps   letter opens  inline
    {false, false, true, true, true, true, false, false, false, false, false},    // 0 quanpin
    {false, false, true, true, true, true, false, false, false, false, false},    // 1 shuangpin
    {false, false, true, true, true, true, false, false, false, false, false},    // 2 wubi
    {false, false, true, false, true, false, false, false, false, false, false},  // 3 japanese
    {true, true, false, false, false, true, true, true, true, true, true},        // 4 korean
    {false, false, true, true, true, false, false, false, false, false, false},   // 5 cantonese
    {true, true, true, false, true, false, false, false, false, true, true},      // 6 zhuyin
    {true, true, false, false, false, false, false, true, true, false, true},     // 7 vietnamese
    {false, false, true, true, true, false, false, false, false, false, false},   // 8 stroke
    {false, false, false, false, false, false, false, false, false, false, false} // unknown
};

constexpr bool Matches(int scheme, const Row &row)
{
    return CommitsOnBlur(scheme) == row.commitsOnBlur && LocksCaret(scheme) == row.locksCaret &&
           UsesChinesePunctuation(scheme) == row.usesChinesePunctuation &&
           HostSmartPunctuation(scheme) == row.hostSmartPunctuation && WidensFullWidth(scheme) == row.widensFullWidth &&
           ShowsGlosses(scheme) == row.showsGlosses && FoldsLetterCase(scheme) == row.foldsLetterCase &&
           CapsLockBypassExempt(scheme) == row.capsLockBypassExempt &&
           LetterComposition(scheme) == row.letterComposition && OpensCandidateList(scheme) == row.opensCandidateList &&
           AlwaysInlinePreedit(scheme) == row.alwaysInlinePreedit;
}

static_assert(Quanpin == 0 && Shuangpin == 1 && Wubi == 2 && Japanese == 3 && Korean == 4 && Cantonese == 5 &&
              Zhuyin == 6 && Vietnamese == 7 && Stroke == 8);
static_assert(Matches(Quanpin, Expected[0]) && Matches(Shuangpin, Expected[1]) && Matches(Wubi, Expected[2]));
static_assert(Matches(Japanese, Expected[3]) && Matches(Korean, Expected[4]));
static_assert(Matches(Cantonese, Expected[5]) && Matches(Zhuyin, Expected[6]) && Matches(Vietnamese, Expected[7]));
static_assert(Matches(Stroke, Expected[8]));
// Ordinals no Engine knows answer false everywhere, the way host-api reads an unknown scheme.
static_assert(Matches(9, Expected[9]) && Matches(-1, Expected[9]) && Matches(255, Expected[9]));
} // namespace

int main()
{
    // The same table at run time, so a failure names the row in a debugger rather than only failing the build.
    for (int scheme = 0; scheme <= 8; ++scheme) assert(Matches(scheme, Expected[scheme]));
    for (int scheme : {9, 99, -1}) assert(Matches(scheme, Expected[9]));
    return 0;
}

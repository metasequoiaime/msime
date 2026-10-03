#include "../../src/input/InputSchemeTraits.h"

#include <cassert>
#include <initializer_list>

using namespace msime::mac::scheme;

namespace
{
// 每个方案序号一行：视图不发布、从 crates/engine/src/types.rs 镜像过来的引擎谓词，以及宿主自己的特性。下标 10 是没有任何引擎认识的序号。第 0-4 行是有这些特性之前本宿主用的规则：智能标点手势在日文和韩文下关闭（japaneseSchemeActive、koreanSchemeActive），释义在日文下关闭、在韩文汉字行上打开（释义开关），所有只给韩文的分支（MSIMEKoreanComposition）只认方案 4。第 5-9 行按 `SchemeType` 谓词填写；藏文（8）的引擎谓词与越南文完全相同，威利转写区分大小写，所以大写锁定下的大写字母也交给引擎组字；笔画（9）照抄粤拼，与引擎的 `SchemeType` 谓词一致。
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
    {true, true, false, false, false, false, false, true, true, false, true},     // 8 tibetan
    {false, false, true, true, true, false, false, false, false, false, false},   // 9 stroke
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
              Zhuyin == 6 && Vietnamese == 7 && Tibetan == 8 && Stroke == 9);
static_assert(Matches(Quanpin, Expected[0]) && Matches(Shuangpin, Expected[1]) && Matches(Wubi, Expected[2]));
static_assert(Matches(Japanese, Expected[3]) && Matches(Korean, Expected[4]));
static_assert(Matches(Cantonese, Expected[5]) && Matches(Zhuyin, Expected[6]) && Matches(Vietnamese, Expected[7]) &&
              Matches(Tibetan, Expected[8]) && Matches(Stroke, Expected[9]));
// 没有引擎认识的序号处处为 false，与 host-api 读未知方案的方式一致。
static_assert(Matches(10, Expected[10]) && Matches(-1, Expected[10]) && Matches(255, Expected[10]));
} // namespace

int main()
{
    // The same table at run time, so a failure names the row in a debugger rather than only failing the build.
    for (int scheme = 0; scheme <= 9; ++scheme) assert(Matches(scheme, Expected[scheme]));
    for (int scheme : {10, 99, -1}) assert(Matches(scheme, Expected[10]));
    return 0;
}

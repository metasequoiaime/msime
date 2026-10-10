#include "../../Global/JapaneseConversionPolicy.h"
#include <cstdio>
#include <cstdlib>

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
    using namespace Global;

    // 日语空格的导航回执不上屏，转换开始；普通回执和其他方案照旧。
    check(JapaneseSpaceReplyKeepsComposition(FanyImeReplyType::NavigationIgnored, true), "start keeps the composition");
    check(JapaneseSpaceReplyKeepsComposition(FanyImeReplyType::MoveSelectionNext, true), "a step keeps the composition");
    check(!JapaneseSpaceReplyKeepsComposition(FanyImeReplyType::Normal, true), "a commit still commits");
    check(!JapaneseSpaceReplyKeepsComposition(FanyImeReplyType::OutofRange, true), "out of range is not a conversion");
    check(!JapaneseSpaceReplyKeepsComposition(FanyImeReplyType::UiLessComposition, true), "UILess composition is not a conversion");
    check(!JapaneseSpaceReplyKeepsComposition(FanyImeReplyType::NavigationIgnored, false), "other schemes keep their rule");

    // 回车：同一段组字、同一段读音、裸回车时上屏高亮候选，否则是假名。
    const JapaneseConversionMark mark{7, "nihon"};
    check(JapaneseEnterCommitsCandidate(mark, 7, "nihon", true, true), "Enter takes the stepped candidate");
    check(!JapaneseEnterCommitsCandidate(mark, 8, "nihon", true, true), "a new composition with the same reading starts over");
    check(!JapaneseEnterCommitsCandidate(mark, 7, "nihong", true, true), "editing the reading abandons the conversion");
    check(!JapaneseEnterCommitsCandidate(mark, 7, "nihon", false, true), "other schemes keep their Enter");
    check(!JapaneseEnterCommitsCandidate(mark, 7, "nihon", true, false), "a modified Enter keeps its meaning");
    check(!JapaneseEnterCommitsCandidate(JapaneseConversionMark{}, 0, "", true, true), "no conversion means the kana");
    check(!JapaneseEnterCommitsCandidate(JapaneseConversionMark{7, ""}, 7, "", true, true), "nothing composed means the kana");

    return failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

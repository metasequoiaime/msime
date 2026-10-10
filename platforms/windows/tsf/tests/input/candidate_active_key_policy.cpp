#include "../../Global/CandidateActiveKeyPolicy.h"
#include <cstdio>
#include <initializer_list>

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
    const unsigned shift = 0b001u, ctrl = 0b010u, alt = 0b100u;

    // 释义列与释义页认的键：候选列表开着时都要带 CandidateActive。
    check(CandidateKeyReportsActiveList(0x09u, 0), "Tab arms a gloss column");
    check(CandidateKeyReportsActiveList(0x09u, shift), "Shift+Tab arms a gloss column backwards");
    check(CandidateKeyReportsActiveList(0x20u, 0), "Space commits the armed column or the first sense");
    for (unsigned key = '1'; key <= '9'; ++key)
        check(CandidateKeyReportsActiveList(key, 0), "a bare digit commits the armed column or a sense");
    check(CandidateKeyReportsActiveList(0x61u, 0) && CandidateKeyReportsActiveList(0x69u, 0), "a numpad digit picks a sense too");
    for (unsigned key : {0x21u, 0x22u, 0x23u, 0x24u, 0x26u, 0x28u})
        check(CandidateKeyReportsActiveList(key, 0), "paging keys stay on the sense page");
    check(CandidateKeyReportsActiveList('1', alt), "Alt+1 commits the first gloss column");
    check(CandidateKeyReportsActiveList('9', ctrl), "Ctrl+9 commits the second gloss column");
    check(CandidateKeyReportsActiveList(0x0Du, ctrl), "Ctrl+Enter commits or opens the senses");

    // 这些键靠没有 CandidateActive 走别的路，不能带。
    check(!CandidateKeyReportsActiveList(0x0Du, 0), "a bare Enter commits the raw composition");
    check(!CandidateKeyReportsActiveList(0x0Du, shift), "Shift+Enter keeps its own meaning");
    check(!CandidateKeyReportsActiveList(0xDBu, 0) && !CandidateKeyReportsActiveList(0xDDu, 0), "brackets take a character of the word");
    check(!CandidateKeyReportsActiveList(0xBDu, 0) && !CandidateKeyReportsActiveList(0xBBu, 0), "minus and equal take a character of the word");
    check(!CandidateKeyReportsActiveList(0x08u, ctrl), "Ctrl+Backspace restores the last segment");
    check(!CandidateKeyReportsActiveList(0x08u, 0), "Backspace edits the composition");
    check(!CandidateKeyReportsActiveList(0x25u, 0) && !CandidateKeyReportsActiveList(0x27u, 0), "left and right move the caret");
    check(!CandidateKeyReportsActiveList('A', 0), "a letter extends the composition");
    check(!CandidateKeyReportsActiveList('0', 0), "0 never picks a candidate");

    // 不是释义列快捷键的组合键。
    check(!CandidateKeyReportsActiveList('1', ctrl | alt), "Ctrl+Alt+1 is neither column");
    check(!CandidateKeyReportsActiveList('1', shift), "Shift+1 types a symbol");
    check(!CandidateKeyReportsActiveList('1', alt | shift), "Alt+Shift+1 is not a column");
    check(!CandidateKeyReportsActiveList(0x61u, alt), "Alt+numpad digit is an Alt code");
    check(!CandidateKeyReportsActiveList(0x20u, shift), "Shift+Space is not Space");
    check(!CandidateKeyReportsActiveList(0x09u, ctrl), "Ctrl+Tab belongs to the application");
    check(!CandidateKeyReportsActiveList(0x0Du, alt), "Alt+Enter is not the translation shortcut");

    if (failures == 0)
        std::puts("candidate-active keys: gloss columns and the sense page only");
    return failures == 0 ? 0 : 1;
}

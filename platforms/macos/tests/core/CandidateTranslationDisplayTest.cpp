#include "../../src/core/CandidateTranslationDisplay.h"

#include <cassert>
#include <string>

// 候选释义的显示规则：词性标签与单条释义两个开关，以及按目标语言分行套用。
//
// macOS 的释义可能带一份到多份目标语言，宿主内部用 "\n" 分行；词性标签和义项分隔符
// 都只在一行内部生效，换行不是义项分隔符。这里逐条钉住判定结果。
int main() {
    using msime::mac::format_translation_for_display;
    using msime::mac::format_translation_gloss_for_display;

    // 出厂默认：显示词性、显示全部释义时整条原样保留。
    assert(format_translation_gloss_for_display("n. silver medal; adj. spreading", true, false) ==
           "n. silver medal; adj. spreading");
    assert(format_translation_gloss_for_display("v. teach, instruct", true, false) ==
           "v. teach, instruct");

    // 关掉词性：标签连同其后的一个空格去掉，义项分隔符和其余释义保留。
    assert(format_translation_gloss_for_display("n. silver medal; adj. spreading", false, false) ==
           "silver medal; spreading");
    assert(format_translation_gloss_for_display("proper n. Wanshan District", false, false) ==
           "Wanshan District");
    assert(format_translation_gloss_for_display("adj. spreading; n. sound of laughter", false, false) ==
           "spreading; sound of laughter");

    // 只取第一条释义：第一个义项分隔符之后全部丢掉；默认显示词性时标签保留。
    assert(format_translation_gloss_for_display("n. silver medal; adj. spreading", true, true) ==
           "n. silver medal");
    assert(format_translation_gloss_for_display("n. silver medal; adj. spreading", false, true) ==
           "silver medal");

    // 全角分号同样算义项分隔符；义项两端的空白在截断时补齐。
    assert(format_translation_gloss_for_display("苹果\xEF\xBC\x9B家伙", false, true) == "苹果");
    assert(format_translation_gloss_for_display("  你好  ;  嗨  ", true, true) == "  你好");
    assert(format_translation_gloss_for_display("hello ; hi", false, false) == "hello ; hi");

    // 没有词性标签时关掉词性不影响释义。
    assert(format_translation_gloss_for_display("hello; hi", false, false) == "hello; hi");

    // 词典释义的英文同义词：single_sense 且 first_synonym 时取到首个逗号前，也是 Windows
    // 对离线词典条目的行为；括号里的逗号不算。
    assert(format_translation_gloss_for_display("v. teach, instruct", true, true, true) ==
           "v. teach");
    assert(format_translation_gloss_for_display("n. acid (LSD, LSD-25), dose", true, true, true) ==
           "n. acid (LSD, LSD-25)");

    // 按目标语言分行：每行各自截断，换行本身保留。
    assert(format_translation_for_display("n. first; adj. second\nn. 第三；adj. 第四", true, true) ==
           "n. first\nn. 第三");
    assert(format_translation_for_display("n. first; adj. second\nn. 第三；adj. 第四", false, false) ==
           "first; second\n第三；第四");
    // 换行只要一行时不动。
    assert(format_translation_for_display("first line\nsecond line", true, true) ==
           "first line\nsecond line");
    // 没有释义时保持为空。
    assert(format_translation_for_display("", true, true).empty());
    return 0;
}

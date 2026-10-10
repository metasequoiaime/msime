import app.msime.android.CompositionCaretPolicy;
import app.msime.android.InputSchemeTraits;
import app.msime.android.PhrasePreeditPolicy;

import java.util.Arrays;

public final class CompositionCaretPolicySmoke {
    static void check(boolean condition, String what) {
        if (!condition) throw new AssertionError(what);
    }

    static void moves(int current, int target, int length, int... expected) {
        int[] actual = CompositionCaretPolicy.moves(current, target, length);
        check(Arrays.equals(actual, expected),
            "moves " + current + "->" + target + " gave " + Arrays.toString(actual));
    }

    public static void main(String[] args) {
        // 能点的方案：光标留在组字里的那些。韩语、注音、越南语、藏文锁住光标，日语和笔画画的是 reading，本地模式和专用英文各管各的。
        check(CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "jintianci"), "quanpin");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.SHUANGPIN, "none", false, "jntm"), "shuangpin");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.WUBI, "none", false, "wqvb"), "wubi");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.CANTONESE, "none", false, "nei hou"), "cantonese");
        for (int scheme : new int[] {InputSchemeTraits.KOREAN, InputSchemeTraits.ZHUYIN,
                InputSchemeTraits.VIETNAMESE, InputSchemeTraits.TIBETAN, InputSchemeTraits.JAPANESE,
                InputSchemeTraits.STROKE, -1}) {
            check(!CompositionCaretPolicy.editable(scheme, "none", false, "abc"), "scheme " + scheme);
        }
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "emoji", false, "vbq"), "local mode");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", true, "hello"), "dedicated English");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, ""), "nothing composed");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "ni好"), "non-ASCII spelling");

        // 全拼：读音就是按键。#5613 的例子：jintianciwufanlemei 里把 ci 改成 chi，点在 c 和 i 之间。
        String typed = "jintianciwufanlemei";
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, -1, 8) == 8, "tap between c and i");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, -1, 99) == typed.length(), "past the end");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, -1, 0) == 0, "the start");
        // 光标在 9 处画成 jintianc|iwufanlemei；点光标符后面的 i 之后，换算时去掉光标符。
        int mark = CompositionCaretPolicy.markIndex(typed, typed, 8);
        check(mark == 8, "caret drawn where it is");
        check("jintianc|iwufanlemei".equals(CompositionCaretPolicy.withMark(typed, mark)), "caret mark drawn");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, mark, 10) == 9, "a tap after the mark skips it");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, mark, 8) == 8, "a tap before the mark");
        check(CompositionCaretPolicy.markIndex(typed, typed, typed.length()) == -1, "a caret at the end is not drawn");

        // 已选好的词画在读音前面：点在它上面，光标到开头。
        check(CompositionCaretPolicy.tapTarget(2, "fanlemei", "fanlemei", -1, 1) == 0, "tap on the chosen piece");
        check(CompositionCaretPolicy.tapTarget(2, "fanlemei", "fanlemei", -1, 5) == 3, "tap after the chosen piece");

        // 九键：读音行画的是拼音（9426 画成 xi'an），按键是数字；字母按键位对上数字，显示用的分隔跳过。
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9426", -1, 3) == 2, "tap after the separator");
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9426", -1, 2) == 2, "tap before the separator");
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9426", -1, 4) == 3, "tap inside the second syllable");
        // 同一个光标位置隔着分隔对应两处时画在分隔后面：引擎的退格先删光标前的切分。
        check(CompositionCaretPolicy.markIndex("94'26", "9426", 2) == 3, "caret after a split");
        check("94'|26".equals(CompositionCaretPolicy.withMark("94'26", 3)), "split then caret");
        // 选过拼音之后：ni'426 对 64426。
        check(CompositionCaretPolicy.tapTarget(0, "ni'426", "64426", -1, 4) == 3, "locked spelling then digits");
        // 首选是简拼行时读音行是首字母（m't 对 68）。
        check(CompositionCaretPolicy.tapTarget(0, "m't", "68", -1, 2) == 1, "initials reading");
        // 粤拼的按键里本身就有空格。
        check(CompositionCaretPolicy.tapTarget(0, "nei hou", "nei hou", -1, 4) == 4, "a typed space is a key");

        // 读音对不上按键时不移光标。
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9427", -1, 3) == -1, "a reading of other keys");
        check(CompositionCaretPolicy.tapTarget(0, "xi", "9426", -1, 1) == -1, "a reading shorter than the keys");
        check(CompositionCaretPolicy.markIndex("xi'an", "9427", 1) == -1, "no mark on a reading of other keys");
        check(CompositionCaretPolicy.tapTarget(0, null, "64", -1, 1) == -1, "no reading");
        check(CompositionCaretPolicy.tapTarget(0, "ni", "64", -1, -1) == -1, "no layout offset");

        // #6110：光标符画成竖条，要知道它在整段标题（已选的词 + 读音）里的下标；标题里那个位置必须正是光标符。
        int nineKeyMark = CompositionCaretPolicy.markIndex("xi'an", "9426", 2);
        String nineKeyTitle = PhrasePreeditPolicy.title("饭了",
            CompositionCaretPolicy.withMark("xi'an", nineKeyMark), false);
        int barIndex = CompositionCaretPolicy.markInTitle("饭了", nineKeyMark, false);
        check(barIndex == 5 && nineKeyTitle.charAt(barIndex) == CompositionCaretPolicy.CARET_MARK,
            "the bar sits on the caret mark after the chosen piece: " + nineKeyTitle);
        String plainTitle = PhrasePreeditPolicy.title("", CompositionCaretPolicy.withMark(typed, mark), false);
        check(plainTitle.charAt(CompositionCaretPolicy.markInTitle("", mark, false)) == CompositionCaretPolicy.CARET_MARK,
            "the bar sits on the caret mark without a chosen piece");
        check(CompositionCaretPolicy.markInTitle(null, mark, false) == mark, "a missing prefix adds nothing");
        check(CompositionCaretPolicy.markInTitle("饭了", mark, true) == mark, "a local mode title has no prefix");
        check(CompositionCaretPolicy.markInTitle("饭了", -1, false) == -1, "no caret drawn, no bar");
        // 竖条只换画法，`|` 仍占一个字符：点在竖条左边或右边，换算出的都是光标现在的位置，光标不动。
        check(CompositionCaretPolicy.tapTarget(2, "xi'an", "9426", nineKeyMark, barIndex) == 2, "a tap just before the bar keeps the caret");
        check(CompositionCaretPolicy.tapTarget(2, "xi'an", "9426", nineKeyMark, barIndex + 1) == 2, "a tap just after the bar keeps the caret");
        check(CompositionCaretPolicy.tapTarget(2, "xi'an", "9426", nineKeyMark, barIndex + 2) == 3, "a tap one letter past the bar moves one key");

        // 发给引擎的命令：两头各一条，其余逐格。
        moves(19, 0, 19, CompositionCaretPolicy.MOVE_HOME, 1);
        moves(3, 19, 19, CompositionCaretPolicy.MOVE_END, 1);
        moves(19, 9, 19, CompositionCaretPolicy.MOVE_LEFT, 10);
        moves(2, 5, 19, CompositionCaretPolicy.MOVE_RIGHT, 3);
        moves(5, 5, 19);
        moves(5, -1, 19);
        moves(5, 20, 19);
        System.out.println("Android composition caret: taps map to the spelling's keys, the caret bar sits on the mark passed");
    }
}

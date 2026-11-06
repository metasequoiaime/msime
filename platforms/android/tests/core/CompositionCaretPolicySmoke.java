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

    // 宿主按 View 的 scheme、key_grid、grid_composing 取的组码表，用例都经同一个入口取，不直接写表。
    static final String NINE = CompositionCaretPolicy.keypad(InputSchemeTraits.QUANPIN, "nine_key", true);
    static final String FOURTEEN = CompositionCaretPolicy.keypad(InputSchemeTraits.QUANPIN, "fourteen_key", true);
    static final String EIGHTEEN = CompositionCaretPolicy.keypad(InputSchemeTraits.QUANPIN, "eighteen_key", true);

    public static void main(String[] args) {
        // 输入框的组字内选择可以包含显示用分隔符；跨出组字、范围改变与选中文字都不接管。
        check(CompositionCaretPolicy.editorTapTarget("ni'hao", "64426", NINE, 0, 8, 14, 11, 11) == 2,
            "nine-key selection skips the displayed separator");
        check(CompositionCaretPolicy.editorTapTarget("合成nihao", "nihao", null, 2, 8, 15, 12, 12) == 2,
            "chosen prefix does not count as key letters");
        check(CompositionCaretPolicy.editorTapTarget("nihao", "nihao", null, 0, 8, 13, 8, 8) == 0,
            "composition start remains editable");
        check(CompositionCaretPolicy.editorTapTarget("nihao", "nihao", null, 0, 8, 13, 13, 13) == 5,
            "composition end remains editable");
        for (int[] selection : new int[][] {{7, 7}, {14, 14}, {9, 11}}) {
            check(CompositionCaretPolicy.editorTapTarget("nihao", "nihao", null, 0, 8, 13,
                selection[0], selection[1]) == -1, "outside or range selection is not a caret tap");
        }
        check(CompositionCaretPolicy.editorTapTarget("nihao", "nihao", null, 0, -1, -1, 2, 2) == -1,
            "missing composition markers");
        check(CompositionCaretPolicy.editorTapTarget("nihao", "nihao", null, 0, 8, 12, 10, 10) == -1,
            "truncated editor composition");
        check(CompositionCaretPolicy.editorTapTarget("nihao", "9426", NINE, 0, 8, 13, 10, 10) == -1,
            "changed spelling must not revive a composition");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "64426", NINE, 5, true) == 6,
            "magnified caret can be drawn at the end");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "64426", NINE, 2, true) == 3,
            "magnified caret follows a display separator");
        // 能点的方案：光标留在组字里的那些。韩语、注音、越南语、藏文锁住光标，日语和笔画画的是 reading，本地模式和专用英文各管各的。
        check(CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "jintianci", false), "quanpin");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.SHUANGPIN, "none", false, "jntm", false), "shuangpin");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.WUBI, "none", false, "wqvb", false), "wubi");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.CANTONESE, "none", false, "nei hou", false), "cantonese");
        for (int scheme : new int[] {InputSchemeTraits.KOREAN, InputSchemeTraits.ZHUYIN,
                InputSchemeTraits.VIETNAMESE, InputSchemeTraits.TIBETAN, InputSchemeTraits.JAPANESE,
                InputSchemeTraits.STROKE, -1}) {
            check(!CompositionCaretPolicy.editable(scheme, "none", false, "abc", false), "scheme " + scheme);
        }
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "emoji", false, "vbq", false), "local mode");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", true, "hello", false), "dedicated English");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "", false), "nothing composed");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "ni好", false), "non-ASCII spelling");
        // #10591：网格里打的拼音照常能编辑：九键的数字、14 键和 18 键的组码字母、九键双拼的数字都和按键一一对应。
        check(CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "64426", true), "quanpin nine-key grid");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "bugao", true), "quanpin 14/18-key grid");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.SHUANGPIN, "none", false, "jntm", true), "shuangpin grid");
        // 五笔 14 键的 editing_text 是每组的代表字母（QW 组记成 q），不是用户打的码，不能编辑，与 iOS 的 typesWubiGrid 一致。
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.WUBI, "none", false, "qq", true), "wubi 14-key grid");
        // 网格开着时硬件键盘打的五笔码走 26 键，grid_composing 为假，照常能编辑。
        check(CompositionCaretPolicy.editable(InputSchemeTraits.WUBI, "none", false, "wqvb", false), "wubi typed on a hardware keyboard");

        // 全拼：读音就是按键。#5613 的例子：jintianciwufanlemei 里把 ci 改成 chi，点在 c 和 i 之间。
        String typed = "jintianciwufanlemei";
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, null, -1, 8) == 8, "tap between c and i");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, null, -1, 99) == typed.length(), "past the end");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, null, -1, 0) == 0, "the start");
        // 光标在 9 处画成 jintianc|iwufanlemei；点光标符后面的 i 之后，换算时去掉光标符。
        int mark = CompositionCaretPolicy.markIndex(typed, typed, null, 8, false);
        check(mark == 8, "caret drawn where it is");
        check("jintianc|iwufanlemei".equals(CompositionCaretPolicy.withMark(typed, mark)), "caret mark drawn");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, null, mark, 10) == 9, "a tap after the mark skips it");
        check(CompositionCaretPolicy.tapTarget(0, typed, typed, null, mark, 8) == 8, "a tap before the mark");
        check(CompositionCaretPolicy.markIndex(typed, typed, null, typed.length(), false) == -1, "a caret at the end is not drawn");

        // #10133：全拼读音带引擎加的分隔（zuo'tian'jin'tian），分隔不在按键里，退格删的是分隔前的字母，光标画在分隔前面。
        String segmented = "zuo'tian'jin'tian";
        String letters = "zuotianjintian";
        int beforeSplit = CompositionCaretPolicy.markIndex(segmented, letters, null, 3, false);
        check(beforeSplit == 3, "letter caret sits before a display separator: " + beforeSplit);
        check("zuo|'tian'jin'tian".equals(CompositionCaretPolicy.withMark(segmented, beforeSplit)), "zuo|'tian");
        check(CompositionCaretPolicy.displayOffset(segmented, letters, null, 3, false) == 3, "magnified letter caret before the separator");
        check(CompositionCaretPolicy.markIndex(segmented, letters, null, 4, false) == 5, "caret after t is zuo't|ian");
        check(CompositionCaretPolicy.markIndex(segmented, letters, null, 2, false) == 2, "caret before o is zu|o'tian");
        check(CompositionCaretPolicy.markIndex(segmented, letters, null, 7, false) == 8, "second separator: zuo'tian|'jin'tian");
        // 逐格左移：zuo't|ian → zuo|'tian → zu|o'tian，每一格都停在一个不同的位置。
        check("zuo't|ian'jin'tian".equals(CompositionCaretPolicy.withMark(segmented,
            CompositionCaretPolicy.displayOffset(segmented, letters, null, 4, false))), "step at t|ian");
        check("zu|o'tian'jin'tian".equals(CompositionCaretPolicy.withMark(segmented,
            CompositionCaretPolicy.displayOffset(segmented, letters, null, 2, false))), "step at zu|o");
        // 画在分隔前面后，点光标符两侧换算出的仍是当前光标，点分隔后面的 t 后移一格。
        check(CompositionCaretPolicy.tapTarget(0, segmented, letters, null, beforeSplit, 3) == 3, "tap before the mark keeps the caret");
        check(CompositionCaretPolicy.tapTarget(0, segmented, letters, null, beforeSplit, 4) == 3, "tap after the mark keeps the caret");
        check(CompositionCaretPolicy.tapTarget(0, segmented, letters, null, beforeSplit, 5) == 3, "tap after the separator keeps the caret");
        check(CompositionCaretPolicy.tapTarget(0, segmented, letters, null, beforeSplit, 6) == 4, "tap after t moves one key");
        // 用户自己打的 ' 在按键里，一一对齐，分隔两侧是两个光标位置。
        check(CompositionCaretPolicy.markIndex("zuo'tian", "zuo'tian", null, 3, false) == 3, "typed apostrophe: before it");
        check(CompositionCaretPolicy.markIndex("zuo'tian", "zuo'tian", null, 4, false) == 4, "typed apostrophe: after it");
        // 粤拼按键里本身就有空格，同样一一对齐。
        check(CompositionCaretPolicy.markIndex("nei hou", "nei hou", null, 4, false) == 4, "typed space is its own key");

        // 已选好的词画在读音前面：点在它上面，光标到开头。
        check(CompositionCaretPolicy.tapTarget(2, "fanlemei", "fanlemei", null, -1, 1) == 0, "tap on the chosen piece");
        check(CompositionCaretPolicy.tapTarget(2, "fanlemei", "fanlemei", null, -1, 5) == 3, "tap after the chosen piece");

        // 九键：读音行画的是拼音（9426 画成 xi'an），按键是数字；字母按键位对上数字，显示用的分隔跳过。
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9426", NINE, -1, 3) == 2, "tap after the separator");
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9426", NINE, -1, 2) == 2, "tap before the separator");
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9426", NINE, -1, 4) == 3, "tap inside the second syllable");
        // 组码网格里同一个光标位置隔着分隔对应两处时画在分隔后面：网格的退格先删光标前的切分（26 键字母按键画在前面，见上面 #10133）。
        check(CompositionCaretPolicy.markIndex("94'26", "9426", NINE, 2, true) == 3, "caret after a split");
        check("94'|26".equals(CompositionCaretPolicy.withMark("94'26", 3)), "split then caret");
        // 14 键、18 键的按键是组码字母（ba 再切分再 ba 是 baba），走的是网格的退格，同样画在切分后面；按 26 键的规则画会成 ba|'ba，退格删的却是切分。
        check(CompositionCaretPolicy.markIndex("ba'ba", "baba", FOURTEEN, 2, true) == 3, "fourteen-key caret after a split");
        check("ba'|ba".equals(CompositionCaretPolicy.withMark("ba'ba", 3)), "fourteen-key split then caret");
        check(CompositionCaretPolicy.displayOffset("ba'ba", "baba", FOURTEEN, 2, true) == 3, "fourteen-key magnified caret after a split");
        check(CompositionCaretPolicy.markIndex("ba'ba", "baba", null, 2, false) == 2, "the same keys on 26 keys sit before the separator");
        // 选过拼音之后：ni'426 对 64426。
        check(CompositionCaretPolicy.tapTarget(0, "ni'426", "64426", NINE, -1, 4) == 3, "locked spelling then digits");
        // 首选是简拼行时读音行是首字母（m't 对 68）。
        check(CompositionCaretPolicy.tapTarget(0, "m't", "68", NINE, -1, 2) == 1, "initials reading");
        // 粤拼的按键里本身就有空格。
        check(CompositionCaretPolicy.tapTarget(0, "nei hou", "nei hou", null, -1, 4) == 4, "a typed space is a key");

        // 14 键、18 键的按键是每组首字母的组码（14 键 nihao 打成 bugao，18 键打成 bihai），读音的字母按这一键的组码对上按键，放大条画读音不退回组码（#10591）。
        check(CompositionCaretPolicy.displayOffset("ni'hao", "bugao", FOURTEEN, 5, true) == 6, "fourteen-key reading aligns with its group codes");
        check(CompositionCaretPolicy.tapTarget(0, "ni'hao", "bugao", FOURTEEN, -1, 1) == 1, "fourteen-key tap after n");
        check(CompositionCaretPolicy.tapTarget(0, "ni'hao", "bugao", FOURTEEN, -1, 3) == 2, "fourteen-key tap before h");
        check(CompositionCaretPolicy.markIndex("ni'hao", "bugao", FOURTEEN, 2, true) == 3, "fourteen-key caret after the split");
        check(CompositionCaretPolicy.editorTapTarget("ni'hao", "bugao", FOURTEEN, 0, 8, 14, 12, 12) == 3,
            "fourteen-key in-field tap maps to the group codes");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "bihai", EIGHTEEN, 2, true) == 3, "eighteen-key reading aligns with its group codes");
        check(CompositionCaretPolicy.tapTarget(0, "ni'hao", "bihai", EIGHTEEN, -1, 6) == 5, "eighteen-key tap at the end");
        // 每张表都与引擎的 KeyGrid::code_of 一致：14 键 QW ER TY UI OP / AS DF GH JK L / ZX CV BN M，18 键 Q WE RT Y U IO P / A SD FG H JK L / Z XC V BN M。
        check(CompositionCaretPolicy.tapTarget(0, "qwertyuiopasdfghjklzxcvbnm", "qqeettuuooaaddggjjlzzccbbm", FOURTEEN, -1, 26) == 26,
            "fourteen-key groups");
        check(CompositionCaretPolicy.tapTarget(0, "qwertyuiopasdfghjklzxcvbnm", "qwwrryuiipassffhjjlzxxvbbm", EIGHTEEN, -1, 26) == 26,
            "eighteen-key groups");
        // 同一个读音换一张表就对不上：14 键的组码不能按 18 键或九键对，反过来也一样。
        check(CompositionCaretPolicy.displayOffset("ni'hao", "bugao", EIGHTEEN, 5, true) == -1, "fourteen-key codes are not eighteen-key codes");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "64426", FOURTEEN, 5, true) == -1, "digits are not fourteen-key codes");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "bugao", NINE, 5, true) == -1, "fourteen-key codes are not digits");
        // 26 键只认同一个字母：n 对不上 b，九键数字也不再当成字母的按键。
        String qwerty = CompositionCaretPolicy.keypad(InputSchemeTraits.QUANPIN, "none", true);
        check(qwerty == null, "26 keys have no group codes");
        check(CompositionCaretPolicy.keypad(InputSchemeTraits.QUANPIN, "fourteen_key", false) == null,
            "hardware letters typed while the grid is on compose on 26 keys");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "bugao", qwerty, 5, false) == -1, "26 keys: n is not b");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "64426", qwerty, 5, false) == -1, "26 keys: letters are not digits");
        check(CompositionCaretPolicy.displayOffset("ni'hao", "nihao", qwerty, 5, false) == 6, "26 keys: the same letters");
        // 双拼网格一个音节两键、按方案编码，没有字母组码表，读音对不上，照旧退回画原始按键。
        for (String grid : new String[] {"nine_key_shuangpin", "fourteen_key", "eighteen_key"}) {
            check(CompositionCaretPolicy.keypad(InputSchemeTraits.SHUANGPIN, grid, true) == null, "shuangpin " + grid);
        }
        // 五笔 14 键的按键是各组的代表字母，不是用户打的码，不能编辑；26 键五笔照常可以。
        check(CompositionCaretPolicy.keypad(InputSchemeTraits.WUBI, "fourteen_key", true) == null, "wubi fourteen-key has no reading keypad");
        check(!CompositionCaretPolicy.editable(InputSchemeTraits.WUBI, "none", false, "qq", true), "wubi fourteen-key");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.QUANPIN, "none", false, "bugao", true), "quanpin fourteen-key");
        check(CompositionCaretPolicy.editable(InputSchemeTraits.SHUANGPIN, "none", false, "bubu", true), "shuangpin grid");

        // 读音对不上按键时不移光标。
        check(CompositionCaretPolicy.tapTarget(0, "xi'an", "9427", NINE, -1, 3) == -1, "a reading of other keys");
        check(CompositionCaretPolicy.tapTarget(0, "xi", "9426", NINE, -1, 1) == -1, "a reading shorter than the keys");
        check(CompositionCaretPolicy.markIndex("xi'an", "9427", NINE, 1, true) == -1, "no mark on a reading of other keys");
        check(CompositionCaretPolicy.tapTarget(0, null, "64", NINE, -1, 1) == -1, "no reading");
        check(CompositionCaretPolicy.tapTarget(0, "ni", "64", NINE, -1, -1) == -1, "no layout offset");

        // #6110：光标符画成竖条，要知道它在整段标题（已选的词 + 读音）里的下标；标题里那个位置必须正是光标符。
        int nineKeyMark = CompositionCaretPolicy.markIndex("xi'an", "9426", NINE, 2, true);
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
        check(CompositionCaretPolicy.tapTarget(2, "xi'an", "9426", NINE, nineKeyMark, barIndex) == 2, "a tap just before the bar keeps the caret");
        check(CompositionCaretPolicy.tapTarget(2, "xi'an", "9426", NINE, nineKeyMark, barIndex + 1) == 2, "a tap just after the bar keeps the caret");
        check(CompositionCaretPolicy.tapTarget(2, "xi'an", "9426", NINE, nineKeyMark, barIndex + 2) == 3, "a tap one letter past the bar moves one key");

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

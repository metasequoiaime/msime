package app.msime.android;

import java.util.List;
import java.util.Map;

/** 九键左侧符号栏：出厂符号、数字层与其他键面的行高、共享偏好里的两张表（空表回到出厂表、自定义原样、右侧大回车只给出厂表接 ！）、编辑器的整表校验与写法，以及只剩迁移读的本机旧键。 */
public final class NineKeySidebarPolicySmoke {
    public static void main(String[] args) {
        defaults();
        digitDefaults();
        layerChoice();
        tallReturn();
        rowHeight();
        parsing();
        sharedPreference();
        editor();
        choices();
        reorder();
        localSetting();
        System.out.println("Android nine-key sidebar: defaults, layer row heights, shared tables, editor validation, symbol choices, drag reorder and ordinals, custom symbol parsing and the legacy local key passed");
    }

    /** 哪个键面用哪张表：数字键面换成算式符号那张，字母键面不变；空表回到各自的出厂表。 */
    private static void layerChoice() {
        List<String> letters = List.of("，", "。");
        List<String> digits = List.of("+", "-");
        check(NineKeySidebarPolicy.sidebarSymbols(true, letters, digits).equals(digits), "the digit layer reads the digit table");
        check(NineKeySidebarPolicy.sidebarSymbols(false, letters, digits).equals(letters), "the letter layer reads the letter table");
        check(NineKeySidebarPolicy.sidebarSymbols(true, List.of(), List.of()).equals(NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS),
            "an empty digit table falls back to the digit defaults");
        check(NineKeySidebarPolicy.sidebarSymbols(false, List.of(), List.of()).equals(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS),
            "an empty letter table falls back to the letter defaults");
    }

    /** 右侧大回车去掉右列的 ！，它只接在字母键面出厂表的末尾：空表（和别处写下的与出厂表相同的表）接，自定义的表原样使用、不接；数字键面的表不变，其他样式不受影响。 */
    private static void tallReturn() {
        List<String> tall = List.of("，", "。", "？", "！");
        List<String> none = List.of();
        check(NineKeySidebarPolicy.TALL_RETURN_LETTER_SYMBOLS.equals(tall), "！ is appended to the default letter table");
        check(NineKeySidebarPolicy.letterSymbols(none, true).equals(tall), "an empty table reads as the tall-return defaults");
        check(NineKeySidebarPolicy.letterSymbols(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS, true).equals(tall),
            "a table equal to the defaults still gets the ！");
        check(NineKeySidebarPolicy.letterSymbols(List.of("+", "-"), true).equals(List.of("+", "-")), "a custom table is used as it is");
        check(NineKeySidebarPolicy.letterSymbols(List.of("，", "。"), true).equals(List.of("，", "。")), "a shortened custom table stays short");
        check(NineKeySidebarPolicy.letterSymbols(none, false).equals(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS),
            "other styles keep the three-symbol default");
        check(NineKeySidebarPolicy.sidebarSymbols(true, none, none, true).equals(NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS),
            "the digit table already ends its punctuation with ！ and does not change");
        check(NineKeySidebarPolicy.sidebarSymbols(false, none, none, true).equals(tall), "the letter layer reads the tall table");
        check(tall.size() <= NineKeySidebarPolicy.VISIBLE_ROWS, "the four symbols fit one screen");
        // 接上去的 ！ 接替右列 ！ 上滑输入 0、长按上下标 0 的手势：只有它，数字键面、别的样式和自定义表里的 ！ 都不带。
        check(NineKeySidebarPolicy.appendedExclamationIndex(false, none, true) == 3, "the appended ！ is the fourth entry");
        check(NineKeySidebarPolicy.appendedExclamationIndex(false, NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS, true) == 3,
            "a table equal to the defaults still appends ！");
        check(tall.get(NineKeySidebarPolicy.appendedExclamationIndex(false, none, true)).equals("！"), "the index points at ！");
        check(NineKeySidebarPolicy.appendedExclamationIndex(false, none, false) == -1, "other styles append nothing");
        check(NineKeySidebarPolicy.appendedExclamationIndex(true, none, true) == -1, "the digit layer appends nothing");
        check(NineKeySidebarPolicy.appendedExclamationIndex(false, tall, true) == -1, "a custom ！ stays a plain symbol");
        check(NineKeySidebarPolicy.appendedExclamationIndex(false, List.of("+", "-"), true) == -1, "a custom table appends nothing");
    }

    private static void defaults() {
        List<String> letters = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        check(letters.equals(List.of("，", "。", "？")), "the default rail has only three common symbols");
        check(!letters.contains("！"), "！ stays in the right column and is not repeated in the rail");
        check(letters.size() <= NineKeySidebarPolicy.VISIBLE_ROWS, "the default rail fits without scrolling");
        check(NineKeySidebarPolicy.VISIBLE_ROWS == 5, "five symbols per screen instead of three");
        check(NineKeySidebarPolicy.letterSymbols(List.of()).equals(letters), "an empty table reads as the defaults");
    }

    /** #5590：数字键面的左栏先给四则运算符号，叹号挪到后面，滚动才看得到。 */
    private static void digitDefaults() {
        List<String> digits = NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS;
        check(digits.subList(0, 4).equals(List.of("+", "-", "*", "/")), "the digit rail opens with the four operators");
        check(digits.indexOf("！") >= NineKeySidebarPolicy.VISIBLE_ROWS, "！ moved into the rail below the first screen");
        check(!digits.contains("."), "the decimal point has its own key in the right column");
        check(digits.size() == digits.stream().distinct().count(), "no symbol is repeated");
        check(NineKeySidebarPolicy.digitSymbols(List.of()).equals(digits), "an empty table reads as the digit defaults");
        check(List.of("+", "-").equals(NineKeySidebarPolicy.digitSymbols(List.of("+", "-"))), "a custom digit table reads back");
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS);
        check(spec.kind == AndroidLocalSettings.Spec.Kind.TEXT && !spec.synced
            && NineKeySidebarPolicy.format(digits).equals(spec.defaultValue), "the legacy digit key is still a local text setting");
    }

    private static void rowHeight() {
        int digitRows = NineKeySidebarPolicy.DIGIT_VISIBLE_ROWS;
        check(NineKeySidebarPolicy.rowHeight(300, 14, 28, digitRows) == 60, "the digit rail shows five symbols before scrolling");
        check(NineKeySidebarPolicy.rowHeight(300, 2, 28, digitRows) == 150, "a short custom digit table fills the rail");
        check(NineKeySidebarPolicy.rowHeight(60, 14, 28, digitRows) == 30, "a compact digit rail respects the minimum height");
        check(NineKeySidebarPolicy.rowHeight(500, 9, 28) == 100, "five rows share a full rail");
        check(NineKeySidebarPolicy.rowHeight(300, 3, 28) == 100, "three symbols fill the rail as before");
        check(NineKeySidebarPolicy.rowHeight(300, 1, 28) == 300, "a single symbol fills the rail");
        check(NineKeySidebarPolicy.rowHeight(300, 0, 28) == 300, "an empty rail does not divide by zero");
        check(NineKeySidebarPolicy.rowHeight(0, 9, 28) == 0, "an unmeasured rail has no rows");
        check(NineKeySidebarPolicy.rowHeight(120, 9, 28) == 30, "a short keyboard shows four rows rather than squeezing five");
        check(NineKeySidebarPolicy.rowHeight(20, 9, 28) == 20, "a rail shorter than one row still shows one");
        check(NineKeySidebarPolicy.rowHeight(500, 9, 0) == 100, "no minimum means five rows");
    }

    private static void parsing() {
        check(List.of("，", "。", "……", ":-)").equals(NineKeySidebarPolicy.parse(" ，  。\t……　:-) ")),
            "any whitespace, including the ideographic space, separates symbols");
        check(List.of("a", "b").equals(NineKeySidebarPolicy.parse("a b a")), "duplicates keep their first position");
        check(NineKeySidebarPolicy.parse("") == null, "an empty table is not a table");
        check(NineKeySidebarPolicy.parse("abcde") == null, "a symbol longer than four characters is refused");
        check(List.of("😀😀").equals(NineKeySidebarPolicy.parse("😀😀")), "length counts code points, not UTF-16 units");
        check(NineKeySidebarPolicy.parse("a\u0007b") == null, "control characters are refused");
        StringBuilder many = new StringBuilder();
        for (int index = 0; index <= NineKeySidebarPolicy.MAX_SYMBOLS; index++) many.append(index).append(' ');
        check(NineKeySidebarPolicy.parse(many.toString()) == null, "more than thirty symbols is refused");
        check(NineKeySidebarPolicy.parse("x".repeat(NineKeySidebarPolicy.MAX_TEXT_LENGTH + 1)) == null, "oversized text is refused");
        check("， 。 ？".equals(NineKeySidebarPolicy.normalize("，  。 ？ ，")), "storage form is single-space separated");
        check(NineKeySidebarPolicy.normalize("toolong") == null, "an invalid table normalises to null");
        check("a b".equals(NineKeySidebarPolicy.summary(List.of("a", "b"), 4)), "a short table is listed whole");
        check("a b 等 3 个".equals(NineKeySidebarPolicy.summary(List.of("a", "b", "c"), 2)), "a long table is summarised");
    }

    /** 共享偏好 `touch_nine_key_sidebar_symbols` / `touch_nine_key_digit_sidebar_symbols`：JSON 字符串数组，只留字符串元素；空表是出厂表；自定义与否按出厂表比。 */
    private static void sharedPreference() {
        check(NineKeySidebarPolicy.LETTER_PREFERENCE.equals("touch_nine_key_sidebar_symbols")
            && NineKeySidebarPolicy.DIGIT_PREFERENCE.equals("touch_nine_key_digit_sidebar_symbols"), "shared preference names");
        check(NineKeySidebarPolicy.EDIT_ARGUMENT.equals("edit_sidebar_key"), "the long-press page argument keeps its name");
        check(NineKeySidebarPolicy.stored(java.util.Arrays.asList("、", 1, null, true, "……")).equals(List.of("、", "……")),
            "non-string elements are dropped");
        check(NineKeySidebarPolicy.stored(List.of()).isEmpty(), "an empty array is the factory table");
        check(NineKeySidebarPolicy.fromPreference(null).isEmpty() && NineKeySidebarPolicy.fromPreference("， 。").isEmpty(),
            "a missing key or a value that is not an array reads as the factory table");
        List<String> letters = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        check(!NineKeySidebarPolicy.custom(List.of(), letters), "an empty table is not custom");
        check(!NineKeySidebarPolicy.custom(letters, letters), "a table equal to the defaults is not custom");
        check(NineKeySidebarPolicy.custom(List.of("？", "。", "，"), letters), "a reordered table is custom");
        check(NineKeySidebarPolicy.symbols(List.of(), letters) == letters, "an empty table shows the defaults");
        // 与 client-core 的 NINE_KEY_SIDEBAR_MAX_SYMBOLS、NINE_KEY_SIDEBAR_MAX_SYMBOL_CODE_POINTS、NINE_KEY_SIDEBAR_MAX_TEXT_UTF16 相同。
        check(NineKeySidebarPolicy.MAX_SYMBOLS == 30 && NineKeySidebarPolicy.MAX_SYMBOL_CODE_POINTS == 4
            && NineKeySidebarPolicy.MAX_TEXT_LENGTH == 200, "limits match client-core");
    }

    /** 编辑器：整表校验决定保存可不可点，手动输入一项先过单项校验，确认时等于出厂表写空数组。 */
    private static void editor() {
        List<String> letters = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        check(NineKeySidebarPolicy.valid(letters) && NineKeySidebarPolicy.valid(NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS),
            "both factory tables are valid");
        check(!NineKeySidebarPolicy.valid(List.of()), "an empty table cannot be saved");
        check(!NineKeySidebarPolicy.valid(List.of("a", "a")), "duplicates cannot be saved");
        check(!NineKeySidebarPolicy.valid(List.of("a b")), "an item with a space cannot be saved");
        check(!NineKeySidebarPolicy.valid(List.of("a\u3000b")) && !NineKeySidebarPolicy.valid(List.of("a\u00a0b")),
            "ideographic and no-break spaces are spaces too");
        check(!NineKeySidebarPolicy.valid(List.of("abcde")), "an item over four code points cannot be saved");
        check(NineKeySidebarPolicy.valid(List.of("👍🏽", "……", ":-)", "#+=")), "short strings, emoji with skin tone and #+= are fine");
        check(!NineKeySidebarPolicy.valid(List.of("👨‍👩‍👧")), "a five-code-point family emoji is refused");
        check(!NineKeySidebarPolicy.valid(List.of("a\u0007")), "control characters cannot be saved");
        List<String> thirty = new java.util.ArrayList<>();
        for (int index = 0; index < NineKeySidebarPolicy.MAX_SYMBOLS; index++) thirty.add(String.valueOf(index));
        check(NineKeySidebarPolicy.valid(thirty), "thirty symbols are allowed");
        List<String> more = new java.util.ArrayList<>(thirty);
        more.add("x");
        check(!NineKeySidebarPolicy.valid(more), "a thirty-first symbol is refused");
        // 每项 4 个码位、7 个 UTF-16 单元，26 项用空格连起来是 26 × 7 + 25 = 207，超过 200。
        List<String> wide = new java.util.ArrayList<>();
        for (int index = 0; index < 26; index++) wide.add("👍🏽" + (char) ('a' + index) + "🏽");
        check(NineKeySidebarPolicy.format(wide).length() > NineKeySidebarPolicy.MAX_TEXT_LENGTH
            && !NineKeySidebarPolicy.valid(wide), "a table longer than 200 UTF-16 units is refused");
        check(NineKeySidebarPolicy.addable(letters, " 、 ").equals("、"), "a typed symbol is trimmed");
        check(NineKeySidebarPolicy.addable(letters, "，") == null, "a duplicate cannot be added");
        check(NineKeySidebarPolicy.addable(letters, "a b") == null && NineKeySidebarPolicy.addable(letters, "") == null
            && NineKeySidebarPolicy.addable(letters, null) == null, "only one symbol at a time");
        check(NineKeySidebarPolicy.addable(thirty, "x") == null, "nothing is added to a full table");
        check(NineKeySidebarPolicy.toPreference(letters, letters).isEmpty(), "the factory table is written as an empty array");
        check(NineKeySidebarPolicy.toPreference(List.of("，", "。", "？", "！"), letters).equals(List.of("，", "。", "？", "！")),
            "a custom table is written as it is");
        check(NineKeySidebarPolicy.toPreference(List.of("？", "。", "，"), letters).size() == 3, "order matters for 恢复默认");
    }

    private static void choices() {
        check(NineKeySidebarPolicy.addableSymbols(List.of()).containsAll(List.of("()", "（）")),
            "the editor offers both complete parentheses pairs");
        check(NineKeySidebarPolicy.addableSymbols(List.of("，", "……")).get(0).equals("。"),
            "symbol choices keep their stable order and omit selected values");
        check(NineKeySidebarPolicy.addableSymbols(NineKeySidebarPolicy.ADDABLE_SYMBOLS).isEmpty(),
            "all fixed choices can be exhausted");
        check(NineKeySidebarPolicy.ADDABLE_SYMBOLS.stream().allMatch(symbol ->
            NineKeySidebarPolicy.parse(symbol) != null && NineKeySidebarPolicy.parse(symbol).size() == 1),
            "every fixed choice is a valid single symbol");
    }

    /** #10396：拖动排序是挪动不是交换，拖动途中每一行的位次（序号）实时跟着变，落点按其余各行的布局中线算、不看被拖行自己。 */
    private static void reorder() {
        List<String> abcd = List.of("a", "b", "c", "d");
        // 原来的 Collections.swap 一次越过两行时把 [a,b,c,d] 变成 [c,b,a,d]，画面却是 [b,c,a,d]。
        check(NineKeySidebarPolicy.moved(abcd, 0, 2).equals(List.of("b", "c", "a", "d")), "dragging a down two rows moves it");
        check(NineKeySidebarPolicy.moved(abcd, 3, 0).equals(List.of("d", "a", "b", "c")), "dragging d to the top moves it");
        check(NineKeySidebarPolicy.moved(abcd, 1, 2).equals(List.of("a", "c", "b", "d")), "a one-row move swaps neighbours");
        check(NineKeySidebarPolicy.moved(abcd, 2, 2).equals(abcd), "dropping in place changes nothing");
        check(NineKeySidebarPolicy.moved(abcd, 0, 4).equals(abcd) && NineKeySidebarPolicy.moved(abcd, -1, 0).equals(abcd),
            "out-of-range indexes leave the table alone");
        // 拖动途中每一行的位次：被拖行在 to，中间的行让一格，按位次画出的就是挪完的表。
        for (int from = 0; from < abcd.size(); from++) {
            for (int to = 0; to < abcd.size(); to++) {
                String[] shown = new String[abcd.size()];
                for (int index = 0; index < abcd.size(); index++) {
                    shown[NineKeySidebarPolicy.dragSlot(index, from, to)] = abcd.get(index);
                }
                check(List.of(shown).equals(NineKeySidebarPolicy.moved(abcd, from, to)),
                    "slots during a drag from " + from + " to " + to + " match the dropped order");
            }
        }
        check(NineKeySidebarPolicy.dragSlot(0, 0, 2) == 2 && NineKeySidebarPolicy.dragSlot(1, 0, 2) == 0
            && NineKeySidebarPolicy.dragSlot(2, 0, 2) == 1 && NineKeySidebarPolicy.dragSlot(3, 0, 2) == 3,
            "rows between the two ends make way by one");
        // 每行 40 高：中线 20、60、100、140。
        float[] centers = {20, 60, 100, 140};
        check(NineKeySidebarPolicy.dragTarget(20, 0, centers) == 0, "a row that has not moved stays put");
        check(NineKeySidebarPolicy.dragTarget(59, 0, centers) == 0, "short of the next row's midline it stays");
        check(NineKeySidebarPolicy.dragTarget(61, 0, centers) == 1, "past the next row's midline it takes that place");
        check(NineKeySidebarPolicy.dragTarget(101, 0, centers) == 2, "past two midlines it moves two places");
        check(NineKeySidebarPolicy.dragTarget(500, 0, centers) == 3, "far below the list it goes last");
        check(NineKeySidebarPolicy.dragTarget(-50, 3, centers) == 0, "far above the list it goes first");
        check(NineKeySidebarPolicy.dragTarget(99, 3, centers) == 2, "dragging up past one midline");
        check(NineKeySidebarPolicy.dragTarget(141, 1, centers) == 3, "the dragged row's own midline is never counted");
        // 被拖行夹在列表里，拖到底或顶时中线正好压在首末行的中线上，也要算到那一格。
        check(NineKeySidebarPolicy.dragTarget(140, 0, centers) == 3, "pinned at the bottom it reaches the last place");
        check(NineKeySidebarPolicy.dragTarget(20, 3, centers) == 0, "pinned at the top it reaches the first place");
        check(NineKeySidebarPolicy.dragTarget(60, 0, centers) == 1 && NineKeySidebarPolicy.dragTarget(60, 3, centers) == 1,
            "touching a midline counts from either side");
        // 序号与读屏描述：1..N，第 30 项也是「30.」。
        check(NineKeySidebarPolicy.ordinal(0).equals("1.") && NineKeySidebarPolicy.ordinal(29).equals("30."),
            "ordinals count from one");
        check(NineKeySidebarPolicy.ordinalDescription(2, "？").equals("第 3 项，符号：？"), "TalkBack reads the position once");
        check(NineKeySidebarPolicy.positionDescription(0).equals("第 1 项"), "the handle's state names its position");
        // 滚动区 100..700，边缘带 50，每帧最多 20。
        check(NineKeySidebarPolicy.edgeScrollStep(400, 100, 700, 50, 20) == 0, "the middle does not scroll");
        check(NineKeySidebarPolicy.edgeScrollStep(150, 100, 700, 50, 20) == 0
            && NineKeySidebarPolicy.edgeScrollStep(650, 100, 700, 50, 20) == 0, "the band starts inside the edge");
        check(NineKeySidebarPolicy.edgeScrollStep(125, 100, 700, 50, 20) == -10, "half into the top band scrolls up at half speed");
        check(NineKeySidebarPolicy.edgeScrollStep(690, 100, 700, 50, 20) == 16, "deep in the bottom band scrolls down faster");
        check(NineKeySidebarPolicy.edgeScrollStep(40, 100, 700, 50, 20) == -20
            && NineKeySidebarPolicy.edgeScrollStep(900, 100, 700, 50, 20) == 20, "past the edge is the top speed");
        check(NineKeySidebarPolicy.edgeScrollStep(149.9f, 100, 700, 50, 20) == -1, "just inside the band still moves");
        check(NineKeySidebarPolicy.edgeScrollStep(120, 100, 180, 50, 20) == 0, "a viewport too short for two bands never auto-scrolls");
        // 被拖行夹在列表首尾后不再朝那一头滚，另一头照滚；还没到头时照常滚。
        check(NineKeySidebarPolicy.scrollTowardRoom(16, 120, -40, 120) == 0, "pinned at the bottom of the list it stops scrolling down");
        check(NineKeySidebarPolicy.scrollTowardRoom(-16, -40, -40, 120) == 0, "pinned at the top of the list it stops scrolling up");
        check(NineKeySidebarPolicy.scrollTowardRoom(-16, 120, -40, 120) == -16
            && NineKeySidebarPolicy.scrollTowardRoom(16, -40, -40, 120) == 16, "the other direction still scrolls");
        check(NineKeySidebarPolicy.scrollTowardRoom(16, 80, -40, 120) == 16, "with room left it keeps scrolling");
        check(NineKeySidebarPolicy.scrollTowardRoom(0, 120, -40, 120) == 0, "no step stays no step");
    }

    /** 本机旧键只留给迁移读：声明和规范化不变，读出来的值经 parse 交给迁移。 */
    private static void localSetting() {
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_SYMBOLS);
        check(spec.kind == AndroidLocalSettings.Spec.Kind.TEXT && !spec.synced, "a local-only text setting");
        check(NineKeySidebarPolicy.format(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS).equals(spec.defaultValue),
            "the default is the default table");
        check("@ #".equals(spec.accept(" @   # ")), "accepted values are normalised");
        check(spec.accept("toolong") == null && spec.accept(1) == null && spec.accept(true) == null,
            "invalid tables and non-text values are refused");
        AndroidLocalSettings.Snapshot custom = new AndroidLocalSettings.Snapshot(
            AndroidLocalSettings.accepted(Map.of(AndroidLocalSettings.NINE_KEY_SYMBOLS, "+ -")));
        check(List.of("+", "-").equals(NineKeySidebarPolicy.parse(custom.text(AndroidLocalSettings.NINE_KEY_SYMBOLS))),
            "a stored legacy table reads back");
        check(!custom.synced().containsKey(AndroidLocalSettings.NINE_KEY_SYMBOLS), "the table never syncs");
        boolean threw = false;
        try { custom.bool(AndroidLocalSettings.NINE_KEY_SYMBOLS); } catch (IllegalArgumentException expected) { threw = true; }
        check(threw, "reading a text setting as a boolean is a programming error");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

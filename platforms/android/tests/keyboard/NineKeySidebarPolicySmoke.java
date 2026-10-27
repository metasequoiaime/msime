package app.msime.android;

import java.util.List;
import java.util.Map;

/** 九键左侧符号栏：默认符号、数字四行与其他键面五行的行高、自定义符号表，以及本地设置。 */
public final class NineKeySidebarPolicySmoke {
    public static void main(String[] args) {
        defaults();
        digitDefaults();
        layerChoice();
        tallReturn();
        rowHeight();
        parsing();
        choices();
        localSetting();
        System.out.println("Android nine-key sidebar: defaults, layer row heights, symbol choices, custom symbol parsing and the local setting passed");
    }

    /** 哪个键面用哪张表：数字键面换成算式符号那张，字母键面不变。 */
    private static void layerChoice() {
        check(NineKeySidebarPolicy.sidebarSymbols(true, "， 。", "+ -").equals(List.of("+", "-")), "the digit layer reads the digit table");
        check(NineKeySidebarPolicy.sidebarSymbols(false, "， 。", "+ -").equals(List.of("，", "。")), "the letter layer reads the letter table");
        check(NineKeySidebarPolicy.sidebarSymbols(true, null, null).equals(NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS), "the digit layer falls back to the digit defaults");
    }

    /** 右侧大回车去掉右列的 ！，它接在字母键面默认表的末尾；自定义的表（与默认表相同的除外，设置页「恢复默认」会把默认表原样写进存储）和数字键面的表不变，其他样式不受影响。 */
    private static void tallReturn() {
        List<String> tall = List.of("，", "。", "？", "！");
        check(NineKeySidebarPolicy.TALL_RETURN_LETTER_SYMBOLS.equals(tall), "！ is appended to the default letter table");
        check(NineKeySidebarPolicy.letterSymbols(null, true).equals(tall), "nothing stored reads as the tall-return defaults");
        check(NineKeySidebarPolicy.letterSymbols(NineKeySidebarPolicy.format(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS), true)
            .equals(tall), "the default table written back by 恢复默认 still gets the ！");
        check(NineKeySidebarPolicy.letterSymbols("+ -", true).equals(List.of("+", "-")), "a custom table is used as it is");
        check(NineKeySidebarPolicy.letterSymbols("， 。", true).equals(List.of("，", "。")), "a shortened custom table stays short");
        check(NineKeySidebarPolicy.letterSymbols(null, false).equals(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS),
            "other styles keep the three-symbol default");
        check(NineKeySidebarPolicy.sidebarSymbols(true, null, null, true).equals(NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS),
            "the digit table already ends its punctuation with ！ and does not change");
        check(NineKeySidebarPolicy.sidebarSymbols(false, null, null, true).equals(tall), "the letter layer reads the tall table");
        check(tall.size() <= NineKeySidebarPolicy.VISIBLE_ROWS, "the four symbols fit one screen");
    }

    private static void defaults() {
        List<String> letters = NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS;
        check(letters.equals(List.of("，", "。", "？")), "the default rail has only three common symbols");
        check(!letters.contains("！"), "！ stays in the right column and is not repeated in the rail");
        check(letters.size() <= NineKeySidebarPolicy.VISIBLE_ROWS, "the default rail fits without scrolling");
        check(NineKeySidebarPolicy.VISIBLE_ROWS == 5, "five symbols per screen instead of three");
        check(NineKeySidebarPolicy.letterSymbols(null).equals(letters), "nothing stored reads as the defaults");
        check(NineKeySidebarPolicy.letterSymbols("   ").equals(letters), "blank text reads as the defaults");
    }

    /** #5590：数字键面的左栏先给四则运算符号，叹号挪到后面，滚动才看得到。 */
    private static void digitDefaults() {
        List<String> digits = NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS;
        check(digits.subList(0, 4).equals(List.of("+", "-", "*", "/")), "the digit rail opens with the four operators");
        check(digits.indexOf("！") >= NineKeySidebarPolicy.VISIBLE_ROWS, "！ moved into the rail below the first screen");
        check(!digits.contains("."), "the decimal point has its own key in the right column");
        check(digits.size() == digits.stream().distinct().count(), "no symbol is repeated");
        check(NineKeySidebarPolicy.digitSymbols(null).equals(digits), "nothing stored reads as the digit defaults");
        check(List.of("+", "-").equals(NineKeySidebarPolicy.digitSymbols("+ -")), "a custom digit table reads back");
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS);
        check(spec.kind == AndroidLocalSettings.Spec.Kind.TEXT && !spec.synced
            && NineKeySidebarPolicy.format(digits).equals(spec.defaultValue), "the digit table is a local text setting");
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
        check(NineKeySidebarPolicy.letterSymbols("toolong").equals(NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS),
            "an invalid stored table falls back to the defaults");
        check("a b".equals(NineKeySidebarPolicy.summary(List.of("a", "b"), 4)), "a short table is listed whole");
        check("a b 等 3 个".equals(NineKeySidebarPolicy.summary(List.of("a", "b", "c"), 2)), "a long table is summarised");
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
        check(List.of("+", "-").equals(NineKeySidebarPolicy.letterSymbols(custom.text(AndroidLocalSettings.NINE_KEY_SYMBOLS))),
            "a stored table reads back");
        check(!custom.synced().containsKey(AndroidLocalSettings.NINE_KEY_SYMBOLS), "the table never syncs");
        boolean threw = false;
        try { custom.bool(AndroidLocalSettings.NINE_KEY_SYMBOLS); } catch (IllegalArgumentException expected) { threw = true; }
        check(threw, "reading a text setting as a boolean is a programming error");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

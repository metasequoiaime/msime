import app.msime.android.PairedPunctuationPolicy;

public final class PairedPunctuationPolicySmoke {
    public static void main(String[] args) {
        engineCompletions();
        quoteReopening();
        symbolPanelPairs();
        stepOver();
        symbolStepOver();
        caretAfterClosingCommit();
        System.out.println("PairedPunctuationPolicySmoke: PASS");
    }

    private static void engineCompletions() {
        check(PairedPunctuationPolicy.completion("（", true).closing().equals("）"), "（ closes");
        check(PairedPunctuationPolicy.completion("你好【", true).closing().equals("】"), "a commit ending in an opening closes");
        PairedPunctuationPolicy.Completion book = PairedPunctuationPolicy.completion("《", true);
        check(book.closing().equals("》") && book.opening() == '<', "《 closes and balances <");
        check(PairedPunctuationPolicy.completion("〈", true).closing().equals("〉"), "nested 〈 closes");
        check(PairedPunctuationPolicy.completion("“", true).closing().equals("”"), "“ closes");
        check(PairedPunctuationPolicy.completion("‘", true).closing().equals("’"), "‘ closes");
        check(PairedPunctuationPolicy.completion("（", false) == null, "switch off: nothing is added");
        check(PairedPunctuationPolicy.completion("）", true) == null, "a closing mark is not completed");
        check(PairedPunctuationPolicy.completion("，", true) == null, "plain punctuation");
        check(PairedPunctuationPolicy.completion("", true) == null && PairedPunctuationPolicy.completion(null, true) == null,
            "empty commit");
    }

    private static void quoteReopening() {
        check(PairedPunctuationPolicy.reopenQuote("”", '"', true).equals("“"), "every \" press opens a pair");
        check(PairedPunctuationPolicy.reopenQuote("’", '\'', true).equals("‘"), "every ' press opens a pair");
        check(PairedPunctuationPolicy.reopenQuote("”", '"', false).equals("”"), "switch off keeps the alternation");
        check(PairedPunctuationPolicy.reopenQuote("）", ')', true).equals("）"), "other keys untouched");
        check(PairedPunctuationPolicy.reopenQuote(null, '"', true) == null, "no commit");
    }

    private static void symbolPanelPairs() {
        // issue 截图里划线的那几行，中文和英文括号都成对。
        String[][] pairs = {
            {"〈", "〉"}, {"「", "」"}, {"『", "』"}, {"〔", "〕"}, {"〖", "〗"},
            {"＜", "＞"}, {"｛", "｝"}, {"［", "］"}, {"（", "）"}, {"《", "》"}, {"【", "】"},
            {"“", "”"}, {"‘", "’"}, {"(", ")"}, {"[", "]"}, {"{", "}"}};
        for (String[] pair : pairs)
            check(pair[1].equals(PairedPunctuationPolicy.symbolClosing(pair[0])), pair[0] + " pairs with " + pair[1]);
        // ASCII 的 < 绝大多数时候是小于号，点一下补成 <> 打 a < b 时还得删；全角 ＜ 和书名号照常成对。
        check(PairedPunctuationPolicy.symbolClosing("<") == null, "ASCII < is inserted alone");
        check("＞".equals(PairedPunctuationPolicy.symbolClosing("＜")) && "》".equals(PairedPunctuationPolicy.symbolClosing("《")),
            "fullwidth ＜ and 《 still pair");
        for (String single : new String[] {"）", "”", "\"", "'", ">", "，", "︵", "﹁", "http://", null})
            check(PairedPunctuationPolicy.symbolClosing(single) == null, single + " is inserted alone");
    }

    private static void stepOver() {
        PairedPunctuationPolicy.Stack stack = new PairedPunctuationPolicy.Stack();
        check(stack.stepOver(')', 1, "）") == null, "nothing to step over");
        stack.push("）", 1);
        check("）".equals(stack.stepOver(')', 1, "）")), ") steps over the closing half");
        check(stack.isEmpty(), "popped");

        // 嵌套时只跨最里层；不收后半个的键不动记录。
        stack.push("》", 1);
        stack.push("”", 1);
        check(stack.stepOver(',', 1, "”》") == null && !stack.isEmpty(), "a plain key keeps the record");
        check("”".equals(stack.stepOver('"', 1, "”》")), "\" steps over ”");
        check("》".equals(stack.stepOver('>', 1, "》")), "> steps over 》");
        stack.push("〉", 1);
        check("〉".equals(stack.stepOver('>', 1, "〉")), "> steps over the nested 〈〉 too");

        // 后半个已经不在光标后面、换了输入框、按了别的后半个键：放弃记录，照常输入。
        stack.push("）", 1);
        check(stack.stepOver(')', 1, "x") == null && stack.isEmpty(), "closing half gone");
        stack.push("）", 1);
        check(stack.stepOver(')', 2, "）") == null && stack.isEmpty(), "another editor");
        stack.push("）", 1);
        check(stack.stepOver(']', 1, "）") == null && stack.isEmpty(), "a different closing key");
        stack.push("）", 0);
        check(stack.isEmpty(), "unknown editor is not recorded");
        // 编辑器读不出光标后的文字时只能信记录（Android 宿主自己会在这种情况下放弃，见 MSIMEInputService.pairedClosingAhead）。
        stack.push("】", 1);
        check("】".equals(stack.stepOver(']', 1, null)), "unknown following text trusts the record");

        for (int index = 0; index < PairedPunctuationPolicy.Stack.LIMIT + 4; index++) stack.push("）", 1);
        int popped = 0;
        while (stack.stepOver(')', 1, "）") != null) popped++;
        check(popped == PairedPunctuationPolicy.Stack.LIMIT, "depth is capped at " + PairedPunctuationPolicy.Stack.LIMIT);
    }

    private static void symbolStepOver() {
        PairedPunctuationPolicy.Stack stack = new PairedPunctuationPolicy.Stack();
        check(stack.stepOverSymbol("」", 1, "」") == null, "nothing to step over");
        // 面板里点「补成「|」，打字后再点」：跨过去，不再多一个。
        stack.push("」", 1);
        check("」".equals(stack.stepOverSymbol("」", 1, "」")) && stack.isEmpty(), "tapping 」 steps over the auto-closed 」");
        // 键盘补上的后半个，到面板里点同一个后半个也跨过。
        stack.push("）", 1);
        check("）".equals(stack.stepOverSymbol("）", 1, "）")), "tapping ） steps over the keyboard's ）");

        // 在一对里面点不是后半个的符号，记录不动。
        stack.push("》", 1);
        stack.push("』", 1);
        check(stack.stepOverSymbol("，", 1, "』》") == null && !stack.isEmpty(), "a non-closing symbol keeps the record");
        check(stack.stepOverSymbol("「", 1, "』》") == null && !stack.isEmpty(), "an opening symbol keeps the record");
        check("』".equals(stack.stepOverSymbol("』", 1, "』》")), "inner pair first");
        check("》".equals(stack.stepOverSymbol("》", 1, "》")), "then the outer pair");

        // 对不上的后半个、ASCII 和全角不同、后半个已不在、换了输入框：放弃记录，照字面上屏。
        stack.push("）", 1);
        check(stack.stepOverSymbol(")", 1, "）") == null && stack.isEmpty(), "ASCII ) does not step over （）");
        stack.push("」", 1);
        check(stack.stepOverSymbol("』", 1, "」") == null && stack.isEmpty(), "a different closing symbol");
        stack.push("」", 1);
        check(stack.stepOverSymbol("」", 1, "x") == null && stack.isEmpty(), "closing half gone");
        stack.push("」", 1);
        check(stack.stepOverSymbol("」", 2, "」") == null && stack.isEmpty(), "another editor");
        stack.push("」", 1);
        check(stack.stepOverSymbol(null, 1, "」") == null && !stack.isEmpty(), "no symbol");
    }

    private static void caretAfterClosingCommit() {
        // 照规矩处理 commitText(closing, 0) 的编辑器：光标后面正是后半个，不补发方向键。
        check(!PairedPunctuationPolicy.caretPassedClosing("）", "）", "（"), "caret already between the pair");
        check(!PairedPunctuationPolicy.caretPassedClosing("）", "）后文", null), "honoured editor needs no read before the caret");
        // #6458：应用不认 0，光标落在后半个后面，前面是它、后面不是它，要左移一格。
        check(PairedPunctuationPolicy.caretPassedClosing("）", "", "）"), "caret at the end after the closing half");
        check(PairedPunctuationPolicy.caretPassedClosing("】", "后文", "】"), "caret after the closing half before other text");
        check(PairedPunctuationPolicy.caretPassedClosing("”", "x", "”"), "quotes too");
        // 读不出光标后的文字、或两侧都不是后半个（应用改写或拒绝了写入）：不知道光标在哪，保持原来的行为。
        check(!PairedPunctuationPolicy.caretPassedClosing("）", null, "）"), "unknown text after the caret");
        check(!PairedPunctuationPolicy.caretPassedClosing("）", "", null), "unknown text before the caret");
        check(!PairedPunctuationPolicy.caretPassedClosing("）", "", "（"), "closing half filtered out by the editor");
        check(!PairedPunctuationPolicy.caretPassedClosing("", "", ""), "nothing was committed");
        check(!PairedPunctuationPolicy.caretPassedClosing(null, "", "）"), "no closing half");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

import app.msime.android.PairedPunctuationPolicy;

public final class PairedPunctuationPolicySmoke {
    public static void main(String[] args) {
        engineCompletions();
        quoteReopening();
        symbolPanelPairs();
        literalPairs();
        stepOver();
        symbolStepOver();
        caretAfterClosingCommit();
        explicitCursor();
        punctuationStyles();
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

    /** 直角引号与空心方头括号（`quote_style`、`bracket_style`）补全、改回前引号和跨过都和缺省样式一样。 */
    private static void punctuationStyles() {
        PairedPunctuationPolicy.Completion corner = PairedPunctuationPolicy.completion("「", true);
        check(corner.closing().equals("」") && corner.opening() == '\'', "「 closes on the ' key");
        check(PairedPunctuationPolicy.completion("你好『", true).closing().equals("』"), "『 closes");
        check(PairedPunctuationPolicy.completion("〖", true).closing().equals("〗"), "〖 closes");
        check(PairedPunctuationPolicy.completion("」", true) == null, "a corner closing half is not completed");
        check(PairedPunctuationPolicy.reopenQuote("」", '\'', true).equals("「"), "」 reopens as 「");
        check(PairedPunctuationPolicy.reopenQuote("好』", '"', true).equals("好『"), "』 reopens as 『");
        check(PairedPunctuationPolicy.reopenQuote("』", '\'', true).equals("』"), "another key's half is left alone");
        PairedPunctuationPolicy.Stack stack = new PairedPunctuationPolicy.Stack();
        stack.push("〗", 9);
        stack.push("』", 9);
        stack.push("」", 9);
        check("」".equals(stack.stepOver('\'', 9, "」』〗")), "' steps over 」");
        check("』".equals(stack.stepOver('"', 9, "』〗")), "\" steps over 』");
        check("〗".equals(stack.stepOver(']', 9, "〗")), "] steps over 〗");
        check(stack.isEmpty(), "every corner pair was closed");
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

    /** `title:"${cursor}"` 以 `(` 结束时上屏 `title:""（`，后缀是 `"（`：光标落在两个引号之间，那里既不补后半个也不改回前引号。宿主两处调用都只走 `punctuationCommit`，这里直接测它。 */
    private static void explicitCursor() {
        check(PairedPunctuationPolicy.explicitCursor("title:\"\"（", "\"（"), "a cursor phrase finished by ( asks for a position");
        check(PairedPunctuationPolicy.explicitCursor("末尾（", ""), "the end marker is an explicit template too");
        check(PairedPunctuationPolicy.explicitCursor("好”", "”"), "a quote tail keeps the request");
        check(!PairedPunctuationPolicy.explicitCursor("（", null), "an ordinary commit has no request");
        check(!PairedPunctuationPolicy.explicitCursor("title:\"\"（", "stale"), "a mismatched suffix is no request");
        check(!PairedPunctuationPolicy.explicitCursor(null, ""), "no commit, no request");

        PairedPunctuationPolicy.PunctuationCommit template =
            PairedPunctuationPolicy.punctuationCommit("title:\"\"（", "\"（", '(', true);
        check(template.commit().equals("title:\"\"（") && template.completion() == null,
            "a cursor template finished by ( lands as written with no closing mark");
        PairedPunctuationPolicy.PunctuationCommit end = PairedPunctuationPolicy.punctuationCommit("末尾（", "", '(', true);
        check(end.commit().equals("末尾（") && end.completion() == null, "the end marker lands as written too");
        PairedPunctuationPolicy.PunctuationCommit quote = PairedPunctuationPolicy.punctuationCommit("好”", "”", '"', true);
        check(quote.commit().equals("好”") && quote.completion() == null, "a cursor template's quote is not reopened");
        PairedPunctuationPolicy.PunctuationCommit plainQuote = PairedPunctuationPolicy.punctuationCommit("好”", null, '"', true);
        check(plainQuote.commit().equals("好“") && plainQuote.completion() != null
            && plainQuote.completion().closing().equals("”"), "without a template the quote reopens and closes");
        PairedPunctuationPolicy.PunctuationCommit plain = PairedPunctuationPolicy.punctuationCommit("你好（", null, '(', true);
        check(plain.commit().equals("你好（") && plain.completion() != null
            && plain.completion().closing().equals("）"), "without a template ( still closes");
        PairedPunctuationPolicy.PunctuationCommit stale =
            PairedPunctuationPolicy.punctuationCommit("title:\"\"（", "stale", '(', true);
        check(stale.completion() != null, "a mismatched suffix is an ordinary commit");
        PairedPunctuationPolicy.PunctuationCommit off = PairedPunctuationPolicy.punctuationCommit("好”", null, '"', false);
        check(off.commit().equals("好”") && off.completion() == null, "switch off: no reopen, no closing");
        PairedPunctuationPolicy.PunctuationCommit none = PairedPunctuationPolicy.punctuationCommit(null, "\"", '"', true);
        check(none.commit() == null && none.completion() == null, "no commit, nothing to do");
    }

    private static void literalPairs() {
        for (String[] pair : new String[][] {{"(", ")"}, {"（", "）"}, {"[", "]"}, {"{", "}"},
                {"【", "】"}, {"《", "》"}, {"〈", "〉"}, {"「", "」"}, {"『", "』"}, {"“", "”"},
                {"‘", "’"}, {"〔", "〕"}, {"〖", "〗"}, {"＜", "＞"}, {"｛", "｝"}, {"［", "］"}})
            check(pair[1].equals(PairedPunctuationPolicy.literalPairClosing(pair[0] + pair[1])),
                "a complete literal pair keeps its closing half");
        for (String literal : new String[] {null, "", "(", ")", "(]", "x()", "()x", "(())", ":-)", "……", "😀", "<>"})
            check(PairedPunctuationPolicy.literalPairClosing(literal) == null,
                "other literal strings are not split into a pair");
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
        check(!PairedPunctuationPolicy.caretPassedClosing("）", "）后文", null), "unknown text before the caret does not trigger a move");
        // #6458：应用不认 0，光标落在后半个后面，前面是它，要左移一格。
        check(PairedPunctuationPolicy.caretPassedClosing("）", "", "）"), "caret at the end after the closing half");
        check(PairedPunctuationPolicy.caretPassedClosing("】", "后文", "】"), "caret after the closing half before other text");
        check(PairedPunctuationPolicy.caretPassedClosing("”", "x", "”"), "quotes too");
        check(PairedPunctuationPolicy.caretPassedClosing(")", ")", ")"),
            "an outer identical closing half must not hide a misplaced nested caret");
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

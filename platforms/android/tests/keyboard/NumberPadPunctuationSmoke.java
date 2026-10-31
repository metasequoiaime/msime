package app.msime.android;

import java.util.List;

/** 每种方案都借数字九宫格之后，左列符号、右列 ！ 和第五排括号跟着当前语言换写法；长空格样式借来的数字九宫格照样有空格。 */
public final class NumberPadPunctuationSmoke {
    public static void main(String[] args) {
        NumberPadPunctuation.Style chinese = NumberPadPunctuation.Style.CHINESE;
        NumberPadPunctuation.Style japanese = NumberPadPunctuation.Style.JAPANESE;
        NumberPadPunctuation.Style ascii = NumberPadPunctuation.Style.ASCII;

        // 写法的判定：中文标点生效时是中文；日文方案（没有英文、本地模式）是日文，不看中文标点开关；英文、本地模式、韩文、越南语、藏文和关掉中文标点的中文是半角。
        for (int scheme : new int[] {InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN, InputSchemeTraits.WUBI,
                InputSchemeTraits.CANTONESE, InputSchemeTraits.ZHUYIN, InputSchemeTraits.STROKE, -1}) {
            check(NumberPadPunctuation.style(true, false, scheme, "none") == chinese, "Chinese scheme " + scheme);
            check(NumberPadPunctuation.style(false, false, scheme, "none") == ascii, "Chinese punctuation off " + scheme);
            check(NumberPadPunctuation.style(true, true, scheme, "none") == ascii, "English mode " + scheme);
            check(NumberPadPunctuation.style(true, false, scheme, "calculator") == ascii, "local mode " + scheme);
        }
        for (boolean punctuation : new boolean[] {true, false}) {
            check(NumberPadPunctuation.style(punctuation, false, InputSchemeTraits.JAPANESE, "none") == japanese,
                "Japanese ignores the Chinese punctuation switch");
            check(NumberPadPunctuation.style(punctuation, true, InputSchemeTraits.JAPANESE, "none") == ascii,
                "English mode from Japanese");
            for (int scheme : new int[] {InputSchemeTraits.KOREAN, InputSchemeTraits.VIETNAMESE, InputSchemeTraits.TIBETAN})
                check(NumberPadPunctuation.style(punctuation, false, scheme, "none") == ascii,
                    "scheme " + scheme + " writes ASCII marks");
        }

        // 出厂数字表：中文原样；英文里不留任何中文全角标点，四则运算不变，也不出现重复的键。
        List<String> defaults = NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS;
        check(NumberPadPunctuation.symbols(defaults, chinese) == defaults, "Chinese keeps the table as is");
        List<String> english = NumberPadPunctuation.symbols(defaults, ascii);
        check(english.equals(List.of("+", "-", "*", "/", "=", "%", "(", ")", ":", "@", ",", ".", "?", "!")),
            "English digit sidebar: " + english);
        for (String symbol : english)
            for (int index = 0; index < symbol.length(); index++)
                check(symbol.charAt(index) < 0x80, "no full-width punctuation in English: " + symbol);
        List<String> kana = NumberPadPunctuation.symbols(defaults, japanese);
        check(kana.equals(List.of("+", "-", "*", "/", "=", "%", "(", ")", ":", "@", "、", "。", "？", "！")),
            "Japanese digit sidebar: " + kana);

        // 自定义表同样换写法；换完相同的符号只留第一个。
        List<String> custom = List.of("，", "、", "“", "”", "（）", "…", "￥");
        check(NumberPadPunctuation.symbols(custom, ascii).equals(List.of(",", "\"", "()", "…", "￥")),
            "custom table in English");
        check(NumberPadPunctuation.symbols(custom, japanese).equals(List.of("、", "「", "」", "（）", "…", "￥")),
            "custom table in Japanese");

        // 右列 ！ 和第五排括号。
        check("!".equals(NumberPadPunctuation.face("！", ascii)) && "！".equals(NumberPadPunctuation.face("！", japanese))
            && "！".equals(NumberPadPunctuation.face("！", chinese)), "right-column exclamation");
        check(KeyboardLayout.symbolExtraRow(false, true, NumberPadPunctuation.fullWidthBrackets(ascii)).get(0).equals("("),
            "English fifth row brackets");
        check(KeyboardLayout.symbolExtraRow(false, true, NumberPadPunctuation.fullWidthBrackets(japanese)).get(0).equals("（"),
            "Japanese fifth row brackets");

        // 长空格样式的 26 键也借数字九宫格：0 和空格的三种排法里，空格要么在底行，要么由宿主放进右列（右列时底行是 0 占满空格的份额）。符 照样在底行。
        for (TouchKeyStyle.NumberPadSpace space : TouchKeyStyle.NumberPadSpace.values()) {
            List<KeyboardActionRow.DesignEntry> pad = KeyboardActionRow.numberPadEntries(
                KeyboardLayout.STANDARD_TOUCH_LAYOUT, false, false, TouchKeyStyle.Row.LONG_SPACE, false, space);
            boolean spaceInRow = pad.stream().anyMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.SPACE);
            check(spaceInRow != space.inRightColumn(), "long-space pad keeps a space key: " + space);
            check(pad.stream().anyMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.ZERO)
                && pad.stream().anyMatch(entry -> entry.slot() == KeyboardActionRow.DesignSlot.SYMBOL_PANEL)
                && pad.get(0).slot() == KeyboardActionRow.DesignSlot.LAYER, "long-space pad row: " + space);
        }
        System.out.println("number pad punctuation follows the language: passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

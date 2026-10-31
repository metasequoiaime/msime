package app.msime.android;

import java.util.List;

/** 每种方案都借数字九宫格之后，左列符号、右列 ！ 和第五排括号跟着当前语言换写法，半角写法的出厂表接上半角符号，本地模式、越南语和藏文里 Engine 当作拼写的键交给会话，注音的 #+= 是共享符号行；长空格样式借来的数字九宫格照样有空格。 */
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

        // 半角写法的左列：出厂表换完写法后按 @ : ; ( ) ' " $ 的顺序接上表里还没有的（@ : ( ) 出厂表里已经有），英文、本地模式、韩文、越南语、藏文和关掉中文标点的中文都是这一张。
        check(NumberPadPunctuation.ASCII_EXTRAS.equals(List.of("@", ":", ";", "(", ")", "'", "\"", "$")),
            "ASCII extras order: " + NumberPadPunctuation.ASCII_EXTRAS);
        List<String> asciiPad = NumberPadPunctuation.symbols(defaults, ascii, false);
        check(asciiPad.equals(List.of("+", "-", "*", "/", "=", "%", "(", ")", ":", "@", ",", ".", "?", "!",
            ";", "'", "\"", "$")), "ASCII digit sidebar with extras: " + asciiPad);
        check(asciiPad.size() == new java.util.LinkedHashSet<>(asciiPad).size(), "extras never duplicate a key: " + asciiPad);
        check(asciiPad.size() <= NineKeySidebarPolicy.MAX_SYMBOLS, "extras stay within the table limit");
        // 共享偏好是空表或写着与出厂表相同的非空表，都按出厂表算（NineKeySidebarPolicy.custom），都接。
        for (List<String> stored : List.of(List.<String>of(), defaults)) {
            check(!NineKeySidebarPolicy.custom(stored, defaults), "factory table " + stored);
            check(NumberPadPunctuation.symbols(NineKeySidebarPolicy.digitSymbols(stored), ascii,
                NineKeySidebarPolicy.custom(stored, defaults)).equals(asciiPad), "factory table gets the extras");
        }
        // 中文、日文写法不接。
        check(NumberPadPunctuation.symbols(defaults, chinese, false) == defaults, "Chinese gets no extras");
        check(NumberPadPunctuation.symbols(defaults, japanese, false).equals(kana), "Japanese gets no extras");
        // 用户自定义的表原样保留（只换写法），不接：用户挑的符号和顺序不被改动。
        List<String> mine = List.of("+", "，", "#");
        check(NineKeySidebarPolicy.custom(mine, defaults), "a user table is custom");
        check(NumberPadPunctuation.symbols(mine, ascii, true).equals(List.of("+", ",", "#")),
            "custom table in English is only localized: " + NumberPadPunctuation.symbols(mine, ascii, true));
        // 表里已经有的半角符号跳过，其余照顺序接在末尾（出厂表以外的情形，例如出厂表本身改了，规则也成立）。
        check(NumberPadPunctuation.symbols(List.of("$", "'", "；"), ascii, false)
                .equals(List.of("$", "'", ";", "@", ":", "(", ")", "\"")),
            "skip extras already present: " + NumberPadPunctuation.symbols(List.of("$", "'", "；"), ascii, false));

        // 数字九宫格的键交不交给会话（与 iOS typeNumberPadText 同一条规则）：只在本地模式和越南语、藏文里，Engine 此刻把这个键列为拼写时交给会话；藏文的 ' 总是先交；其余按字面上屏。
        // U 模式的十六进制数字：本地模式算作组字中，spelling_symbols 列着数字。
        check(NumberPadPunctuation.typesThroughSession("7", "unicode", false, false, "", "0123456789abcdefABCDEF"),
            "U-mode code-point digit goes to the session");
        check(NumberPadPunctuation.typesThroughSession("+", "calculator", false, false, "", "0123456789+-*/.()"),
            "calculator operator goes to the session");
        check(!NumberPadPunctuation.typesThroughSession("$", "calculator", false, false, "", "0123456789+-*/.()"),
            "a key the local mode does not spell stays literal");
        // VNI 越南语：正在拼的词里声调数字是拼写；没有组字时数字只是数字。
        check(NumberPadPunctuation.typesThroughSession("6", "none", true, false, "viet", "0123456789"),
            "VNI tone digit while composing goes to the session");
        check(!NumberPadPunctuation.typesThroughSession("6", "none", true, false, "", "0123456789"),
            "Vietnamese digit without a word is literal");
        // 藏文：组字时 + - . / 是拼写（叠写、消歧、垂符）；没有组字时 + 按字面；' 不论有没有组字都先交给会话（achung 'od）。
        for (String spelling : List.of("+", "-", ".", "/"))
            check(NumberPadPunctuation.typesThroughSession(spelling, "none", true, true, "pad", "'+-./"),
                "Tibetan " + spelling + " while composing goes to the session");
        check(!NumberPadPunctuation.typesThroughSession("+", "none", true, true, "", "'/"), "idle Tibetan + is literal");
        check(NumberPadPunctuation.typesThroughSession("'", "none", true, true, "", "'/")
            && NumberPadPunctuation.typesThroughSession("'", "none", true, true, "", ""), "Tibetan achung always goes to the session");
        check(!NumberPadPunctuation.typesThroughSession("$", "none", true, true, "pad", "'+-./"), "Tibetan $ is literal");
        // 大千注音把 1–0 和 , . / ; - 列为拼写（它们是注音键），数字九宫格上的数字照样按字面；英文、中文、韩文同样按字面。
        check(!NumberPadPunctuation.typesThroughSession("1", "none", false, false, "1", "1234567890,./;-"),
            "Dachen digit stays literal");
        check(!NumberPadPunctuation.typesThroughSession("1", "none", false, false, "", ""), "English digit stays literal");
        check(!NumberPadPunctuation.typesThroughSession("$", "none", false, false, "nihao", "$%"),
            "Chinese spelling $ stays literal on the number pad");
        check(!NumberPadPunctuation.typesThroughSession("'", "none", false, false, "", "'"), "apostrophe outside Tibetan is literal");
        // 不是单个 ASCII 可见字符的键（全角括号、¥ 等）一律按字面。
        check(!NumberPadPunctuation.typesThroughSession("（", "calculator", false, false, "", "（"), "full-width key is literal");
        check(!NumberPadPunctuation.typesThroughSession("()", "calculator", false, false, "", "()"), "multi-character key is literal");
        check(asciiPad.contains("'"), "the Tibetan number pad offers the achung key");

        // 注音（大千和注音九键）的 #+= 是共享符号行，不是设计稿的 #+= 层；123 那一页仍是数字九宫格，其他键面的 #+= 是设计稿那一页。
        for (int layout : new int[] {KeyboardLayout.ZHUYIN_LAYOUT, KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT}) {
            check(KeyboardLayout.drawsSharedSymbolRows(KeyboardLayout.Layer.SYMBOLS, true, layout), "Zhuyin #+= " + layout);
            check(!KeyboardLayout.drawsSharedSymbolRows(KeyboardLayout.Layer.SYMBOLS, false, layout), "Zhuyin 123 " + layout);
            check(!KeyboardLayout.drawsSharedSymbolRows(KeyboardLayout.Layer.LETTERS, true, layout), "Zhuyin letters " + layout);
        }
        for (int layout : new int[] {KeyboardLayout.STANDARD_TOUCH_LAYOUT, KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT,
                KeyboardLayout.KOREAN_LAYOUT, KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT, KeyboardLayout.STROKE_LAYOUT,
                KeyboardLayout.HANDWRITING_LAYOUT, KeyboardLayout.FOURTEEN_KEY_LAYOUT, KeyboardLayout.EIGHTEEN_KEY_LAYOUT})
            check(!KeyboardLayout.drawsSharedSymbolRows(KeyboardLayout.Layer.SYMBOLS, true, layout), "design #+= " + layout);
        // 共享符号行就是注音原来的数字符号页：1–0、, . ? ! ; : ' " @ /、( ) [ ] < > \ - _ =，中文标点时键面是 ； ： （ ） 这样的全角；大千会读成音键的数字和 , . / ; - 按字面上屏。
        List<List<String>> shared = KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT);
        check(shared.equals(KeyboardLayout.rows(KeyboardLayout.Layer.SYMBOLS, KeyboardLayout.ZHUYIN_LAYOUT))
            && shared.size() == 3 && shared.stream().allMatch(row -> row.size() == 10), "shared rows: " + shared);
        check(shared.get(1).contains(";") && shared.get(1).contains(":") && shared.get(1).contains("@")
            && shared.get(2).contains("-") && shared.get(2).contains("("), "shared rows carry ; : @ - (");
        check("；".equals(ChineseSymbolFaces.face(";", true)) && "：".equals(ChineseSymbolFaces.face(":", true)),
            "shared rows draw full-width marks under Chinese punctuation");
        for (String claimed : List.of("1", "0", ",", ".", "/", ";", "-"))
            check(ZhuyinKeyboardLayout.claimsSymbol(claimed), "Dachen claims " + claimed);
        // 这一页用大千注音的底行（注音九键的字母层没有底行），底行有 123（层键）回到数字九宫格。
        check(KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_LAYOUT, false).get(0).slot()
            == KeyboardActionRow.DesignSlot.LAYER, "shared rows bottom row starts with 123");
        check(KeyboardActionRow.designEntries(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false).isEmpty(),
            "Zhuyin nine-key letters have no bottom row to borrow");
        check("123".equals(KeyboardActionRow.layerTitle(KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT, false))
            && "切换到数字和符号".equals(KeyboardActionRow.layerDescription(false)), "shared rows layer key reads 123");

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

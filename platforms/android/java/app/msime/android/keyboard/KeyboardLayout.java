package app.msime.android;

import java.util.List;

/** The two keyboard layers shared by the Android view and its host-side tests. */
public final class KeyboardLayout {
    public enum Layer { LETTERS, SYMBOLS }

    public static final int STANDARD_TOUCH_LAYOUT = 0;
    public static final int QUANPIN_NINE_KEY_LAYOUT = 1;
    public static final int JAPANESE_NINE_KEY_LAYOUT = 2;
    public static final int HANDWRITING_LAYOUT = 3;
    /** The 26 QWERTY keys drawn with Dubeolsik jamo keycaps; they still send the ASCII letters. */
    public static final int KOREAN_LAYOUT = 4;
    /** The four Dachen bopomofo rows of {@link ZhuyinKeyboardLayout}; they send the Dachen ASCII keys. */
    public static final int ZHUYIN_LAYOUT = 5;
    /** 笔画方案的九键外框：标点侧栏、{@link StrokeKeyboardLayout} 的 2×3 笔画网格和 ⌫ 列；笔画键发送字母 h s p n z x。 */
    public static final int STROKE_LAYOUT = 6;
    /** 注音 9 键：声调列、{@link ZhuyinNineKeyLayout} 的 1-9/0 音键网格和 ⌫ 列；音键发送数字，声调键发送 z x c v b。 */
    public static final int ZHUYIN_NINE_KEY_LAYOUT = 7;
    /** 全拼 14 键：{@link FourteenKeyLayout} 的三行两字母键，点按经 `gridKey` 送这一组的首字母；123 层、底行和 26 键相同。 */
    public static final int FOURTEEN_KEY_LAYOUT = 8;
    /** 十八键全拼：组码、拼音选择和候选复用已有网格路径。 */
    public static final int EIGHTEEN_KEY_LAYOUT = 9;

    /** 以字母组输入、把拼音读音留在键盘上的网格。 */
    public static boolean drawsLetterGrid(int layout) {
        return layout == FOURTEEN_KEY_LAYOUT || layout == EIGHTEEN_KEY_LAYOUT;
    }

    /** 引擎组码网格的键面：读音取 `nine_key_reading`，九键写进输入框，十四／十八键画在读音行上；展开候选是三栏面板。 */
    public static boolean drawsKeyGrid(int layout) {
        return layout == QUANPIN_NINE_KEY_LAYOUT || drawsLetterGrid(layout);
    }

    private static final List<List<String>> LETTER_ROWS = List.of(
        List.of("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
        List.of("a", "s", "d", "f", "g", "h", "j", "k", "l"),
        List.of("z", "x", "c", "v", "b", "n", "m")
    );
    private static final List<List<String>> SYMBOL_ROWS = List.of(
        List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0"),
        List.of(",", ".", "?", "!", ";", ":", "'", "\"", "@", "/"),
        List.of("(", ")", "[", "]", "<", ">", "\\", "-", "_", "=")
    );

    /** 数字行（#6022）：本地设置「数字行」打开时画在字母上方的 1–0。 */
    public static final List<String> NUMBER_ROW = List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0");
    /** 窗口可用高度（`Configuration.screenHeightDp`）低于它时不画数字行：横屏手机的窗口只有 330–430 dp，默认高度的键盘（约 300 dp）再加一行 63 dp 就把工具栏或底行挤出窗口、应用也没有可见区域了。竖屏手机、平板横屏和折叠屏内屏都在 480 dp 以上。 */
    public static final int NUMBER_ROW_MIN_WINDOW_HEIGHT_DP = 480;

    private KeyboardLayout() {}

    /** 数字网格只换显示：中文的常规入口复用九宫格，本地模式与独立语言布局保留原符号行。 */
    public static boolean drawsNumberPad(Layer layer, int layout, boolean chinese, boolean localMode) {
        if (layer != Layer.SYMBOLS) return false;
        if (layout == QUANPIN_NINE_KEY_LAYOUT) return true;
        return chinese && !localMode && (layout == STANDARD_TOUCH_LAYOUT
            || layout == FOURTEEN_KEY_LAYOUT || layout == EIGHTEEN_KEY_LAYOUT || layout == HANDWRITING_LAYOUT
            || layout == STROKE_LAYOUT || layout == ZHUYIN_NINE_KEY_LAYOUT);
    }

    /** 数字九宫格只属于中文方案：日文罗马字、越南语、藏文等独立语言与 iOS、HarmonyOS 一样保留各自的符号层；视图还没给出方案时沿用中文默认。 */
    public static boolean numberPadLanguage(int scheme, boolean english) {
        return !english && (!InputSchemeTraits.known(scheme) || InputSchemeTraits.isChinese(scheme));
    }

    /**
     * 这一刻要不要在字母上方画数字行（#6022）：设置打开、在字母层、画的是标准 26 键一族或韩文两套式，并且窗口够高。九键、笔画、手写、日文假名网格和注音（大千的数字本身是注音键，注音 9 键是网格）都不画；123 / #+= 层由补充符号行保留同一排高度；窗口低于 {@link #NUMBER_ROW_MIN_WINDOW_HEIGHT_DP}（横屏手机）时也不画，存着的设置不变，转回竖屏自动回来。
     *
     * @param enabled 本地设置 `platform.android.number_row`
     * @param windowHeightDp 键盘所在窗口的可用高度（`Configuration.screenHeightDp`）；未知（0 或负数）时不按高度拦
     */
    public static boolean drawsNumberRow(boolean enabled, Layer layer, int touchLayout, int windowHeightDp) {
        return enabled && layer == Layer.LETTERS
            && (touchLayout == STANDARD_TOUCH_LAYOUT || touchLayout == KOREAN_LAYOUT)
            && (windowHeightDp <= 0 || windowHeightDp >= NUMBER_ROW_MIN_WINDOW_HEIGHT_DP);
    }

    /** 符号层沿用源字母页的第五排资格；借用数字九宫格也不缩高。 */
    public static boolean drawsSymbolExtraRow(boolean enabled, Layer layer, int layout, int heightDp) {
        return layer == Layer.SYMBOLS && drawsNumberRow(enabled, Layer.LETTERS, layout, heightDp);
    }

    /** 数字页补原来没有的符号，扩展符号页补数字；网格页的括号随标点设置。 */
    public static List<String> symbolExtraRow(boolean more, boolean numberPad, boolean chinesePunctuation) {
        if (more) return NUMBER_ROW;
        if (!numberPad) return MORE_SYMBOLS_FIRST;
        return List.of(chinesePunctuation ? "（" : "(", chinesePunctuation ? "）" : ")",
            "%", "¥", "$", "€", "£", "‰", "²", "³");
    }

    /** 只替换中文全拼字母页的 Shift；直输状态由宿主作为 chinese=false 排除。 */
    public static boolean usesQuanpinSeparator(boolean quanpin, boolean chinese,
            Layer layer, int touchLayout, String localMode) {
        return quanpin && chinese && layer == Layer.LETTERS
            && touchLayout == STANDARD_TOUCH_LAYOUT && "none".equals(localMode);
    }

    /** Resolves the host surface from the Engine view without conflating Japanese nine-key. */
    public static int resolveTouchLayout(boolean handwriting, boolean nineKey, int scheme,
                                         String touchLayout) {
        return resolveTouchLayout(handwriting, nineKey, false, scheme, touchLayout);
    }

    /**
     * 同 {@link #resolveTouchLayout(boolean, boolean, int, String)}，`fourteenKey` 是引擎此刻开着 14 键网格（`View.key_grid` 为 `fourteen_key`）且没有本地模式：本地模式要逐个确定的字母，14 键的组码打不出来，所以回落 26 键。14 键在全拼或双拼下开，排在九键之前判断。
     */
    public static int resolveTouchLayout(boolean handwriting, boolean nineKey, boolean fourteenKey,
                                         int scheme, String touchLayout) {
        return resolveTouchLayout(handwriting, nineKey, fourteenKey, false, scheme, touchLayout);
    }

    /** 网格开关来自引擎当前视图；只存着十八键偏好时不能改变本地模式的确定字母键面。 */
    public static int resolveTouchLayout(boolean handwriting, boolean nineKey, boolean fourteenKey,
                                         boolean eighteenKey, int scheme, String touchLayout) {
        // Korean has only the 26-key Dubeolsik keyboard, whatever layout the document carries.
        if (scheme == KoreanInputPolicy.KOREAN_SCHEME) return KOREAN_LAYOUT;
        // Zhuyin draws the nine-key bopomofo grid when the document stores the nine-key layout, the Dachen rows otherwise; it has no handwriting surface. The stored layout decides, as for Japanese, rather than the view's nine_key flag.
        if (scheme == InputSchemeTraits.ZHUYIN && "nine_key".equals(touchLayout)) return ZHUYIN_NINE_KEY_LAYOUT;
        if (scheme == InputSchemeTraits.ZHUYIN) return ZHUYIN_LAYOUT;
        if (handwriting || "handwriting".equals(touchLayout)) return HANDWRITING_LAYOUT;
        // 笔画方案画自己的笔画键盘：偏好是 26 键还是 9 键都一样，只有手写让给手写面板。
        if (scheme == InputSchemeTraits.STROKE) return STROKE_LAYOUT;
        if (scheme == 3 && "nine_key".equals(touchLayout)) return JAPANESE_NINE_KEY_LAYOUT;
        if (fourteenKey) return FOURTEEN_KEY_LAYOUT;
        if (eighteenKey) return EIGHTEEN_KEY_LAYOUT;
        if (nineKey) return QUANPIN_NINE_KEY_LAYOUT;
        return STANDARD_TOUCH_LAYOUT;
    }

    /**
     * Whether the surface has a Shift state of its own: English case on the standard rows, the double consonants and ㅒ ㅖ on the Korean rows. Every other surface resets it, the Dachen rows included: bopomofo has no case, and Shift there would only send a key the Dachen table does not read.
     */
    public static boolean carriesLetterCase(int touchLayout) {
        return touchLayout == STANDARD_TOUCH_LAYOUT || touchLayout == KOREAN_LAYOUT;
    }

    /**
     * The keys a layer sends, always in their canonical form.
     *
     * <p>Letters stay lowercase here because this is what reaches the input engine, and the engine
     * only starts a pinyin composition from a lowercase letter. What the user sees is a separate
     * question answered by {@link LetterKeyFacePolicy}: a Chinese keyboard draws its 26 keys in
     * caps without any of them being a capital letter. This returned uppercase alongside the face
     * once, which sent `N` to the engine, got it declined, and committed the letter literally --
     * Chinese input produced `NIHAO` instead of 你好.
     */
    public static List<List<String>> rows(Layer layer) {
        return layer == Layer.SYMBOLS ? SYMBOL_ROWS : LETTER_ROWS;
    }

    /** 符号页按键实际发出的字符：藏文方案下第三排的 `=` 换成威利叠写用的 `+`，让组字中的叠写（如 `pad+ma`）照常作为字符交给 Engine；符号面板会先上屏组字，不能用来叠写。其他方案原样发出。 */
    public static String symbolRowKey(String key, boolean tibetan) {
        return tibetan && "=".equals(key) ? "+" : key;
    }

    /** The rows a surface draws: the Dachen letter layer has four rows of its own, every other surface the shared ones. */
    public static List<List<String>> rows(Layer layer, int touchLayout) {
        if (layer == Layer.LETTERS && touchLayout == ZHUYIN_LAYOUT) return ZhuyinKeyboardLayout.rows();
        return rows(layer);
    }

    // ---- 新设计的 123 层与 #+= 层（plan §2.8、N/design-screens.md IME · keys） ----

    /** 新设计 123 / #+= 层里一个键的种类。 */
    public enum LayerKeyKind {
        /** 发出 {@link LayerKey#text()} 本身的字符键。 */
        CHARACTER,
        /** 123 层的 `#+=`（切到更多符号）或 #+= 层的 `123`（切回数字）。 */
        LAYER_TOGGLE,
        DELETE,
        /** 底行左下：回到字母键盘（中文「拼音」、英文「ABC」）。 */
        LETTERS,
        /** 123 层底行的表情键。 */
        EMOJI,
        /** 123 / #+= 层底行的中英文标点切换。 */
        PUNCTUATION_TOGGLE,
        /** #+= 层底行原表情位的「符号」键，打开符号面板。 */
        SYMBOL_PANEL,
        SPACE,
        /** 回车：文字与描述由视图按 {@link ReturnKeyAction} 替换，这里给的是空闲时的「换行」。 */
        RETURN
    }

    /**
     * 新设计层里的一个键。
     *
     * @param text 节点 text（字符键就是它发出的字符）
     * @param description contentDescription
     * @param kind 种类
     * @param weight 在本行里的宽度份额
     */
    public record LayerKey(String text, String description, LayerKeyKind kind, float weight) {}

    private static final List<String> NUMBER_DIGITS =
        List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0");
    private static final List<String> CHINESE_NUMBER_SYMBOLS =
        List.of("-", "/", "：", "；", "（", "）", "¥", "@", "“", "”");
    /** 直角引号样式（View 的 `punctuation_marks`）下的中文 123 层：两个引号键代表首选引号、原样上屏，所以换成「」，不按 ASCII 键位对应成『』。 */
    private static final List<String> CHINESE_CORNER_NUMBER_SYMBOLS =
        List.of("-", "/", "：", "；", "（", "）", "¥", "@", "「", "」");
    private static final List<String> ENGLISH_NUMBER_SYMBOLS =
        List.of("-", "/", ":", ";", "(", ")", "$", "@", "\"", "'");
    private static final List<String> CHINESE_PUNCTUATION = List.of("。", "，", "、", "？", "！");
    private static final List<String> ENGLISH_PUNCTUATION = List.of(".", ",", "?", "!", "…");
    private static final List<String> MORE_SYMBOLS_FIRST =
        List.of("[", "]", "{", "}", "#", "%", "^", "*", "+", "=");
    private static final List<String> CHINESE_MORE_SYMBOLS_SECOND =
        List.of("_", "\\", "|", "~", "《", "》", "€", "&", "·", "…");
    private static final List<String> ENGLISH_MORE_SYMBOLS_SECOND =
        List.of("_", "\\", "|", "~", "<", ">", "€", "&", "·", "£");

    /** 第三行两端（`#+=` / `123` 与 ⌫）的宽度份额，与字母层的 ⇧ ⌫ 相同。 */
    public static final float LAYER_EDGE_WEIGHT = 1.4f;
    /** 层底行的宽度份额：返回字母 1.25、表情 / 符号 1.05、中英文标点 1.05、空格 4.95、回车 1.9，合计与原来的底行一样宽（10.2）。 */
    public static final float LAYER_BACK_WEIGHT = 1.25f;
    public static final float LAYER_EMOJI_WEIGHT = 1.05f;
    public static final float LAYER_SPACE_WEIGHT = 4.95f;
    public static final float LAYER_RETURN_WEIGHT = 1.9f;

    /**
     * 新设计的 123 层：1–0 / 十个符号 / `#+=` + 五个标点 + ⌫ / 拼音（或 ABC）| 😀 | 中标或英标 | 空格 | ↵。
     *
     * @param chinese 中文模式用全角符号与中文标点，英文模式用 ASCII 版，底行返回键为「ABC」
     */
    public static List<List<LayerKey>> numberLayer(boolean chinese) {
        return numberLayer(chinese, chinese);
    }

    /** `chinese` 决定左下返回键是「拼音」还是「ABC」；`chinesePunctuation` 决定标点和符号用全角中文还是半角英文，跟「中文标点」开关走。两者分开：中文模式下关掉中文标点时，这一层要上屏半角标点。 */
    public static List<List<LayerKey>> numberLayer(boolean chinese, boolean chinesePunctuation) {
        return numberLayer(chinese, chinesePunctuation, false);
    }

    /** 同 {@link #numberLayer(boolean, boolean)}；`cornerQuotes` 为真时中文标点下的两个引号键是「」。 */
    public static List<List<LayerKey>> numberLayer(boolean chinese, boolean chinesePunctuation,
            boolean cornerQuotes) {
        return numberLayer(chinese, chinesePunctuation, cornerQuotes, false);
    }

    /** 顶部已有表情入口时，同一位置直接打开符号面板；关闭顶部入口时保留表情兜底。 */
    public static List<List<LayerKey>> numberLayer(boolean chinese, boolean chinesePunctuation,
            boolean cornerQuotes, boolean toolbarEmojiAvailable) {
        return layer(NUMBER_DIGITS,
            chinesePunctuation ? (cornerQuotes ? CHINESE_CORNER_NUMBER_SYMBOLS : CHINESE_NUMBER_SYMBOLS)
                : ENGLISH_NUMBER_SYMBOLS,
            new LayerKey("#+=", "更多符号", LayerKeyKind.LAYER_TOGGLE, LAYER_EDGE_WEIGHT),
            chinese, chinesePunctuation,
            toolbarEmojiAvailable
                ? new LayerKey("符号", "切换符号键盘", LayerKeyKind.SYMBOL_PANEL, LAYER_EMOJI_WEIGHT)
                : new LayerKey("😀", "表情", LayerKeyKind.EMOJI, LAYER_EMOJI_WEIGHT),
            punctuationToggle(chinesePunctuation));
    }

    /**
     * 新设计的 #+= 层：[ ] { } # % ^ * + = / _ \ | ~ 《 》 € & · … / `123` + 五个标点 + ⌫ / 拼音（或 ABC）| 符号 | 空格 | ↵。左下原表情位是打开符号面板的「符号」键。
     */
    public static List<List<LayerKey>> moreSymbolLayer(boolean chinese) {
        return moreSymbolLayer(chinese, chinese);
    }

    /** 参数的含义同 {@link #numberLayer(boolean, boolean)}。 */
    public static List<List<LayerKey>> moreSymbolLayer(boolean chinese, boolean chinesePunctuation) {
        return layer(MORE_SYMBOLS_FIRST,
            chinesePunctuation ? CHINESE_MORE_SYMBOLS_SECOND : ENGLISH_MORE_SYMBOLS_SECOND,
            new LayerKey("123", "切换到数字和符号", LayerKeyKind.LAYER_TOGGLE, LAYER_EDGE_WEIGHT),
            chinese, chinesePunctuation, new LayerKey("符号", "切换符号键盘", LayerKeyKind.SYMBOL_PANEL,
                LAYER_EMOJI_WEIGHT), punctuationToggle(chinesePunctuation));
    }

    private static LayerKey punctuationToggle(boolean chinesePunctuation) {
        return new LayerKey(chinesePunctuation ? "中标" : "英标",
            chinesePunctuation ? "切换到英文标点" : "切换到中文标点",
            LayerKeyKind.PUNCTUATION_TOGGLE, LAYER_EMOJI_WEIGHT);
    }

    /** 层底行左下返回字母键盘的键面：中文「拼音」、英文「ABC」；描述都是「切换到字母键盘」。 */
    public static String lettersKeyTitle(boolean chinese) {
        return chinese ? "拼音" : "ABC";
    }

    private static List<List<LayerKey>> layer(List<String> first, List<String> second,
            LayerKey toggle, boolean chinese, boolean chinesePunctuation, LayerKey panelKey,
            LayerKey punctuationToggle) {
        List<String> punctuation = chinesePunctuation ? CHINESE_PUNCTUATION : ENGLISH_PUNCTUATION;
        List<LayerKey> third = new java.util.ArrayList<>(punctuation.size() + 2);
        third.add(toggle);
        for (String key : punctuation)
            third.add(character(key));
        third.add(new LayerKey("⌫", "删除", LayerKeyKind.DELETE, LAYER_EDGE_WEIGHT));
        List<LayerKey> bottom = List.of(
            new LayerKey(lettersKeyTitle(chinese), "切换到字母键盘", LayerKeyKind.LETTERS,
                LAYER_BACK_WEIGHT),
            panelKey,
            punctuationToggle,
            new LayerKey("空格", "空格", LayerKeyKind.SPACE, LAYER_SPACE_WEIGHT),
            new LayerKey("换行", "换行", LayerKeyKind.RETURN, LAYER_RETURN_WEIGHT));
        return List.of(characters(first), characters(second), List.copyOf(third), bottom);
    }

    private static List<LayerKey> characters(List<String> keys) {
        List<LayerKey> row = new java.util.ArrayList<>(keys.size());
        for (String key : keys) row.add(character(key));
        return List.copyOf(row);
    }

    private static LayerKey character(String key) {
        return new LayerKey(key, key, LayerKeyKind.CHARACTER, 1f);
    }
}

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

    private KeyboardLayout() {}

    /** Resolves the host surface from the Engine view without conflating Japanese nine-key. */
    public static int resolveTouchLayout(boolean handwriting, boolean nineKey, int scheme,
                                         String touchLayout) {
        // Korean has only the 26-key Dubeolsik keyboard, whatever layout the document carries.
        if (scheme == KoreanInputPolicy.KOREAN_SCHEME) return KOREAN_LAYOUT;
        // Zhuyin draws the nine-key bopomofo grid when the document stores the nine-key layout, the Dachen rows otherwise; it has no handwriting surface. The stored layout decides, as for Japanese, rather than the view's nine_key flag.
        if (scheme == InputSchemeTraits.ZHUYIN && "nine_key".equals(touchLayout)) return ZHUYIN_NINE_KEY_LAYOUT;
        if (scheme == InputSchemeTraits.ZHUYIN) return ZHUYIN_LAYOUT;
        if (handwriting || "handwriting".equals(touchLayout)) return HANDWRITING_LAYOUT;
        // 笔画方案画自己的笔画键盘：偏好是 26 键还是 9 键都一样，只有手写让给手写面板。
        if (scheme == InputSchemeTraits.STROKE) return STROKE_LAYOUT;
        if (scheme == 3 && "nine_key".equals(touchLayout)) return JAPANESE_NINE_KEY_LAYOUT;
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
    /** 层底行的宽度份额：返回字母 1.25、表情 / 符号 1.05、空格 6、回车 1.9，合计与字母层底行一样宽（10.2）。 */
    public static final float LAYER_BACK_WEIGHT = 1.25f;
    public static final float LAYER_EMOJI_WEIGHT = 1.05f;
    public static final float LAYER_SPACE_WEIGHT = 6f;
    public static final float LAYER_RETURN_WEIGHT = 1.9f;

    /**
     * 新设计的 123 层：1–0 / 十个符号 / `#+=` + 五个标点 + ⌫ / 拼音（或 ABC）| 😀 | 空格 | ↵。
     *
     * @param chinese 中文模式用全角符号与中文标点，英文模式用 ASCII 版，底行返回键为「ABC」
     */
    public static List<List<LayerKey>> numberLayer(boolean chinese) {
        return layer(NUMBER_DIGITS, chinese ? CHINESE_NUMBER_SYMBOLS : ENGLISH_NUMBER_SYMBOLS,
            new LayerKey("#+=", "更多符号", LayerKeyKind.LAYER_TOGGLE, LAYER_EDGE_WEIGHT),
            chinese, new LayerKey("😀", "表情", LayerKeyKind.EMOJI, LAYER_EMOJI_WEIGHT));
    }

    /**
     * 新设计的 #+= 层：[ ] { } # % ^ * + = / _ \ | ~ 《 》 € & · … / `123` + 五个标点 + ⌫ / 拼音（或 ABC）| 符号 | 空格 | ↵。左下原表情位是打开符号面板的「符号」键。
     */
    public static List<List<LayerKey>> moreSymbolLayer(boolean chinese) {
        return layer(MORE_SYMBOLS_FIRST,
            chinese ? CHINESE_MORE_SYMBOLS_SECOND : ENGLISH_MORE_SYMBOLS_SECOND,
            new LayerKey("123", "切换到数字和符号", LayerKeyKind.LAYER_TOGGLE, LAYER_EDGE_WEIGHT),
            chinese, new LayerKey("符号", "切换符号键盘", LayerKeyKind.SYMBOL_PANEL,
                LAYER_EMOJI_WEIGHT));
    }

    /** 层底行左下返回字母键盘的键面：中文「拼音」、英文「ABC」；描述都是「切换到字母键盘」。 */
    public static String lettersKeyTitle(boolean chinese) {
        return chinese ? "拼音" : "ABC";
    }

    private static List<List<LayerKey>> layer(List<String> first, List<String> second,
            LayerKey toggle, boolean chinese, LayerKey panelKey) {
        List<LayerKey> third = new java.util.ArrayList<>();
        third.add(toggle);
        for (String key : chinese ? CHINESE_PUNCTUATION : ENGLISH_PUNCTUATION)
            third.add(character(key));
        third.add(new LayerKey("⌫", "删除", LayerKeyKind.DELETE, LAYER_EDGE_WEIGHT));
        List<LayerKey> bottom = List.of(
            new LayerKey(lettersKeyTitle(chinese), "切换到字母键盘", LayerKeyKind.LETTERS,
                LAYER_BACK_WEIGHT),
            panelKey,
            new LayerKey("空格", "空格", LayerKeyKind.SPACE, LAYER_SPACE_WEIGHT),
            new LayerKey("换行", "换行", LayerKeyKind.RETURN, LAYER_RETURN_WEIGHT));
        return List.of(characters(first), characters(second), List.copyOf(third), bottom);
    }

    private static List<LayerKey> characters(List<String> keys) {
        List<LayerKey> row = new java.util.ArrayList<>();
        for (String key : keys) row.add(character(key));
        return List.copyOf(row);
    }

    private static LayerKey character(String key) {
        return new LayerKey(key, key, LayerKeyKind.CHARACTER, 1f);
    }
}

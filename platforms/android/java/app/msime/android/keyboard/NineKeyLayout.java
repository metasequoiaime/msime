package app.msime.android;

import java.util.List;

/** Display labels and Engine inputs for the Apple-compatible quanpin nine-key grid. */
public final class NineKeyLayout {
    /**
     * A grid key.
     *
     * <p>{@code digit} is what the key prints on the digit layer and what a hold offers; it is not
     * always {@code input}, because key 1 opens the symbol panel rather than feeding a number.
     */
    public record Key(int digit, String label, char input, String description) {}


    /** 1 键在拼音键面上是「@#」，点按打开符号面板，不送进引擎（引擎的九键不收 1）。组字时它改作「分词」，见 {@link #separatesSyllables}。 */
    public static boolean opensSymbols(Key key, boolean digits) {
        return !digits && key != null && key.digit() == 1;
    }

    /** 组字时 1 键是「分词」：在已打的数字末尾定一个音节分界（向引擎送 '）。和搜狗、讯飞九键的习惯一样，右列中间那格就留给「重输」。 */
    public static boolean separatesSyllables(Key key, boolean digits, boolean composing) {
        return composing && opensSymbols(key, digits);
    }

    /** 拼音键面上 1 键随组字状态换的键面。 */
    public static String face(Key key, boolean digits, boolean composing) {
        return separatesSyllables(key, digits, composing) ? "分词" : face(key, digits);
    }

    /** 与 {@link #face(Key, boolean, boolean)} 对应的无障碍描述。 */
    public static String description(Key key, boolean digits, boolean composing) {
        return separatesSyllables(key, digits, composing) ? "分词，在这里断开音节" : description(key, digits);
    }

    private static final List<List<Key>> ROWS = List.of(
        List.of(new Key(1, "@#", '@', "符号"), new Key(2, "ABC", '2', "2 ABC"),
            new Key(3, "DEF", '3', "3 DEF")),
        List.of(new Key(4, "GHI", '4', "4 GHI"), new Key(5, "JKL", '5', "5 JKL"),
            new Key(6, "MNO", '6', "6 MNO")),
        List.of(new Key(7, "PQRS", '7', "7 PQRS"), new Key(8, "TUV", '8', "8 TUV"),
            new Key(9, "WXYZ", '9', "9 WXYZ")));
    private static final List<String> PUNCTUATION = List.of("，", "。", "？", "！");

    private NineKeyLayout() {}

    public static List<List<Key>> rows() { return ROWS; }
    public static List<String> punctuation() { return PUNCTUATION; }

    /**
     * 九键网格在拼音键面与数字键面之间切换。
     *
     * <p>The same three columns carry both layers. Handing the digit layer over to the 26-key symbol
     * page would put a ten-across keypad under a keyboard the user picked for three columns, so the
     * grid keeps its geometry and only the legends change.
     */
    public static String face(Key key, boolean digits) {
        if (key == null) throw new IllegalArgumentException("Missing nine-key key");
        return digits ? String.valueOf(key.digit()) : key.label();
    }

    /** Accessibility description for the face {@link #face} prints. */
    public static String description(Key key, boolean digits) {
        if (key == null) throw new IllegalArgumentException("Missing nine-key key");
        return digits ? "数字 " + key.digit() : key.description();
    }

    /** Literal text a digit-layer tap commits instead of feeding the pinyin session. */
    public static String digitInput(Key key) {
        if (key == null) throw new IllegalArgumentException("Missing nine-key key");
        return String.valueOf(key.digit());
    }
}

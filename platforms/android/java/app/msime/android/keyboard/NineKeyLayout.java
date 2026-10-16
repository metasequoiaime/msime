package app.msime.android;

import java.util.List;

/** Display labels and Engine inputs for the Apple-compatible quanpin nine-key grid. */
public final class NineKeyLayout {
    /**
     * A grid key.
     *
     * <p>{@code digit} 是这个键在数字键面上印的数字，也是长按给出的数字；它不一定等于 {@code input}：1 键在拼音键面上把常用标点放进候选条，不送数字。
     */
    public record Key(int digit, String label, char input, String description) {}


    /** 1 键在拼音键面上是「@#」，点按把当前模式的常用标点放进候选条（点选上屏），不送进引擎（引擎的九键不收 1）；符号面板从底行最左边的 符 打开。组字时它改作「分词」，见 {@link #separatesSyllables}。 */
    public static boolean offersPunctuation(Key key, boolean digits) {
        return !digits && key != null && key.digit() == 1;
    }

    /** 组字时 1 键是「分词」：在已打的数字末尾定一个音节分界（向引擎送 '）。和搜狗、讯飞九键的习惯一样，右列中间那格就留给「重输」。 */
    public static boolean separatesSyllables(Key key, boolean digits, boolean composing) {
        return composing && offersPunctuation(key, digits);
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

    /** 九键外框里左侧符号栏和右侧 ⌫ 列各占的宽度份额；统计页的热力图按同一个份额画（{@link KeyHeatmapLayout}）。 */
    public static final float SIDE_COLUMN_WEIGHT = 0.9f;
    /** 九键外框中间 3×3 网格整体的宽度份额，三列均分。 */
    public static final float GRID_WEIGHT = 3f;

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

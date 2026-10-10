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
    /** 计算器顺序的数字键面：7 8 9 在最上，1 2 3 在最下；键本身（数字、无障碍描述、键位 id）不变，只换行序。 */
    private static final List<List<Key>> CALCULATOR_ROWS = List.of(ROWS.get(2), ROWS.get(1), ROWS.get(0));
    private static final List<String> PUNCTUATION = List.of("，", "。", "？", "！");

    /** 共享偏好里九键数字键面的排列。 */
    public static final String NUMBER_KEYPAD_ORDER_KEY = "touch_number_keypad_order";
    /** 电话顺序（默认）：1 2 3 在上。 */
    public static final String PHONE_ORDER = "phone";
    /** 计算器顺序：7 8 9 在上。 */
    public static final String CALCULATOR_ORDER = "calculator";

    /** 共享偏好里 26 键按「123」切到的数字层。 */
    public static final String TWENTY_SIX_KEY_NUMBER_LAYOUT_KEY = "touch_twenty_six_key_number_layout";
    /** 一行（默认）：新设计的 123 层，1 到 0 排成一行，下面是符号行。 */
    public static final String ROW_NUMBER_LAYOUT = "row";
    /** 九宫格：和拼音九键数字键面同一个键面（左侧符号栏、3×3 数字、右列删除 / 小数点 / 0）。 */
    public static final String NINE_KEY_NUMBER_LAYOUT = "nine_key";

    private NineKeyLayout() {}

    public static List<List<Key>> rows() { return ROWS; }

    /** 偏好值是不是计算器顺序；缺省和不认识的值按电话顺序。 */
    public static boolean calculatorOrder(String value) {
        return CALCULATOR_ORDER.equals(value);
    }

    /** 当前键面要画的三行：字母键面始终是 1 2 3 在上（ABC 在第一行），只有数字键面跟 `touch_number_keypad_order` 走。 */
    public static List<List<Key>> rows(boolean digits, boolean calculator) {
        return digits && calculator ? CALCULATOR_ROWS : ROWS;
    }

    /** 偏好值是不是九宫格；缺省和不认识的值按一行。 */
    public static boolean nineKeyNumberLayout(String value) {
        return NINE_KEY_NUMBER_LAYOUT.equals(value);
    }

    /**
     * 26 键的数字层此刻是否画成九键数字键面，而不是新设计的 123 层。
     *
     * <p>只有字母层是 26 个 QWERTY 键的界面才算：标准 26 键（全拼、双拼、五笔、英文、日文罗马字、粤拼、越南语、藏文）和韩文两套式。拼音九键本来就是九键数字键面；手写、笔画和注音 9 键的字母层是网格，大千注音的数字和标点键另有用途，日语九键自带数字层，这些都不受这个偏好影响。平板横屏画成分离式键盘时仍用 123 层：分离的意义是两半各贴一侧给拇指，三列网格铺满整个宽度反而两只手都够不着中间。
     *
     * @param touchLayout {@link KeyboardLayout} 的界面常量（英文模式下已经是 26 键）
     * @param symbols 当前是不是数字符号层
     * @param nineKeyNumberLayout 偏好 {@link #TWENTY_SIX_KEY_NUMBER_LAYOUT_KEY} 选的是九宫格
     * @param splitKeyboard 正在画分离式键盘（{@link SplitKeyboardPolicy#drawn}）
     */
    public static boolean twentySixKeyDigits(int touchLayout, boolean symbols, boolean nineKeyNumberLayout,
                                             boolean splitKeyboard) {
        return symbols && nineKeyNumberLayout && !splitKeyboard
            && (touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
                || touchLayout == KeyboardLayout.KOREAN_LAYOUT);
    }
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

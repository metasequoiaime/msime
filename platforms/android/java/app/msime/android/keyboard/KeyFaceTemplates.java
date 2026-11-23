package app.msime.android;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 键面模板的取值。比例按默认键高上的设计字号定（字母 22 sp、角标 10 sp、双拼提示 9 sp、功能键文字 15 sp、图标 22 dp），上下限防止矮键上的字小到认不出、平板和大键上的字无限放大。
 *
 * <p>默认键高就是一行键的高度 {@link KeyboardGeometry#KEY_ROW_HEIGHT_DP}（行距另加在行上，键本身 52 dp），不按设计稿里的 {@link KeyboardGeometry#DESIGN_KEY_HEIGHT_DP}：按 46 dp 算时默认键盘上的字整体大了约一成。
 */
public final class KeyFaceTemplates {
    private KeyFaceTemplates() { }

    /** 设计字号所在的键高（dp），比例都按它算。 */
    private static final float REFERENCE_KEY_HEIGHT_DP = KeyboardGeometry.KEY_ROW_HEIGHT_DP;

    private static KeyFaceTemplate.SlotSpec slot(float designSp, float minSp, float maxSp, float insetEm, int alpha) {
        return new KeyFaceTemplate.SlotSpec(designSp / REFERENCE_KEY_HEIGHT_DP, minSp, maxSp, insetEm, alpha);
    }

    /** 字母键的主标签：设计 22 sp，最小 12 sp（再小就认不出），最大 24 sp。 */
    static final KeyFaceTemplate.SlotSpec LETTER = slot(22, 12, 24, 0, 255);
    /** 右上角的符号：设计 10 sp，文字框离上边 0.3、离右边 0.5 个字号（10 sp 时 3 dp、5 dp，与原来的角标同一个位置）。 */
    static final KeyFaceTemplate.SlotSpec CORNER = new KeyFaceTemplate.SlotSpec(
        KeyHintButton.CORNER_HINT_SP / REFERENCE_KEY_HEIGHT_DP, 8, 12,
        KeyHintButton.CORNER_HINT_TOP_DP / KeyHintButton.CORNER_HINT_SP,
        KeyHintButton.CORNER_HINT_RIGHT_DP / KeyHintButton.CORNER_HINT_SP, 255);
    /** 底部的双拼提示：设计 9 sp，墨迹离下边 0.25 个字号，稍淡。 */
    static final KeyFaceTemplate.SlotSpec BOTTOM = slot(9, 7, 11, 0.25f, 204);

    /** 功能键（分词、重输）的文字：设计 {@link KeyboardTypographyPolicy#FUNCTION_KEY_TEXT_SP}（15 sp），最小 {@link KeyboardTypographyPolicy#FUNCTION_KEY_MIN_TEXT_SP}（9 sp，与底行功能键一样），最大 17 sp；另外不大过字母（{@link KeyFaceGroup#capMainAt}）。 */
    static final KeyFaceTemplate.SlotSpec FUNCTION_TEXT = slot(KeyboardTypographyPolicy.FUNCTION_KEY_TEXT_SP,
        KeyboardTypographyPolicy.FUNCTION_KEY_MIN_TEXT_SP, 17, 0, 255);
    /** 功能键的图标（⌫、⇧）：设计 {@link KeyboardIconKey#ICON_DP}，最小 16 dp，最大 26 dp。 */
    static final KeyFaceTemplate.IconSpec FUNCTION_ICON = new KeyFaceTemplate.IconSpec(
        KeyboardIconKey.ICON_DP / REFERENCE_KEY_HEIGHT_DP, 16, 26);

    /** 数字行的 1–0：设计 20 sp，最小 12 sp，最大 22 sp。 */
    static final KeyFaceTemplate.SlotSpec NUMBER = slot(20, 12, 22, 0, 255);

    /** 拼音九键的字母组（「ABC」…）：设计 18 sp，最小 12 sp，最大 20 sp。 */
    static final KeyFaceTemplate.SlotSpec NINE_LETTER = slot(18, 12, 20, 0, 255);
    /** 数字九宫格和九键双拼的数字：设计 22 sp，最小 12 sp，最大 24 sp，与字母键相同。 */
    static final KeyFaceTemplate.SlotSpec NINE_DIGIT = slot(22, 12, 24, 0, 255);
    /**
     * 九键右上角的数字：字号和文字框离上边的距离与字母键角标相同，离右边 1.2 个字号（10 sp 时 12 dp），与原来的九键数字同一个位置；键帽轮廓在那里收进去时再沿对角线挪进来（{@link KeyOutline}）。
     */
    static final KeyFaceTemplate.SlotSpec NINE_CORNER = new KeyFaceTemplate.SlotSpec(
        KeyHintButton.CORNER_HINT_SP / REFERENCE_KEY_HEIGHT_DP, 8, 12,
        KeyHintButton.CORNER_HINT_TOP_DP / KeyHintButton.CORNER_HINT_SP,
        NineKeyDigitButton.DIGIT_HINT_RIGHT_DP / KeyHintButton.CORNER_HINT_SP, 255);

    private static Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> letterSlots() {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, LETTER);
        slots.put(KeyFaceSlot.TOP_RIGHT, CORNER);
        slots.put(KeyFaceSlot.BOTTOM, BOTTOM);
        return slots;
    }

    /**
     * 字母键碰上角标时：先保持居中、最多缩小 15%（大约是字体缩放封顶 1.15 倍多出来的那部分），再左移让出右上角，最后才排到角标下方。全组用同一种，按组里最窄的键定；排到角标下方会让一排字母整体下沉，所以放在最后。
     */
    private static final List<KeyFaceTemplate.Strategy> LETTER_STRATEGIES = List.of(
        KeyFaceTemplate.Strategy.CENTERED, KeyFaceTemplate.Strategy.SHRINK_MAIN,
        KeyFaceTemplate.Strategy.SIDE_BY_SIDE, KeyFaceTemplate.Strategy.STACKED);

    /** 居中时主标签最多缩小多少。 */
    private static final float LETTER_MAX_SHRINK = 0.15f;

    /** 26 键（和同一套字母行的其他语言）的字母键：主标签、右上角符号、底部双拼提示。 */
    public static final KeyFaceTemplate LETTERS = new KeyFaceTemplate("letters", letterSlots(),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, 0, true, LETTER_MAX_SHRINK, LETTER_STRATEGIES);

    /** 14/18 键的字母键：同字母键，两个字母的键面分开画，中间隔 0.15 个字号。 */
    public static final KeyFaceTemplate GRID_LETTERS = new KeyFaceTemplate("grid-letters", letterSlots(),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, FourteenKeyLayout.TWO_LETTER_SPACING_EM, true,
        LETTER_MAX_SHRINK, LETTER_STRATEGIES);

    /**
     * 键区里的功能键（⇧、⌫、14/18 键的分词键、九键右列的 ⌫ 和重输）：主标签是图标或一两个字，右上角可能有「辅」。汉字按整个墨迹居中，所以和同一排按大写字母高居中的字母落在同一条中线上；退让方式与字母键相同。
     */
    public static final KeyFaceTemplate FUNCTIONS = new KeyFaceTemplate("functions", functionSlots(),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, 0, false, LETTER_MAX_SHRINK, LETTER_STRATEGIES,
        FUNCTION_ICON);

    private static Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> functionSlots() {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, FUNCTION_TEXT);
        slots.put(KeyFaceSlot.TOP_RIGHT, CORNER);
        return slots;
    }

    /** 26 键一族和 14/18 键字母上方的数字行（#6022）：只有主标签，数字按大写字母高居中（数字与大写字母同高）。 */
    public static final KeyFaceTemplate NUMBER_ROW = new KeyFaceTemplate("number-row", nineSlots(NUMBER, false, false),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, 0, true, LETTER_MAX_SHRINK, LETTER_STRATEGIES);

    /** 拼音九键字母层：字母组（1 键组字时是「分词」或「重输」）、右上角长按输入的数字；退让方式与字母键相同。 */
    public static final KeyFaceTemplate NINE_LETTERS = new KeyFaceTemplate("nine-letters", nineSlots(NINE_LETTER, true, false),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, 0, true, LETTER_MAX_SHRINK, LETTER_STRATEGIES);

    /** 数字九宫格和九键双拼的数字键：数字作主标签，九键双拼底部是简短的声韵提示。 */
    public static final KeyFaceTemplate NINE_DIGITS = new KeyFaceTemplate("nine-digits", nineSlots(NINE_DIGIT, false, true),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, 0, true, LETTER_MAX_SHRINK, LETTER_STRATEGIES);

    /** 九键、笔画和数字九宫格左侧符号栏的一格有多高时用设计字号（dp）：三行键高分成一屏 {@link NineKeySidebarPolicy#VISIBLE_ROWS} 格。 */
    private static final float SIDE_ROW_REFERENCE_DP = 3 * REFERENCE_KEY_HEIGHT_DP / NineKeySidebarPolicy.VISIBLE_ROWS;

    /** 符号栏的符号：设计 {@link KeyboardTypographyPolicy#NINE_KEY_SIDE_TEXT_SP}（18 sp，原来的固定字号），随格高变化，最小 12 sp；最大也是 18 sp，格子更高时不再放大，与原来一样。 */
    static final KeyFaceTemplate.SlotSpec SIDE_SYMBOL = new KeyFaceTemplate.SlotSpec(
        KeyboardTypographyPolicy.NINE_KEY_SIDE_TEXT_SP / SIDE_ROW_REFERENCE_DP, 12, KeyboardTypographyPolicy.NINE_KEY_SIDE_TEXT_SP,
        0, 255);
    /** 符号栏里 ！ 右上角的 0（大回车外框，{@link SuperscriptShortcutPolicy#ZERO}）：在设计格高上与九键右上角的数字同字号、同位置。 */
    static final KeyFaceTemplate.SlotSpec SIDE_CORNER = new KeyFaceTemplate.SlotSpec(
        KeyHintButton.CORNER_HINT_SP / SIDE_ROW_REFERENCE_DP, 8, 12,
        KeyHintButton.CORNER_HINT_TOP_DP / KeyHintButton.CORNER_HINT_SP,
        NineKeyDigitButton.DIGIT_HINT_RIGHT_DP / KeyHintButton.CORNER_HINT_SP, 255);
    /**
     * 全角「，」「。」「、」的墨迹只占字身左下角一小块：按字号的 1.3 倍画、按自己的墨迹居中，再往下挪 0.1 个字号，与原来的 {@link CenteredGlyphSpan} 相同（它把墨迹中点放在行盒中线以下行高的 8%，行高约 1.2 个字号）。
     */
    static final KeyFaceTemplate.Glyphs SMALL_PUNCTUATION = new KeyFaceTemplate.Glyphs(
        Map.of("，", 1.3f, "。", 1.3f, "、", 1.3f), 0.1f);

    /**
     * 九键、笔画和数字九宫格左侧的符号栏：每格一个符号（表可自定义），按每个符号自己的墨迹居中（{@link KeyFaceTemplate.Glyphs}），字号全栏一个；大回车外框接在末尾的 ！ 右上角印 0。
     */
    public static final KeyFaceTemplate SIDE_SYMBOLS = new KeyFaceTemplate("side-symbols", sideSlots(),
        KeyboardTypographyPolicy.KEY_CAP_HORIZONTAL_PADDING_DP, 1, 0, false, LETTER_MAX_SHRINK, LETTER_STRATEGIES, null,
        SMALL_PUNCTUATION);

    private static Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> sideSlots() {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, SIDE_SYMBOL);
        slots.put(KeyFaceSlot.TOP_RIGHT, SIDE_CORNER);
        return slots;
    }

    private static Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> nineSlots(KeyFaceTemplate.SlotSpec main, boolean corner,
            boolean bottom) {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, main);
        if (corner) slots.put(KeyFaceSlot.TOP_RIGHT, NINE_CORNER);
        if (bottom) slots.put(KeyFaceSlot.BOTTOM, BOTTOM);
        return slots;
    }
}

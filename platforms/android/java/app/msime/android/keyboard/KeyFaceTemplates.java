package app.msime.android;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 键面模板的取值。比例按默认键高上的设计字号定（字母 22 sp、角标 10 sp、双拼提示 9 sp），上下限防止矮键上的字小到认不出、平板和大键上的字无限放大。
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
    /** 右上角的符号：设计 10 sp，墨迹离上边、右边 0.3 个字号（10 sp 时 3 dp）。 */
    static final KeyFaceTemplate.SlotSpec CORNER = slot(10, 8, 12, 0.3f, 255);
    /** 底部的双拼提示：设计 9 sp，墨迹离下边 0.25 个字号，稍淡。 */
    static final KeyFaceTemplate.SlotSpec BOTTOM = slot(9, 7, 11, 0.25f, 204);

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
}

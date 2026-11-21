package app.msime.android;

import java.util.Collections;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 一类键的键面模板：用到哪些位置（{@link KeyFaceSlot}），每个位置的字号范围和离键边的距离，主标签离两侧多远，提示与主标签之间至少隔多远，以及放不下时依次尝试的退让方式。
 *
 * <p>模板只是数据，算法在 {@link KeyFaceSolver}。同一键组的键共用一个模板（{@link KeyFaceGroup}）。
 *
 * @param slots 用到的位置；必须有 {@link KeyFaceSlot#MAIN}
 * @param sideInsetDp 主标签离键格左右两边至少多远
 * @param gapDp 主标签与提示的墨迹之间至少隔多远
 * @param pairGapEm 两个字母的键面（18 键的 {@code WE}）两字母之间的间距，按主标签字号的倍数；0 表示整串照常排
 * @param capAligned 主标签按基线以上的墨迹（大写字母高）竖直居中，基线以下的部分（Q、分号的尾巴）往下挂，全组共用一条基线；否则按整个墨迹居中（汉字、符号）
 * @param maxShrink {@link Strategy#SHRINK_MAIN} 最多把主标签缩小多少（按放得下时的字号的比例，0.15 即最多缩小 15%）；再小就试下一种退让
 * @param strategies 居中放不下时依次尝试的退让方式；都不行时隐藏提示
 */
public record KeyFaceTemplate(String name, Map<KeyFaceSlot, SlotSpec> slots, float sideInsetDp, float gapDp,
        float pairGapEm, boolean capAligned, float maxShrink, List<Strategy> strategies) {

    public KeyFaceTemplate {
        if (!slots.containsKey(KeyFaceSlot.MAIN)) throw new IllegalArgumentException(name + " has no MAIN slot");
        slots = Collections.unmodifiableMap(new EnumMap<>(slots));
        strategies = List.copyOf(strategies);
    }

    /**
     * 一个位置的字号规则：首选字号随键格高度按 {@code heightRatio} 变化，夹在 {@code minSp} 和 {@code maxSp} 之间（都按键盘的字体缩放换成像素，见 {@link KeyFaceEnvironment}）。
     *
     * @param insetEm 提示的墨迹离键边多远，按这个位置自己的字号的倍数，所以随字号一起缩放；主标签不用它
     * @param alpha 绘制时的不透明度（0–255）
     */
    public record SlotSpec(float heightRatio, float minSp, float maxSp, float insetEm, int alpha) {
        public SlotSpec {
            if (!(heightRatio > 0) || !(minSp > 0) || maxSp < minSp)
                throw new IllegalArgumentException("bad slot spec " + heightRatio + " " + minSp + " " + maxSp);
        }
    }

    /** 主标签和右上角提示在居中时相碰后的退让方式，按模板列出的顺序尝试。 */
    public enum Strategy {
        /** 两者不相碰，主标签照常居中。 */
        CENTERED,
        /** 主标签向左移开，让出右上角。 */
        SIDE_BY_SIDE,
        /** 主标签排到提示下方的剩余高度里，字号按剩余高度重新收。 */
        STACKED,
        /** 主标签保持居中，缩小到不相碰为止，但不小于最小字号，也不超过模板的 {@code maxShrink}。 */
        SHRINK_MAIN,
        /** 不画右上角提示，主标签照常居中。 */
        HIDE_HINTS
    }

    public SlotSpec slot(KeyFaceSlot slot) { return slots.get(slot); }

    public boolean has(KeyFaceSlot slot) { return slots.containsKey(slot); }
}

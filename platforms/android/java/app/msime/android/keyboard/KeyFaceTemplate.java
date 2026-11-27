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
 * @param icon 画图标的键（⌫、⇧）的图标尺寸；{@code null} 表示这类键不画图标
 * @param glyphs 主标签按每个键自己的墨迹居中（符号栏的标点、声调键），见 {@link Glyphs}；{@code null} 表示全组共用一条基线
 */
public record KeyFaceTemplate(String name, Map<KeyFaceSlot, SlotSpec> slots, float sideInsetDp, float gapDp,
        float pairGapEm, boolean capAligned, float maxShrink, List<Strategy> strategies, IconSpec icon, Glyphs glyphs) {

    public KeyFaceTemplate {
        if (!slots.containsKey(KeyFaceSlot.MAIN)) throw new IllegalArgumentException(name + " has no MAIN slot");
        requireGlyphAlignment(name, capAligned, pairGapEm, glyphs);
        slots = Collections.unmodifiableMap(new EnumMap<>(slots));
        strategies = List.copyOf(strategies);
    }

    /**
     * 按自己的墨迹居中的模板不按大写字母高对齐、不分开画两个字母。放在静态方法里：Android lint 的 API 检查在 record 紧凑构造函数里遇到带 `||` 的条件会崩（找不到紧凑构造函数的参数表）。
     */
    private static void requireGlyphAlignment(String name, boolean capAligned, float pairGapEm, Glyphs glyphs) {
        if (glyphs == null) return;
        if (capAligned || pairGapEm > 0)
            throw new IllegalArgumentException(name + " centres glyphs on their own ink: no cap alignment, no letter pairs");
    }

    /** 主标签全组共用一条基线的模板。 */
    public KeyFaceTemplate(String name, Map<KeyFaceSlot, SlotSpec> slots, float sideInsetDp, float gapDp,
            float pairGapEm, boolean capAligned, float maxShrink, List<Strategy> strategies, IconSpec icon) {
        this(name, slots, sideInsetDp, gapDp, pairGapEm, capAligned, maxShrink, strategies, icon, null);
    }

    /** 不画图标的模板。 */
    public KeyFaceTemplate(String name, Map<KeyFaceSlot, SlotSpec> slots, float sideInsetDp, float gapDp,
            float pairGapEm, boolean capAligned, float maxShrink, List<Strategy> strategies) {
        this(name, slots, sideInsetDp, gapDp, pairGapEm, capAligned, maxShrink, strategies, null);
    }

    /**
     * 一个位置的字号规则：首选字号随键格高度按 {@code heightRatio} 变化，夹在 {@code minSp} 和 {@code maxSp} 之间（都按键盘的字体缩放换成像素，见 {@link KeyFaceEnvironment}）。
     *
     * @param insetEm 底部提示的墨迹离下边多远；右上角提示的文字框（字体上行线）离上边多远。按这个位置自己的字号的倍数，所以随字号一起缩放；主标签不用它
     * @param insetXEm 右上角提示的文字框（排版宽度的右端）离右边多远，同样按字号的倍数（九键数字比字母角标离右边远）；键帽轮廓在那里收进去时由 {@link KeyOutline} 再往里挪
     * @param alpha 绘制时的不透明度（0–255）
     */
    public record SlotSpec(float heightRatio, float minSp, float maxSp, float insetEm, float insetXEm, int alpha) {
        public SlotSpec {
            if (!(heightRatio > 0) || !(minSp > 0) || maxSp < minSp)
                throw new IllegalArgumentException("bad slot spec " + heightRatio + " " + minSp + " " + maxSp);
        }

        /** 离上下边和右边一样远。 */
        public SlotSpec(float heightRatio, float minSp, float maxSp, float insetEm, int alpha) {
            this(heightRatio, minSp, maxSp, insetEm, insetEm, alpha);
        }
    }

    /**
     * 图标的尺寸规则：边长随键格高度按 {@code heightRatio} 变化，夹在 {@code minDp} 和 {@code maxDp} 之间，再不超过键格的短边（去掉两侧的 {@code sideInsetDp}）。图标不跟字体缩放，与原来的 22 dp 图标一样。{@code inkTop}、{@code inkRight} 是图标墨迹的上沿和右沿在方框里的位置（按边长的比例）：图标和右上角提示隔不隔得开按墨迹算，不按整个方框（↵ 的墨迹离方框右上角还有一截，正是「换行」的位置）。
     */
    public record IconSpec(float heightRatio, float minDp, float maxDp, float inkTop, float inkRight) {
        public IconSpec {
            if (!(heightRatio > 0) || !(minDp > 0) || maxDp < minDp || inkTop < 0 || inkTop >= .5f || inkRight <= .5f || inkRight > 1)
                throw new IllegalArgumentException("bad icon spec " + heightRatio + " " + minDp + " " + maxDp);
        }

        /** 图标墨迹占满整个方框（核对右上角提示时按整个方框算）。 */
        public IconSpec(float heightRatio, float minDp, float maxDp) {
            this(heightRatio, minDp, maxDp, 0, 1);
        }
    }

    /**
     * 主标签按自己的墨迹居中（水平、竖直都是），字号仍是全组一个（D6 的例外，只用于标点和声调这类墨迹位置各不相同的键）：全角「，」「。」只占字身左下角一小块，声调符号只是字身上方一道短笔画，和别的字共用一条基线时会贴底、飘在上面或小得看不见。
     *
     * <p>{@code every} 为真时组里每段文字都按自己的墨迹居中（符号栏）；为假时只有 {@code scales} 里登记的文字这样画，其余的照常共用全组的基线（大千、注音九键的声调键和同一组里的音符键）。登记的文字按字号再放大 {@code scales} 倍，墨迹中点比主标签区中点低 {@code dropEm} 个字号（设计稿里逗号、句号落在键面中线稍下方，与原来的 {@link CenteredGlyphSpan} 一样）；{@code every} 为真时没登记的文字按 1 倍、正中。内容域按放大、下移后的墨迹量，所以它们同样放得进键格。
     */
    public record Glyphs(Map<String, Float> scales, float dropEm, boolean every) {
        public Glyphs {
            for (Map.Entry<String, Float> entry : scales.entrySet())
                if (!(entry.getValue() > 0)) throw new IllegalArgumentException("bad glyph scale " + entry);
            if (dropEm < 0) throw new IllegalArgumentException("bad glyph drop " + dropEm);
            // 不用 Map.copyOf（API 30 起才有）：D8 为它补的方法会改变设备测试包里合并补丁类的名字，与应用里 R8 留下的同名类相撞（API 35 上测试进程因此崩溃）。
            scales = Collections.unmodifiableMap(new java.util.HashMap<>(scales));
        }

        /** 每段文字都按自己的墨迹居中。 */
        public Glyphs(Map<String, Float> scales, float dropEm) {
            this(scales, dropEm, true);
        }

        /** {@code text} 是否按自己的墨迹居中。 */
        public boolean centres(String text) {
            return every || text != null && scales.containsKey(text);
        }

        /** {@code text} 画的时候是字号的几倍。 */
        public float scale(String text) {
            Float scale = text == null ? null : scales.get(text);
            return scale == null ? 1f : scale;
        }

        /** {@code text} 的墨迹中点在字号 {@code sizePx}（组的字号，不乘放大倍数）下比主标签区中点低多少像素。 */
        public float drop(String text, float sizePx) {
            return text != null && scales.containsKey(text) ? dropEm * sizePx : 0;
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

package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

/**
 * 按模板、量好的内容域和键格尺寸算出一组键的 {@link KeyFaceParams}。纯函数：不量字（字宽、墨迹都由 {@link KeyFaceDomain#measure} 事先按基准字号量好，这里按比例换算），所以每次键格变了重算也只是几步算术。
 *
 * <p>规则：
 * <ul>
 * <li>每个位置的首选字号随键格高度变化，和上下限一样跟随键盘的字体缩放，夹在上下限之间；提示按组里带这个位置的最小键格算。</li>
 * <li>主标签按组里最小的键格算，并为内容域里所有可能出现的提示留位置：底部提示在下方占一条，主标签在剩下的区域里按最宽的文字和墨迹的高度收字号。收到最小字号以下时先放弃底部提示。</li>
 * <li>右上角提示与居中的主标签按墨迹（再加模板的间距）判断碰不碰；碰了就按模板的顺序尝试退让，都不行就不画右上角提示。</li>
 * <li>键帽轮廓（{@link KeyFaceEnvironment#outline}）：主标签的宽度扣掉轮廓在它那几行收进来的部分（票根的缺口、胶囊的两头）；右上角提示在轮廓收进去的地方沿对角线挪进键帽，按组里带它的最小和最大键格里挪得多的那个判断碰撞，哪个键格里都放不下就不画；底部提示离开凸起键帽的厚度。</li>
 * <li>主标签的首选字号可以另有上限（功能键的字不大过同一副键盘的字母）；图标的边长按键高比例、夹在模板的上下限之间，不跟字体缩放。</li>
 * </ul>
 */
public final class KeyFaceSolver {
    private KeyFaceSolver() { }

    /** 键格尺寸（像素）。 */
    public record Cell(int width, int height) { }

    /** 主标签要让出的轮廓按它在键格中线上下各占多少个字号的几行算。 */
    private static final float MAIN_BAND_EM = 0.4f;

    /** 主标签不另设上限，见 {@link #solve(KeyFaceTemplate, Map, KeyFaceEnvironment, Map, Map, float)}。 */
    public static KeyFaceParams solve(KeyFaceTemplate template, Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics,
            KeyFaceEnvironment environment, Map<KeyFaceSlot, Cell> cells) {
        return solve(template, metrics, environment, cells, Float.POSITIVE_INFINITY);
    }

    /** 组里的键一样大（最大键格就是最小键格）。 */
    public static KeyFaceParams solve(KeyFaceTemplate template, Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics,
            KeyFaceEnvironment environment, Map<KeyFaceSlot, Cell> cells, float mainCapPx) {
        return solve(template, metrics, environment, cells, cells, mainCapPx);
    }

    /**
     * @param cells 每个位置按带这个位置的键里最小的宽和最小的高；必须有 {@link KeyFaceSlot#MAIN}
     * @param largest 每个位置按带它的键里最大的宽和最大的高：卵石这样随键宽缩放的轮廓在大键上收得更多，右上角提示挪得更远
     * @param mainCapPx 主标签的首选字号最多多大（像素），例如功能键的字不大过同一副键盘的字母（{@link KeyFaceGroup#capMainAt}）；不限时为正无穷
     */
    public static KeyFaceParams solve(KeyFaceTemplate template, Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics,
            KeyFaceEnvironment environment, Map<KeyFaceSlot, Cell> cells, Map<KeyFaceSlot, Cell> largest,
            float mainCapPx) {
        Cell cell = cells.get(KeyFaceSlot.MAIN);
        KeyOutline outline = environment.outline();
        KeyFaceDomain.SlotMetrics main = metrics.get(KeyFaceSlot.MAIN);
        KeyFaceTemplate.SlotSpec spec = template.slot(KeyFaceSlot.MAIN);
        float side = environment.px(template.sideInsetDp());
        float gap = environment.px(template.gapDp());
        float minimum = environment.sp(spec.minSp());
        float preferred = Math.min(mainCapPx, clamp(spec.heightRatio() * cell.height() * environment.fontScale(),
            minimum, environment.sp(spec.maxSp())));
        float icon = icon(template, environment, cell, side);
        // 主标签两侧至少让出模板的边距，轮廓在主标签那几行收进来更多时让出更多（票根的缺口、胶囊的两头）。
        float band = preferred * MAIN_BAND_EM;
        float mainSide = Math.max(side, outline.none() ? 0
            : outline.spans(cell.width(), cell.height()).sideInset(cell.height() / 2f - band, cell.height() / 2f + band));
        KeyFaceParams.Hint bottom = hint(template, metrics, environment, cells, KeyFaceSlot.BOTTOM, side);
        KeyFaceParams.Hint corner = hint(template, metrics, environment, cells, KeyFaceSlot.TOP_RIGHT, side);
        float bottomInset = bottom == null ? 0 : bottom.insetPx() + bottom.inkHeight() + gap;
        float size = fit(preferred, main, cell.width() - 2 * mainSide, cell.height() - bottomInset, template.capAligned());
        if (size < minimum && bottom != null) {
            bottom = null;
            bottomInset = 0;
            size = fit(preferred, main, cell.width() - 2 * mainSide, cell.height(), template.capAligned());
        }
        if (corner == null || main == null)
            return params(outline, cell, template, icon, KeyFaceTemplate.Strategy.CENTERED, size, 0, 0, bottomInset, main, null, bottom);
        KeyFaceDomain.SlotMetrics hint = metrics.get(KeyFaceSlot.TOP_RIGHT);
        // 右上角提示按文字框定位：排版宽度的右端离右边 insetX，上行线离上边 inset；碰撞按墨迹算。
        float cornerRight = cell.width() - corner.insetXPx();
        float cornerLeft = cornerRight - hint.width(corner.sizePx());
        float cornerTop = corner.topBaseline() + corner.inkTopPx();
        float cornerBottom = corner.topBaseline() + corner.inkBottomPx();
        if (!outline.none()) {
            float offset = cornerOffset(outline, cells.get(KeyFaceSlot.TOP_RIGHT), corner, cornerTop,
                hint.width(corner.sizePx()));
            if (Float.isNaN(offset))
                return params(outline, cell, template, icon, KeyFaceTemplate.Strategy.HIDE_HINTS, size, 0, 0, bottomInset, main, null,
                    bottom);
            float wide = cornerOffset(outline, largest.get(KeyFaceSlot.TOP_RIGHT), corner, cornerTop,
                hint.width(corner.sizePx()));
            if (!Float.isNaN(wide)) offset = Math.max(offset, wide);
            cornerLeft -= offset;
            cornerBottom += offset;
        }
        List<KeyFaceTemplate.Strategy> order = new ArrayList<>(template.strategies());
        order.add(KeyFaceTemplate.Strategy.HIDE_HINTS);
        for (KeyFaceTemplate.Strategy strategy : order) {
            switch (strategy) {
                case CENTERED -> {
                    if (clear(cell, main, size, 0, 0, bottomInset, cornerLeft, cornerBottom, gap,
                            template.capAligned()))
                        return params(outline, cell, template, icon, strategy, size, 0, 0, bottomInset, main, corner, bottom);
                }
                case SIDE_BY_SIDE -> {
                    float right = (cell.width() + main.width(size)) / 2f;
                    float shift = right + gap - cornerLeft;
                    if (cell.width() / 2f - main.width(size) / 2f - shift >= mainSide)
                        return params(outline, cell, template, icon, strategy, size, -Math.max(0, shift), 0, bottomInset, main, corner,
                            bottom);
                }
                case STACKED -> {
                    float top = cornerBottom + gap;
                    float stacked = fit(preferred, main, cell.width() - 2 * mainSide, cell.height() - top - bottomInset,
                        template.capAligned());
                    if (stacked >= minimum)
                        return params(outline, cell, template, icon, strategy, stacked, 0, top, bottomInset, main, corner, bottom);
                }
                case SHRINK_MAIN -> {
                    boolean capAligned = template.capAligned();
                    float floor = Math.max(minimum, size * (1 - template.maxShrink()));
                    if (floor <= size
                            && clear(cell, main, floor, 0, 0, bottomInset, cornerLeft, cornerBottom, gap, capAligned)) {
                        float low = floor, high = size;
                        for (int step = 0; step < 24; step++) {
                            float middle = (low + high) / 2f;
                            if (clear(cell, main, middle, 0, 0, bottomInset, cornerLeft, cornerBottom, gap, capAligned))
                                low = middle;
                            else high = middle;
                        }
                        return params(outline, cell, template, icon, strategy, low, 0, 0, bottomInset, main, corner, bottom);
                    }
                }
                case HIDE_HINTS -> {
                    return params(outline, cell, template, icon, strategy, size, 0, 0, bottomInset, main, null, bottom);
                }
            }
        }
        throw new AssertionError("HIDE_HINTS always resolves");
    }

    /** 一处提示的字号和边距；模板或内容域不用这个位置、或组里没有带它的键时为 {@code null}。底部提示按键宽（扣掉轮廓在那几行收进来的部分）收字号、离开凸起键帽的厚度，右上角提示最多占键宽的一半。 */
    private static KeyFaceParams.Hint hint(KeyFaceTemplate template, Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics,
            KeyFaceEnvironment environment, Map<KeyFaceSlot, Cell> cells, KeyFaceSlot slot, float side) {
        KeyFaceTemplate.SlotSpec spec = template.slot(slot);
        KeyFaceDomain.SlotMetrics measured = metrics.get(slot);
        Cell cell = cells.get(slot);
        if (spec == null || measured == null || cell == null) return null;
        float size = clamp(spec.heightRatio() * cell.height() * environment.fontScale(), environment.sp(spec.minSp()),
            environment.sp(spec.maxSp()));
        float limit = slot == KeyFaceSlot.BOTTOM ? cell.width() - 2 * side : cell.width() / 2f;
        float wide = slot == KeyFaceSlot.BOTTOM ? measured.width(size) : measured.inkWidth(size);
        if (wide > limit && wide > 0) size = Math.max(0, size * limit / wide);
        KeyOutline outline = environment.outline();
        float lift = slot == KeyFaceSlot.BOTTOM ? outline.bottomDepthPx() : 0;
        if (slot == KeyFaceSlot.BOTTOM && !outline.none()) {
            // 底部提示整条（最宽的文字、墨迹高）要落在键帽里：先往上挪（键帽下端是圆弧时最下面几行窄或者没有，窄而高的胶囊下端是个半圆），挪到键高的四分之一还不行就缩小一点再从原位试。主标签随后按挪过的位置让出底部那一条。
            KeyOutline.Spans spans = outline.spans(cell.width(), cell.height());
            float highest = cell.height() / 4f;
            float raise = Float.NaN;
            for (int shrink = 0; shrink < 30 && size > 0 && Float.isNaN(raise); shrink++) {
                for (float up = 0; up <= highest; up += 0.5f) {
                    float bottomEdge = cell.height() - spec.insetEm() * size - lift - up;
                    float width = measured.width(size);
                    float left = (cell.width() - width) / 2f;
                    if (spans.fits(left, bottomEdge - measured.inkHeight(size), left + width, bottomEdge)) {
                        raise = up;
                        break;
                    }
                }
                if (Float.isNaN(raise)) size *= 0.97f;
            }
            if (!Float.isNaN(raise)) lift += raise;
        }
        return new KeyFaceParams.Hint(size, spec.insetEm() * size + lift, spec.insetXEm() * size, measured.inkTop(size),
            measured.inkBottom(size), measured.ascent(size));
    }

    /** 首选字号收到放得下：最宽的文字不超过 {@code width}，墨迹的高度不超过 {@code height}。内容域没有主标签文字（只画图标的键）时原样返回。 */
    private static float fit(float preferred, KeyFaceDomain.SlotMetrics main, float width, float height,
            boolean capAligned) {
        if (main == null) return preferred;
        float size = preferred;
        if (main.width() > 0) size = Math.min(size, width * KeyFaceDomain.REFERENCE_PX / main.width());
        float tall = main.alignedHeight(KeyFaceDomain.REFERENCE_PX, capAligned);
        if (tall > 0) size = Math.min(size, height * KeyFaceDomain.REFERENCE_PX / tall);
        return Math.max(0, size);
    }

    /** 右上角提示（排版宽度 {@code width}、上沿 {@code top}）在 {@code cell} 里要沿对角线挪多少才落进键帽；没有这样的键格时为 0，放不下时为 NaN。 */
    private static float cornerOffset(KeyOutline outline, Cell cell, KeyFaceParams.Hint corner, float top, float width) {
        if (cell == null) return 0;
        return outline.spans(cell.width(), cell.height()).cornerOffset(cell.width() - corner.insetXPx(), top, width,
            corner.inkHeight());
    }

    /** 居中（再偏 {@code shift}）的主标签和右上角提示的墨迹是否上下或左右隔开至少 {@code gap}。 */
    private static boolean clear(Cell cell, KeyFaceDomain.SlotMetrics main, float size, float shift, float top,
            float bottomInset, float cornerLeft, float cornerBottom, float gap, boolean capAligned) {
        KeyFaceParams.Main face = new KeyFaceParams.Main(size, shift, top, bottomInset, main.inkTop(size),
            main.inkBottom(size), capAligned);
        float inkTop = face.baseline(cell.height()) + face.inkTopPx();
        float right = (cell.width() + main.width(size)) / 2f + shift;
        return inkTop >= cornerBottom + gap || right + gap <= cornerLeft;
    }

    private static KeyFaceParams params(KeyOutline outline, Cell cell, KeyFaceTemplate template, float icon,
            KeyFaceTemplate.Strategy strategy, float size, float shift, float top, float bottomInset,
            KeyFaceDomain.SlotMetrics main, KeyFaceParams.Hint corner, KeyFaceParams.Hint bottom) {
        size = insideOutline(outline, cell, main, size, shift, top, bottomInset, template.capAligned());
        float inkTop = main == null ? 0 : main.inkTop(size);
        float inkBottom = main == null ? 0 : main.inkBottom(size);
        return new KeyFaceParams(strategy,
            new KeyFaceParams.Main(size, shift, top, bottomInset, inkTop, inkBottom, template.capAligned()), corner, bottom,
            icon);
    }

    /**
     * 主标签按退让方式摆好以后，在它实际占的几行里还出键帽轮廓（排到角标下方时离圆角近、凸起键帽的面更矮）就再缩小，直到每一行都在轮廓里。缩小只会让主标签离角标更远，不改变退让方式的判断。
     */
    private static float insideOutline(KeyOutline outline, Cell cell, KeyFaceDomain.SlotMetrics main, float size,
            float shift, float top, float bottomInset, boolean capAligned) {
        if (outline.none() || main == null) return size;
        KeyOutline.Spans spans = outline.spans(cell.width(), cell.height());
        for (int step = 0; step < 40 && size > 0; step++) {
            KeyFaceParams.Main face = new KeyFaceParams.Main(size, shift, top, bottomInset, main.inkTop(size),
                main.inkBottom(size), capAligned);
            float baseline = face.baseline(cell.height());
            float left = (cell.width() - main.width(size)) / 2f + shift;
            if (spans.fits(left, baseline + face.inkTopPx(), left + main.width(size), baseline + face.inkBottomPx()))
                return size;
            size *= 0.97f;
        }
        return size;
    }

    /** 图标边长：按键高比例、夹在上下限之间，再不超过键格的短边（去掉两侧边距）；模板不画图标时为 0。 */
    private static float icon(KeyFaceTemplate template, KeyFaceEnvironment environment, Cell cell, float side) {
        KeyFaceTemplate.IconSpec spec = template.icon();
        if (spec == null) return 0;
        float size = clamp(spec.heightRatio() * cell.height(), environment.px(spec.minDp()), environment.px(spec.maxDp()));
        return Math.max(0, Math.min(size, Math.min(cell.width() - 2 * side, cell.height())));
    }

    private static float clamp(float value, float low, float high) {
        return Math.max(low, Math.min(high, value));
    }
}

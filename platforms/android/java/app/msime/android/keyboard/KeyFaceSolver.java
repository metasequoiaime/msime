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
 * <li>主标签的首选字号可以另有上限（功能键的字不大过同一副键盘的字母）；图标的边长按键高比例、夹在模板的上下限之间，不跟字体缩放。</li>
 * </ul>
 */
public final class KeyFaceSolver {
    private KeyFaceSolver() { }

    /** 键格尺寸（像素）。 */
    public record Cell(int width, int height) { }

    /** 主标签不另设上限，见 {@link #solve(KeyFaceTemplate, Map, KeyFaceEnvironment, Map, float)}。 */
    public static KeyFaceParams solve(KeyFaceTemplate template, Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics,
            KeyFaceEnvironment environment, Map<KeyFaceSlot, Cell> cells) {
        return solve(template, metrics, environment, cells, Float.POSITIVE_INFINITY);
    }

    /**
     * @param cells 每个位置按带这个位置的键里最小的宽和最小的高；必须有 {@link KeyFaceSlot#MAIN}
     * @param mainCapPx 主标签的首选字号最多多大（像素），例如功能键的字不大过同一副键盘的字母（{@link KeyFaceGroup#capMainAt}）；不限时为正无穷
     */
    public static KeyFaceParams solve(KeyFaceTemplate template, Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics,
            KeyFaceEnvironment environment, Map<KeyFaceSlot, Cell> cells, float mainCapPx) {
        Cell cell = cells.get(KeyFaceSlot.MAIN);
        KeyFaceDomain.SlotMetrics main = metrics.get(KeyFaceSlot.MAIN);
        KeyFaceTemplate.SlotSpec spec = template.slot(KeyFaceSlot.MAIN);
        float side = environment.px(template.sideInsetDp());
        float gap = environment.px(template.gapDp());
        float minimum = environment.sp(spec.minSp());
        float preferred = Math.min(mainCapPx, clamp(spec.heightRatio() * cell.height() * environment.fontScale(),
            minimum, environment.sp(spec.maxSp())));
        float icon = icon(template, environment, cell, side);
        KeyFaceParams.Hint bottom = hint(template, metrics, environment, cells, KeyFaceSlot.BOTTOM, side);
        KeyFaceParams.Hint corner = hint(template, metrics, environment, cells, KeyFaceSlot.TOP_RIGHT, side);
        float bottomInset = bottom == null ? 0 : bottom.insetPx() + bottom.inkHeight() + gap;
        float size = fit(preferred, main, cell.width() - 2 * side, cell.height() - bottomInset, template.capAligned());
        if (size < minimum && bottom != null) {
            bottom = null;
            bottomInset = 0;
            size = fit(preferred, main, cell.width() - 2 * side, cell.height(), template.capAligned());
        }
        if (corner == null || main == null)
            return params(template, icon, KeyFaceTemplate.Strategy.CENTERED, size, 0, 0, bottomInset, main, null, bottom);
        KeyFaceDomain.SlotMetrics hint = metrics.get(KeyFaceSlot.TOP_RIGHT);
        float cornerRight = cell.width() - corner.insetXPx();
        float cornerLeft = cornerRight - hint.inkWidth(corner.sizePx());
        float cornerBottom = corner.insetPx() + corner.inkHeight();
        List<KeyFaceTemplate.Strategy> order = new ArrayList<>(template.strategies());
        order.add(KeyFaceTemplate.Strategy.HIDE_HINTS);
        for (KeyFaceTemplate.Strategy strategy : order) {
            switch (strategy) {
                case CENTERED -> {
                    if (clear(cell, main, size, 0, 0, bottomInset, cornerLeft, cornerBottom, gap,
                            template.capAligned()))
                        return params(template, icon, strategy, size, 0, 0, bottomInset, main, corner, bottom);
                }
                case SIDE_BY_SIDE -> {
                    float right = (cell.width() + main.width(size)) / 2f;
                    float shift = right + gap - cornerLeft;
                    if (cell.width() / 2f - main.width(size) / 2f - shift >= side)
                        return params(template, icon, strategy, size, -Math.max(0, shift), 0, bottomInset, main, corner,
                            bottom);
                }
                case STACKED -> {
                    float top = cornerBottom + gap;
                    float stacked = fit(preferred, main, cell.width() - 2 * side, cell.height() - top - bottomInset,
                        template.capAligned());
                    if (stacked >= minimum)
                        return params(template, icon, strategy, stacked, 0, top, bottomInset, main, corner, bottom);
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
                        return params(template, icon, strategy, low, 0, 0, bottomInset, main, corner, bottom);
                    }
                }
                case HIDE_HINTS -> {
                    return params(template, icon, strategy, size, 0, 0, bottomInset, main, null, bottom);
                }
            }
        }
        throw new AssertionError("HIDE_HINTS always resolves");
    }

    /** 一处提示的字号和边距；模板或内容域不用这个位置、或组里没有带它的键时为 {@code null}。底部提示按键宽收字号，右上角提示最多占键宽的一半。 */
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
        return new KeyFaceParams.Hint(size, spec.insetEm() * size, spec.insetXEm() * size, measured.inkTop(size),
            measured.inkBottom(size));
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

    /** 居中（再偏 {@code shift}）的主标签和右上角提示的墨迹是否上下或左右隔开至少 {@code gap}。 */
    private static boolean clear(Cell cell, KeyFaceDomain.SlotMetrics main, float size, float shift, float top,
            float bottomInset, float cornerLeft, float cornerBottom, float gap, boolean capAligned) {
        KeyFaceParams.Main face = new KeyFaceParams.Main(size, shift, top, bottomInset, main.inkTop(size),
            main.inkBottom(size), capAligned);
        float inkTop = face.baseline(cell.height()) + face.inkTopPx();
        float right = (cell.width() + main.width(size)) / 2f + shift;
        return inkTop >= cornerBottom + gap || right + gap <= cornerLeft;
    }

    private static KeyFaceParams params(KeyFaceTemplate template, float icon, KeyFaceTemplate.Strategy strategy,
            float size, float shift, float top, float bottomInset, KeyFaceDomain.SlotMetrics main,
            KeyFaceParams.Hint corner, KeyFaceParams.Hint bottom) {
        float inkTop = main == null ? 0 : main.inkTop(size);
        float inkBottom = main == null ? 0 : main.inkBottom(size);
        return new KeyFaceParams(strategy,
            new KeyFaceParams.Main(size, shift, top, bottomInset, inkTop, inkBottom, template.capAligned()), corner, bottom,
            icon);
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

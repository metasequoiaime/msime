package app.msime.android;

import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/** 键面排版规则：字号随键高变化并夹在上下限之间，按内容域留位置，提示与主标签的墨迹不相碰，放不下时按模板的顺序退让。 */
public final class KeyFaceSolverSmoke {
    private KeyFaceSolverSmoke() { }

    static final KeyFaceTemplate.SlotSpec MAIN = new KeyFaceTemplate.SlotSpec(0.48f, 12, 26, 0, 255);
    static final KeyFaceTemplate.SlotSpec CORNER = new KeyFaceTemplate.SlotSpec(0.22f, 8, 12, 0.3f, 255);
    static final KeyFaceTemplate.SlotSpec BOTTOM = new KeyFaceTemplate.SlotSpec(0.2f, 7, 10, 0.2f, 204);
    static final List<KeyFaceTemplate.Strategy> ORDER = List.of(KeyFaceTemplate.Strategy.CENTERED,
        KeyFaceTemplate.Strategy.SIDE_BY_SIDE, KeyFaceTemplate.Strategy.STACKED, KeyFaceTemplate.Strategy.SHRINK_MAIN);
    static final KeyFaceEnvironment ENVIRONMENT = new KeyFaceEnvironment(3f, 1f);

    /** 假字体：字宽和墨迹都是字号的固定倍数，W 最宽；墨迹从基线到 0.7 个字号高，Q 的尾巴挂到基线下 0.2 个字号。 */
    static final KeyFaceMeasurer MEASURER = new KeyFaceMeasurer() {
        @Override public float advance(KeyFaceSlot slot, String text, float sizePx) {
            float width = 0;
            for (char c : text.toCharArray()) width += (c == 'W' || c == 'M' ? 0.9f : 0.6f) * sizePx;
            return slot == KeyFaceSlot.MAIN ? width : width * 0.8f;
        }

        @Override public Ink ink(KeyFaceSlot slot, String text, float sizePx) {
            float below = text.contains("Q") ? 0.2f * sizePx : 0;
            return new Ink(0.05f * sizePx, -0.7f * sizePx, advance(slot, text, sizePx) - 0.05f * sizePx, below);
        }

        @Override public float ascent(KeyFaceSlot slot, float sizePx) { return -0.9f * sizePx; }
    };

    static KeyFaceTemplate template(boolean corner, boolean bottom) {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, MAIN);
        if (corner) slots.put(KeyFaceSlot.TOP_RIGHT, CORNER);
        if (bottom) slots.put(KeyFaceSlot.BOTTOM, BOTTOM);
        return new KeyFaceTemplate("smoke", slots, 2, 1, 0, true, 0.15f, ORDER);
    }

    static KeyFaceDomain domain(boolean corner, boolean bottom) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, List.of("A", "Q", "W", "M"));
        if (corner) texts.put(KeyFaceSlot.TOP_RIGHT, List.of("!", "@", "（"));
        if (bottom) texts.put(KeyFaceSlot.BOTTOM, List.of("iu", "ong"));
        return new KeyFaceDomain(texts);
    }

    static KeyFaceParams solve(boolean corner, boolean bottom, KeyFaceEnvironment environment, int width, int height) {
        KeyFaceTemplate template = template(corner, bottom);
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        for (KeyFaceSlot slot : template.slots().keySet()) cells.put(slot, new KeyFaceSolver.Cell(width, height));
        return KeyFaceSolver.solve(template, domain(corner, bottom).measure(MEASURER, 0), environment, cells);
    }

    /** 假字体里的全角逗号：墨迹只占字身左下角一小块（基线附近），和其他字差得很远。 */
    static final KeyFaceMeasurer PUNCTUATION = new KeyFaceMeasurer() {
        @Override public float advance(KeyFaceSlot slot, String text, float sizePx) { return sizePx; }

        @Override public Ink ink(KeyFaceSlot slot, String text, float sizePx) {
            if ("，".equals(text)) return new Ink(0.1f * sizePx, -0.15f * sizePx, 0.3f * sizePx, 0.15f * sizePx);
            if ("……".equals(text)) return new Ink(0.1f * sizePx, -0.45f * sizePx, 1.9f * sizePx, -0.3f * sizePx);
            return new Ink(0.2f * sizePx, -0.75f * sizePx, 0.8f * sizePx, 0);
        }

        @Override public float ascent(KeyFaceSlot slot, float sizePx) { return -0.9f * sizePx; }
    };

    /** 按自己的墨迹居中（KeyFaceTemplate.Glyphs）：字号全组一个，每个字的墨迹中点落在主标签区中点上（登记的字再往下挪 0.1 个字号），放大的字和最宽的字都落在键格里。 */
    static void glyphCentring() {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, MAIN);
        KeyFaceTemplate.Glyphs glyphs = new KeyFaceTemplate.Glyphs(Map.of("，", 1.3f), 0.1f);
        KeyFaceTemplate template = new KeyFaceTemplate("glyphs", slots, 2, 1, 0, false, 0.15f, ORDER, null, glyphs);
        List<String> texts = List.of("，", "？", "……");
        Map<KeyFaceSlot, List<String>> domainTexts = new EnumMap<>(KeyFaceSlot.class);
        domainTexts.put(KeyFaceSlot.MAIN, texts);
        Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics = new KeyFaceDomain(domainTexts).measure(PUNCTUATION, 0, glyphs);
        KeyFaceDomain.SlotMetrics main = metrics.get(KeyFaceSlot.MAIN);
        // 最宽的墨迹是 ……（1.8 个字号），最高的是 ？（0.75 个字号）；框以基线为中线。
        check(near(main.width(), 180) && near(main.inkHeight(), 75) && near(main.inkTop(), -37.5f), "按墨迹量的框不对：" + main);
        for (int[] cell : new int[][] {{160, 93}, {60, 93}, {160, 40}}) {
            Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = Map.of(KeyFaceSlot.MAIN, new KeyFaceSolver.Cell(cell[0], cell[1]));
            KeyFaceParams params = KeyFaceSolver.solve(template, metrics, ENVIRONMENT, cells);
            float size = params.main().sizePx();
            for (String text : texts) {
                float drawn = size * glyphs.scale(text);
                KeyFaceMeasurer.Ink ink = PUNCTUATION.ink(KeyFaceSlot.MAIN, text, drawn);
                float x = params.main().glyphX(cell[0], ink.left(), ink.right());
                // 与 KeyFaceRenderer 相同：按墨迹居中的基线，登记的字再往下挪。
                float baseline = params.main().glyphBaseline(cell[1], ink.top(), ink.bottom()) + glyphs.drop(text, size);
                String where = text + " in " + cell[0] + "x" + cell[1];
                check(Math.abs(x + (ink.left() + ink.right()) / 2f - cell[0] / 2f) < 0.01f, "墨迹应水平居中：" + where);
                check(Math.abs(baseline - glyphs.drop(text, size) + (ink.top() + ink.bottom()) / 2f - cell[1] / 2f) < 0.01f,
                    "墨迹应竖直居中（登记的字再往下挪）：" + where);
                check(x + ink.left() >= ENVIRONMENT.px(2) - 0.5f && x + ink.right() <= cell[0] - ENVIRONMENT.px(2) + 0.5f
                    && baseline + ink.top() >= -0.5f && baseline + ink.bottom() <= cell[1] + 0.5f, "墨迹出了键格：" + where);
            }
        }
        // 只有逗号时：放大以后墨迹高 0.39 个字号，往下挪 0.1 个字号，框上下各留 0.195 + 0.1 个字号。
        check(near(new KeyFaceDomain(Map.of(KeyFaceSlot.MAIN, List.of("，"))).measure(PUNCTUATION, 0, glyphs)
            .get(KeyFaceSlot.MAIN).inkHeight(), 59), "放大的字应按放大、下移后的墨迹量");
        check(glyphs.drop("？", 100) == 0, "没登记的字不往下挪");
        try {
            new KeyFaceTemplate("bad", slots, 2, 1, 0, true, 0.15f, ORDER, null, glyphs);
            throw new AssertionError("按自己的墨迹居中的模板不应同时按大写字母高对齐");
        } catch (IllegalArgumentException expected) { }
    }

    public static void main(String[] args) {
        glyphCentring();
        // 字号随键高变化，夹在上下限之间。
        check(near(solve(false, false, ENVIRONMENT, 300, 150).main().sizePx(), 0.48f * 150), "首选字号应按键高比例");
        check(near(solve(false, false, ENVIRONMENT, 600, 400).main().sizePx(), 26 * 3), "大键应封顶在最大字号");
        check(near(solve(false, false, new KeyFaceEnvironment(3f, 1.15f), 600, 400).main().sizePx(), 26 * 3 * 1.15f),
            "上下限应跟随字体缩放");
        // 按大写字母高居中：基线以上的墨迹的中线落在键格中线上，Q 的尾巴往下挂。
        KeyFaceParams centered = solve(false, false, ENVIRONMENT, 300, 150);
        float capCenter = centered.main().baseline(150) + centered.main().inkTopPx() / 2f;
        check(Math.abs(capCenter - 75) < 0.01f, "大写字母应在键格中线上：" + capCenter);
        // 窄键按最宽的字收字号：W 在基准字号下宽 90。
        float narrow = solve(false, false, ENVIRONMENT, 60, 150).main().sizePx();
        check(near(narrow, (60 - 12) * KeyFaceDomain.REFERENCE_PX / 90f), "窄键应按最宽的主标签收字号：" + narrow);

        // 内容域里有提示就为它留位置：底部提示占一条，主标签区变矮。
        KeyFaceParams plain = solve(false, false, ENVIRONMENT, 120, 120);
        KeyFaceParams withBottom = solve(false, true, ENVIRONMENT, 120, 120);
        check(withBottom.bottom() != null && withBottom.main().boxBottomInsetPx() > 0, "底部提示应在下方占一条");
        check(withBottom.main().baseline(120) < plain.main().baseline(120), "留出底部提示后主标签应上移");
        // 键太小、主标签会小于最小字号时，先放弃底部提示。
        KeyFaceParams tiny = solve(false, true, ENVIRONMENT, 200, 40);
        check(tiny.bottom() == null, "主标签放不下最小字号时应先放弃底部提示");
        check(solve(false, true, ENVIRONMENT, 200, 70).bottom() != null, "主标签放得下最小字号时应保留底部提示");

        // 宽键：角标在右上角，居中的主标签碰不到。
        check(solve(true, false, ENVIRONMENT, 300, 150).strategy() == KeyFaceTemplate.Strategy.CENTERED, "宽键应照常居中");
        // 缩小主标签有上限：居中缩小超过 15% 才能避开时改试别的退让。
        KeyFaceTemplate shrinkFirst = new KeyFaceTemplate("shrink-first", template(true, false).slots(), 2, 1, 0, true, 0.15f,
            List.of(KeyFaceTemplate.Strategy.SHRINK_MAIN, KeyFaceTemplate.Strategy.STACKED));
        for (int width = 60; width <= 330; width += 10) {
            Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
            for (KeyFaceSlot slot : shrinkFirst.slots().keySet()) cells.put(slot, new KeyFaceSolver.Cell(width, 150));
            KeyFaceParams shrunk = KeyFaceSolver.solve(shrinkFirst, domain(true, false).measure(MEASURER, 0), ENVIRONMENT, cells);
            KeyFaceParams free = solve(false, false, ENVIRONMENT, width, 150);
            if (shrunk.strategy() == KeyFaceTemplate.Strategy.SHRINK_MAIN)
                check(shrunk.main().sizePx() >= free.main().sizePx() * 0.85f - 0.01f, "居中缩小超过了上限：" + width);
        }

        // 各种尺寸下：画出来的东西不相碰、不出键格。
        int strategies = 0;
        boolean[] seen = new boolean[KeyFaceTemplate.Strategy.values().length];
        for (int width = 60; width <= 330; width += 10) {
            for (int height = 60; height <= 200; height += 10) {
                for (boolean bottom : new boolean[] {false, true}) {
                    KeyFaceParams params = solve(true, bottom, ENVIRONMENT, width, height);
                    seen[params.strategy().ordinal()] = true;
                    verify(params, bottom, width, height);
                }
            }
        }
        for (boolean value : seen) if (value) strategies++;
        check(strategies >= 3, "尺寸网格应走到至少三种退让方式，实际 " + strategies);

        // 主标签另有上限时首选字号不超过它；不画图标的模板图标边长为 0。
        Map<KeyFaceSlot, KeyFaceSolver.Cell> wideCells = new EnumMap<>(KeyFaceSlot.class);
        wideCells.put(KeyFaceSlot.MAIN, new KeyFaceSolver.Cell(300, 150));
        KeyFaceParams capped = KeyFaceSolver.solve(template(false, false), domain(false, false).measure(MEASURER, 0),
            ENVIRONMENT, wideCells, 40);
        check(near(capped.main().sizePx(), 40), "主标签应封顶在给定的上限：" + capped.main().sizePx());
        check(capped.iconPx() == 0, "不画图标的模板不应有图标边长");

        // 图标边长按键高比例、夹在上下限之间，不跟字体缩放，也不超过键格的短边。
        KeyFaceTemplate icons = new KeyFaceTemplate("icons", template(false, false).slots(), 2, 1, 0, false, 0.15f, ORDER,
            new KeyFaceTemplate.IconSpec(0.4f, 16, 26));
        check(near(icon(icons, ENVIRONMENT, 300, 150), 0.4f * 150), "图标边长应按键高比例");
        check(near(icon(icons, new KeyFaceEnvironment(3f, 1.15f), 300, 150), 0.4f * 150), "图标不应跟字体缩放");
        check(near(icon(icons, ENVIRONMENT, 300, 400), 26 * 3), "大键上的图标应封顶");
        check(near(icon(icons, ENVIRONMENT, 300, 60), 16 * 3), "矮键上的图标不应小于下限");
        check(near(icon(icons, ENVIRONMENT, 40, 150), 40 - 2 * 6), "窄键上的图标不应超出左右边距");

                // 两个字母的键面按「左字 + 间距 + 右字」算宽度。
        float pair = KeyFaceDomain.mainWidth(MEASURER, "WE", 100, 0.15f);
        check(near(pair, 90 + 15 + 60), "双字母宽度应含间距：" + pair);
        check(near(KeyFaceDomain.mainWidth(MEASURER, "WE", 100, 0), 150), "没有间距时应整串排");
        // 键帽轮廓：各种键形和尺寸下，右上角提示挪进键帽后仍不压住主标签，主标签不出轮廓；凸起键帽的底部提示离开厚度。
        int moved = 0;
        for (KeyOutline.Shape shape : KeyOutline.Shape.values()) {
            for (float depth : new float[] {0, 9}) {
                KeyOutline outline = new KeyOutline(shape, 30, 3, depth, 6);
                KeyFaceEnvironment framed = new KeyFaceEnvironment(3f, 1f, outline);
                for (int width = 60; width <= 330; width += 30) {
                    for (int height = 120; height <= 240; height += 20) {
                        for (boolean bottom : new boolean[] {false, true}) {
                            KeyFaceParams params = solve(true, bottom, framed, width, height);
                            moved += verifyOutline(params, bottom, outline, width, height);
                            KeyFaceParams.Hint under = params.bottom();
                            if (under != null) {
                                String at = shape + " " + width + "×" + height;
                                check(under.insetPx() - BOTTOM.insetEm() * under.sizePx() >= depth - 0.01f,
                                    "凸起键帽的底部提示应离开厚度：" + at);
                                float hintWidth = domain(true, true).measure(MEASURER, 0).get(KeyFaceSlot.BOTTOM)
                                    .width(under.sizePx());
                                float edge = height - under.insetPx();
                                check(outline.spans(width, height).fits((width - hintWidth) / 2f, edge - under.inkHeight(),
                                    (width + hintWidth) / 2f, edge), "底部提示出了键帽轮廓：" + at);
                            }
                        }
                    }
                }
            }
        }
        check(moved > 0, "轮廓网格里应有挪进键帽的角标");
        System.out.println("KeyFaceSolverSmoke passed");
    }

    /** 在轮廓里按绘制规则（与 {@link KeyFaceRenderer} 相同）摆出主标签和挪过的角标，检查都在键帽里、互不相碰；返回角标是否挪过（0 或 1）。 */
    private static int verifyOutline(KeyFaceParams params, boolean bottom, KeyOutline outline, int width, int height) {
        String where = outline.shape() + " 深 " + outline.bottomDepthPx() + " " + width + "×" + height
            + (bottom ? " 带底部提示" : "") + " " + params.strategy();
        KeyOutline.Spans spans = outline.spans(width, height);
        Map<KeyFaceSlot, KeyFaceDomain.SlotMetrics> metrics = domain(true, bottom).measure(MEASURER, 0);
        KeyFaceDomain.SlotMetrics main = metrics.get(KeyFaceSlot.MAIN);
        KeyFaceParams.Main face = params.main();
        float size = face.sizePx();
        float left = (width - main.width(size)) / 2f + face.shiftXPx();
        float right = left + main.width(size);
        float baseline = face.baseline(height);
        float inkTop = baseline + face.inkTopPx(), inkBottom = baseline + face.inkBottomPx();
        check(spans.fits(left, inkTop, right, inkBottom), "主标签出了键帽轮廓：" + where);
        KeyFaceParams.Hint corner = params.topRight();
        if (corner == null) return 0;
        KeyFaceDomain.SlotMetrics hint = metrics.get(KeyFaceSlot.TOP_RIGHT);
        float boxRight = width - corner.insetXPx(), boxWidth = hint.width(corner.sizePx());
        float boxTop = corner.topBaseline() + corner.inkTopPx();
        float offset = spans.cornerOffset(boxRight, boxTop, boxWidth, corner.inkHeight());
        check(!Float.isNaN(offset), "画出来的角标应放得进键帽：" + where);
        float gap = ENVIRONMENT.px(1);
        float cornerLeft = boxRight - boxWidth - offset, cornerBottom = boxTop + corner.inkHeight() + offset;
        check(inkTop >= cornerBottom + gap - 0.5f || right + gap <= cornerLeft + 0.5f, "挪过的角标压住主标签：" + where);
        check(inkBottom <= height, "主标签出了键格：" + where);
        return offset > 0 ? 1 : 0;
    }

    private static float icon(KeyFaceTemplate template, KeyFaceEnvironment environment, int width, int height) {
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        cells.put(KeyFaceSlot.MAIN, new KeyFaceSolver.Cell(width, height));
        return KeyFaceSolver.solve(template, domain(false, false).measure(MEASURER, 0), environment, cells).iconPx();
    }

    /** 主标签在键格里、不压住提示：按与 {@link KeyFaceParams} 说明相同的绘制规则算出墨迹框再比较。 */
    private static void verify(KeyFaceParams params, boolean bottom, int width, int height) {
        String where = width + "×" + height + (bottom ? " 带底部提示" : "") + " " + params.strategy();
        KeyFaceDomain.SlotMetrics main = domain(true, bottom).measure(MEASURER, 0).get(KeyFaceSlot.MAIN);
        KeyFaceParams.Main face = params.main();
        float size = face.sizePx();
        check(size > 0, "主标签字号应为正：" + where);
        float left = (width - main.width(size)) / 2f + face.shiftXPx();
        float right = left + main.width(size);
        float baseline = face.baseline(height);
        float inkTop = baseline + face.inkTopPx();
        float inkBottom = baseline + face.inkBottomPx();
        float side = ENVIRONMENT.px(2), gap = ENVIRONMENT.px(1);
        check(left >= side - 0.5f && right <= width - side + 0.5f, "主标签超出左右边距：" + where);
        check(inkTop >= face.boxTopPx() - 0.5f && inkBottom <= height - face.boxBottomInsetPx() + 0.5f,
            "主标签超出主标签区：" + where);
        KeyFaceParams.Hint corner = params.topRight();
        if (corner != null) {
            KeyFaceDomain.SlotMetrics hint = domain(true, bottom).measure(MEASURER, 0).get(KeyFaceSlot.TOP_RIGHT);
            float cornerLeft = width - corner.insetXPx() - hint.width(corner.sizePx());
            float cornerBottom = corner.topBaseline() + corner.inkBottomPx();
            check(inkTop >= cornerBottom + gap - 0.5f || right + gap <= cornerLeft + 0.5f, "主标签压住角标：" + where);
        } else {
            check(params.strategy() == KeyFaceTemplate.Strategy.HIDE_HINTS, "角标只应在隐藏退让时缺席：" + where);
        }
        KeyFaceParams.Hint under = params.bottom();
        if (under != null) {
            KeyFaceDomain.SlotMetrics hint = domain(true, bottom).measure(MEASURER, 0).get(KeyFaceSlot.BOTTOM);
            check(hint.width(under.sizePx()) <= width - 2 * side + 0.5f, "底部提示超出键宽：" + where);
            check(inkBottom <= height - under.insetPx() - under.inkHeight() + 0.5f, "主标签压住底部提示：" + where);
        }
    }

    static boolean near(float actual, float expected) {
        return Math.abs(actual - expected) < 0.01f;
    }

    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

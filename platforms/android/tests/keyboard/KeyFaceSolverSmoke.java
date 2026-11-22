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

    public static void main(String[] args) {
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
        System.out.println("KeyFaceSolverSmoke passed");
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
            float cornerLeft = width - corner.insetXPx() - hint.inkWidth(corner.sizePx());
            float cornerBottom = corner.insetPx() + corner.inkHeight();
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

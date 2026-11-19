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

    /** 假字体：字宽和墨迹都是字号的固定倍数，W 最宽；墨迹从基线到 0.7 个字号高。 */
    static final KeyFaceMeasurer MEASURER = new KeyFaceMeasurer() {
        @Override public float advance(KeyFaceSlot slot, String text, float sizePx) {
            float width = 0;
            for (char c : text.toCharArray()) width += (c == 'W' || c == 'M' ? 0.9f : 0.6f) * sizePx;
            return slot == KeyFaceSlot.MAIN ? width : width * 0.8f;
        }

        @Override public Ink ink(KeyFaceSlot slot, String text, float sizePx) {
            return new Ink(0.05f * sizePx, -0.7f * sizePx, advance(slot, text, sizePx) - 0.05f * sizePx, 0);
        }
    };

    static KeyFaceTemplate template(boolean corner, boolean bottom) {
        Map<KeyFaceSlot, KeyFaceTemplate.SlotSpec> slots = new EnumMap<>(KeyFaceSlot.class);
        slots.put(KeyFaceSlot.MAIN, MAIN);
        if (corner) slots.put(KeyFaceSlot.TOP_RIGHT, CORNER);
        if (bottom) slots.put(KeyFaceSlot.BOTTOM, BOTTOM);
        return new KeyFaceTemplate("smoke", slots, 2, 1, 0, ORDER);
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
        check(solve(false, true, ENVIRONMENT, 200, 60).bottom() != null, "主标签放得下最小字号时应保留底部提示");

        // 宽键：角标在右上角，居中的主标签碰不到。
        check(solve(true, false, ENVIRONMENT, 300, 150).strategy() == KeyFaceTemplate.Strategy.CENTERED, "宽键应照常居中");

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

        // 两个字母的键面按「左字 + 间距 + 右字」算宽度。
        float pair = KeyFaceDomain.mainWidth(MEASURER, "WE", 100, 0.15f);
        check(near(pair, 90 + 15 + 60), "双字母宽度应含间距：" + pair);
        check(near(KeyFaceDomain.mainWidth(MEASURER, "WE", 100, 0), 150), "没有间距时应整串排");
        System.out.println("KeyFaceSolverSmoke passed");
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
            float cornerLeft = width - corner.insetPx() - hint.inkWidth(corner.sizePx());
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

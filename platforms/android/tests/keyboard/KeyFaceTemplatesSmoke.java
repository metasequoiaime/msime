import app.msime.android.FourteenKeyLayout;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceEnvironment;
import app.msime.android.KeyFaceMeasurer;
import app.msime.android.KeyFaceParams;
import app.msime.android.KeyFaceSlot;
import app.msime.android.KeyFaceSolver;
import app.msime.android.KeyFaceTemplate;
import app.msime.android.KeyFaceTemplates;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/** 字母键模板的取值：默认 52 dp 键上是设计字号（字母 22、角标 10、双拼提示 9 sp），随字体缩放，退让顺序先居中缩小、最后才上下分区。 */
public final class KeyFaceTemplatesSmoke {
    private KeyFaceTemplatesSmoke() { }

    /** 窄字形的假字体，键宽不会限制字号。 */
    private static final KeyFaceMeasurer NARROW = new KeyFaceMeasurer() {
        @Override public float advance(KeyFaceSlot slot, String text, float sizePx) { return 0.1f * sizePx * text.length(); }

        @Override public Ink ink(KeyFaceSlot slot, String text, float sizePx) {
            return new Ink(0, -0.5f * sizePx, advance(slot, text, sizePx), 0);
        }
    };

    public static void main(String[] args) {
        for (KeyFaceTemplate template : List.of(KeyFaceTemplates.LETTERS, KeyFaceTemplates.GRID_LETTERS)) {
            KeyFaceParams design = solve(template, new KeyFaceEnvironment(1, 1), 400, 52);
            check(near(design.main().sizePx(), 22), template.name() + " 默认键高上的字母应为 22 sp：" + design.main().sizePx());
            check(near(design.topRight().sizePx(), 10), template.name() + " 角标应为 10 sp：" + design.topRight().sizePx());
            check(near(design.bottom().sizePx(), 9), template.name() + " 双拼提示应为 9 sp：" + design.bottom().sizePx());
            KeyFaceParams scaled = solve(template, new KeyFaceEnvironment(1, 1.15f), 400, 52);
            check(near(scaled.main().sizePx(), 22 * 1.15f), template.name() + " 应跟随字体缩放：" + scaled.main().sizePx());
            check(near(solve(template, new KeyFaceEnvironment(1, 1), 400, 400).main().sizePx(), 24),
                template.name() + " 大键应封顶在 24 sp");
            check(near(solve(template, new KeyFaceEnvironment(1, 1), 400, 20).main().sizePx(), 12),
                template.name() + " 矮键不应小于 12 sp");
            check(template.capAligned(), template.name() + " 字母应按大写字母高居中");
            check(template.strategies().equals(List.of(KeyFaceTemplate.Strategy.CENTERED, KeyFaceTemplate.Strategy.SHRINK_MAIN,
                KeyFaceTemplate.Strategy.SIDE_BY_SIDE, KeyFaceTemplate.Strategy.STACKED)), template.name() + " 退让顺序变了");
            check(near(template.maxShrink(), 0.15f), template.name() + " 居中缩小上限应为 15%");
        }
        check(KeyFaceTemplates.LETTERS.pairGapEm() == 0, "26 键字母不应分开画");
        check(KeyFaceTemplates.GRID_LETTERS.pairGapEm() == FourteenKeyLayout.TWO_LETTER_SPACING_EM,
            "14/18 键双字母的间距应与键面字距相同");
        System.out.println("KeyFaceTemplatesSmoke passed");
    }

    private static KeyFaceParams solve(KeyFaceTemplate template, KeyFaceEnvironment environment, int width, int height) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, List.of("A"));
        texts.put(KeyFaceSlot.TOP_RIGHT, List.of("1"));
        texts.put(KeyFaceSlot.BOTTOM, List.of("a"));
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        for (KeyFaceSlot slot : KeyFaceSlot.values()) cells.put(slot, new KeyFaceSolver.Cell(width, height));
        return KeyFaceSolver.solve(template, new KeyFaceDomain(texts).measure(NARROW, template.pairGapEm()), environment, cells);
    }

    private static boolean near(float actual, float expected) {
        return Math.abs(actual - expected) < 0.01f;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

import app.msime.android.FourteenKeyLayout;
import app.msime.android.KeyFaceDomain;
import app.msime.android.KeyFaceEnvironment;
import app.msime.android.KeyFaceMeasurer;
import app.msime.android.KeyFaceParams;
import app.msime.android.KeyFaceSlot;
import app.msime.android.KeyFaceSolver;
import app.msime.android.KeyFaceTemplate;
import app.msime.android.KeyFaceTemplates;
import app.msime.android.KeyHintButton;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardTypographyPolicy;
import app.msime.android.NineKeyDigitButton;
import app.msime.android.NineKeySidebarPolicy;
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

        @Override public float ascent(KeyFaceSlot slot, float sizePx) { return -0.9f * sizePx; }
    };

    public static void main(String[] args) {
        for (KeyFaceTemplate template : List.of(KeyFaceTemplates.LETTERS, KeyFaceTemplates.GRID_LETTERS)) {
            KeyFaceParams design = solve(template, new KeyFaceEnvironment(1, 1), 400, 52);
            check(near(design.main().sizePx(), 22), template.name() + " 默认键高上的字母应为 22 sp：" + design.main().sizePx());
            check(near(design.topRight().sizePx(), 10), template.name() + " 角标应为 10 sp：" + design.topRight().sizePx());
            check(near(design.bottom().sizePx(), 9), template.name() + " 双拼提示应为 9 sp：" + design.bottom().sizePx());
            // 角标按文字框放在原来的位置：上行线离上边 3 dp，排版宽度的右端离右边 5 dp。
            check(near(design.topRight().insetPx(), KeyHintButton.CORNER_HINT_TOP_DP)
                && near(design.topRight().insetXPx(), KeyHintButton.CORNER_HINT_RIGHT_DP),
                template.name() + " 角标应离上边 3 dp、右边 5 dp：" + design.topRight());
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

        // 功能键：默认键高上文字 15 sp、图标 22 dp；文字随字体缩放、封顶 17 sp，图标不随字体缩放、在 16–26 dp 之间。
        KeyFaceTemplate functions = KeyFaceTemplates.FUNCTIONS;
        KeyFaceParams design = solve(functions, new KeyFaceEnvironment(1, 1), 400, 52);
        check(near(design.main().sizePx(), 15), "功能键文字应为 15 sp：" + design.main().sizePx());
        check(near(design.iconPx(), 22), "功能键图标应为 22 dp：" + design.iconPx());
        KeyFaceParams scaled = solve(functions, new KeyFaceEnvironment(1, 1.15f), 400, 52);
        check(near(scaled.main().sizePx(), 15 * 1.15f) && near(scaled.iconPx(), 22),
            "功能键文字应跟随字体缩放、图标不跟：" + scaled.main().sizePx() + " " + scaled.iconPx());
        KeyFaceParams tall = solve(functions, new KeyFaceEnvironment(1, 1), 400, 400);
        check(near(tall.main().sizePx(), 17) && near(tall.iconPx(), 26), "大键上的功能键应封顶");
        check(near(solve(functions, new KeyFaceEnvironment(1, 1), 400, 20).iconPx(), 16), "矮键上的图标不应小于 16 dp");
        check(!functions.capAligned(), "功能键的汉字应按整个墨迹居中");
        check(functions.has(KeyFaceSlot.TOP_RIGHT) && !functions.has(KeyFaceSlot.BOTTOM), "功能键只有右上角提示");

        // 九键：默认键高上字母组 18 sp、数字 22 sp；右上角的数字与字母键角标同字号（10 sp）、文字框离上边 3 dp，离右边 12 dp（与原来的九键数字相同）。
        KeyFaceParams nine = solve(KeyFaceTemplates.NINE_LETTERS, new KeyFaceEnvironment(1, 1), 400, 52);
        check(near(nine.main().sizePx(), 18), "九键字母组应为 18 sp：" + nine.main().sizePx());
        check(near(nine.topRight().sizePx(), KeyHintButton.CORNER_HINT_SP), "九键数字应与字母键角标同字号：" + nine.topRight().sizePx());
        check(near(nine.topRight().insetPx(), KeyHintButton.CORNER_HINT_TOP_DP), "九键数字离上边应为 3 dp：" + nine.topRight().insetPx());
        check(near(nine.topRight().insetXPx(), NineKeyDigitButton.DIGIT_HINT_RIGHT_DP),
            "九键数字离右边应为 12 dp：" + nine.topRight().insetXPx());
        check(nine.bottom() == null, "九键字母层没有底部提示");
        KeyFaceParams digits = solve(KeyFaceTemplates.NINE_DIGITS, new KeyFaceEnvironment(1, 1), 400, 52);
        check(near(digits.main().sizePx(), 22) && digits.topRight() == null && near(digits.bottom().sizePx(), 9),
            "九键数字应为 22 sp、没有右上角、底部提示 9 sp：" + digits);
        check(near(KeyFaceTemplates.LETTERS.slot(KeyFaceSlot.TOP_RIGHT).insetEm(),
            KeyFaceTemplates.NINE_LETTERS.slot(KeyFaceSlot.TOP_RIGHT).insetEm()), "九键数字与字母键角标离上边一样远");
        // 数字行：默认键高上 20 sp，没有提示，按大写字母高居中。
        KeyFaceParams numbers = solve(KeyFaceTemplates.NUMBER_ROW, new KeyFaceEnvironment(1, 1), 400, 52);
        check(near(numbers.main().sizePx(), 20) && numbers.topRight() == null && numbers.bottom() == null,
            "数字行应为 20 sp、没有提示：" + numbers);
        check(KeyFaceTemplates.NUMBER_ROW.capAligned(), "数字行应按大写字母高居中");
        // 功能键文字最小 9 sp，与底行功能键相同（#11824 把 FUNCTION_KEY_MIN_TEXT_SP 降到了 9）。
        check(near(functions.slot(KeyFaceSlot.MAIN).minSp(), KeyboardTypographyPolicy.FUNCTION_KEY_MIN_TEXT_SP)
            && near(KeyboardTypographyPolicy.FUNCTION_KEY_MIN_TEXT_SP, 9), "功能键文字最小应为 9 sp");

        // 符号栏：一格是三行键高的五分之一时 18 sp（原来的固定字号）、右上角的 0 为 10 sp，格子更高也不超过 18 sp，矮格不小于 12 sp；按每个符号自己的墨迹居中，「，」「。」「、」放大 1.3 倍、低 0.1 个字号。
        KeyFaceTemplate side = KeyFaceTemplates.SIDE_SYMBOLS;
        int row = Math.round(3f * KeyboardGeometry.KEY_ROW_HEIGHT_DP / NineKeySidebarPolicy.VISIBLE_ROWS * 10);
        KeyFaceParams rail = solve(side, new KeyFaceEnvironment(10, 1), 600, row);
        check(Math.abs(rail.main().sizePx() / 10 - 18) < 0.1f, "设计格高上符号栏应为 18 sp：" + rail.main().sizePx() / 10);
        check(Math.abs(rail.topRight().sizePx() / 10 - 10) < 0.1f, "符号栏右上角的 0 应为 10 sp：" + rail.topRight().sizePx() / 10);
        check(near(solve(side, new KeyFaceEnvironment(1, 1), 600, 80).main().sizePx(), 18), "高格上的符号不应超过 18 sp");
        check(near(solve(side, new KeyFaceEnvironment(1, 1), 600, 10).main().sizePx(), 12), "矮格上的符号不应小于 12 sp");
        check(!side.capAligned() && side.glyphs() != null, "符号栏应按每个符号自己的墨迹居中");
        for (String small : List.of("，", "。", "、"))
            check(near(side.glyphs().scale(small), 1.3f), small + " 应放大 1.3 倍");
        check(near(side.glyphs().scale("？"), 1) && near(side.glyphs().scale("……"), 1), "其他符号不放大");
        check(near(side.glyphs().dropEm(), 0.1f), "「，」「。」「、」应比中线低 0.1 个字号");
        // #+= 层：默认键高上第一排 20 sp、其余 18 sp（原来的固定字号），只有主标签，全排共用一条基线。
        KeyFaceParams top = solve(KeyFaceTemplates.LAYER_TOP_ROW, new KeyFaceEnvironment(1, 1), 400, 52);
        KeyFaceParams rest = solve(KeyFaceTemplates.LAYER_CHARACTERS, new KeyFaceEnvironment(1, 1), 400, 52);
        check(near(top.main().sizePx(), 20) && near(rest.main().sizePx(), 18), "#+= 层应为 20 / 18 sp：" + top + " " + rest);
        check(top.topRight() == null && top.bottom() == null && rest.topRight() == null && rest.bottom() == null,
            "#+= 层没有提示");
        check(near(solve(KeyFaceTemplates.LAYER_CHARACTERS, new KeyFaceEnvironment(1, 1), 400, 20).main().sizePx(), 12)
            && near(solve(KeyFaceTemplates.LAYER_TOP_ROW, new KeyFaceEnvironment(1, 1), 400, 400).main().sizePx(), 22),
            "#+= 层应夹在 12 sp 和最大字号之间");
        for (KeyFaceTemplate layer : List.of(KeyFaceTemplates.LAYER_TOP_ROW, KeyFaceTemplates.LAYER_CHARACTERS))
            check(!layer.capAligned() && layer.glyphs() == null, layer.name() + " 应按整个内容域的墨迹共用一条基线");
        // 韩文和大千：默认键高上与字母同为 22 sp，按整个墨迹居中；大千的四个声调按 1.8 倍、按自己的墨迹居中，其余注音共用基线。
        for (KeyFaceTemplate script : List.of(KeyFaceTemplates.KOREAN, KeyFaceTemplates.DACHEN)) {
            check(near(solve(script, new KeyFaceEnvironment(1, 1), 400, 52).main().sizePx(), 22), script.name() + " 应为 22 sp");
            check(!script.capAligned(), script.name() + " 应按整个墨迹居中");
        }
        KeyFaceTemplate.Glyphs dachenTones = KeyFaceTemplates.DACHEN.glyphs();
        check(dachenTones != null && !dachenTones.every() && dachenTones.scales().size() == 4, "大千应登记四个声调：" + dachenTones);
        for (float toneScale : dachenTones.scales().values()) check(near(toneScale, 1.8f), "大千声调应放大 1.8 倍");
        check(!dachenTones.centres("ㄅ"), "大千的注音符号不按自己的墨迹居中");
        check(KeyFaceTemplates.KOREAN.glyphs() == null, "韩文不按自己的墨迹居中");
        System.out.println("KeyFaceTemplatesSmoke passed");
    }

    private static KeyFaceParams solve(KeyFaceTemplate template, KeyFaceEnvironment environment, int width, int height) {
        Map<KeyFaceSlot, List<String>> texts = new EnumMap<>(KeyFaceSlot.class);
        texts.put(KeyFaceSlot.MAIN, List.of("A"));
        texts.put(KeyFaceSlot.TOP_RIGHT, List.of("1"));
        texts.put(KeyFaceSlot.BOTTOM, List.of("a"));
        Map<KeyFaceSlot, KeyFaceSolver.Cell> cells = new EnumMap<>(KeyFaceSlot.class);
        for (KeyFaceSlot slot : KeyFaceSlot.values()) cells.put(slot, new KeyFaceSolver.Cell(width, height));
        return KeyFaceSolver.solve(template, new KeyFaceDomain(texts).measure(NARROW, template.pairGapEm(), template.glyphs()), environment, cells);
    }

    private static boolean near(float actual, float expected) {
        return Math.abs(actual - expected) < 0.01f;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

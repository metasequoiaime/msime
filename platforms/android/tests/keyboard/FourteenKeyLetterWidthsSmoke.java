package app.msime.android;

import java.util.List;

/** 14/18 键字母键宽度（#11051）：等宽照原来的份额排，宽窄按 issue 里的比例排，偏好写法与 client-core 一致。 */
public final class FourteenKeyLetterWidthsSmoke {
    private static final float TOLERANCE = 1e-4f;

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    private static void near(float actual, float expected, String message) {
        if (Math.abs(actual - expected) > TOLERANCE)
            throw new AssertionError(message + "：期望 " + expected + "，实际 " + actual);
    }

    private static List<List<FourteenKeyLetterWidths.Cell>> eighteen(FourteenKeyLetterWidths.Mode mode) {
        return FourteenKeyLetterWidths.plan(EighteenKeyLayout.rows(), 1f, 1f, mode.geometry());
    }

    private static List<List<FourteenKeyLetterWidths.Cell>> fourteen(FourteenKeyLetterWidths.Mode mode) {
        return FourteenKeyLetterWidths.plan(FourteenKeyLayout.rows(), FourteenKeyLayout.LETTER_WEIGHT,
            KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT, mode.geometry());
    }

    /** 一格占这一行宽度的比例。 */
    private static float share(List<FourteenKeyLetterWidths.Cell> row, int index) {
        return row.get(index).weight() / FourteenKeyLetterWidths.total(row);
    }

    private static List<FourteenKeyLetterWidths.Kind> kinds(List<FourteenKeyLetterWidths.Cell> row) {
        return row.stream().map(FourteenKeyLetterWidths.Cell::kind).toList();
    }

    private static List<String> letters(List<FourteenKeyLetterWidths.Cell> row) {
        return row.stream().filter(cell -> cell.kind() == FourteenKeyLetterWidths.Kind.KEY)
            .map(cell -> cell.key().letters()).toList();
    }

    public static void main(String[] arguments) {
        wire();
        equalKeepsTodaysWeights();
        variedMatchesTheIssue();
        everyLetterKeyIsPlacedOnce();
        unofferedCombinationsStillCompute();
        System.out.println("Android 14/18-key letter widths passed");
    }

    private static void wire() {
        check(FourteenKeyLetterWidths.PREFERENCE.equals("touch_fourteen_key_letter_widths"), "偏好键名与 client-core 相同");
        check(FourteenKeyLetterWidths.Mode.EQUAL.wire().equals("equal"), "等宽的写法");
        check(FourteenKeyLetterWidths.Mode.VARIED.wire().equals("varied"), "宽窄的写法");
        check(FourteenKeyLetterWidths.Mode.parse("varied") == FourteenKeyLetterWidths.Mode.VARIED, "认得宽窄");
        for (String wire : new String[] {null, "", "equal", "equal_indented", "Varied"})
            check(FourteenKeyLetterWidths.Mode.parse(wire) == FourteenKeyLetterWidths.Mode.EQUAL,
                "缺省和不认识的写法按等宽读：" + wire);
        check(FourteenKeyLetterWidths.Mode.CHOICES.equals(
            List.of(FourteenKeyLetterWidths.Mode.EQUAL, FourteenKeyLetterWidths.Mode.VARIED)), "胶囊先等宽后宽窄");
        check(FourteenKeyLetterWidths.Mode.EQUAL.title().equals("等宽")
            && FourteenKeyLetterWidths.Mode.VARIED.title().equals("宽窄"), "胶囊名字");
        boolean rejected = false;
        try {
            new FourteenKeyLetterWidths.Geometry(1.5f, true);
        } catch (IllegalArgumentException expected) {
            rejected = true;
        }
        check(rejected, "单字母键份额不能超过双字母键");
    }

    /** 默认值不能改变任何人的键盘：等宽逐格等于改动前 `rebuildFourteenKeyRows` 用的份额。 */
    private static void equalKeepsTodaysWeights() {
        List<List<FourteenKeyLetterWidths.Cell>> eighteen = eighteen(FourteenKeyLetterWidths.Mode.EQUAL);
        check(eighteen.stream().map(List::size).toList().equals(List.of(7, 6, 7)), "18 键等宽没有占位格");
        for (List<FourteenKeyLetterWidths.Cell> row : eighteen)
            for (FourteenKeyLetterWidths.Cell cell : row) near(cell.weight(), 1f, "18 键等宽每格 1 份");
        List<List<FourteenKeyLetterWidths.Cell>> fourteen = fourteen(FourteenKeyLetterWidths.Mode.EQUAL);
        check(fourteen.stream().map(List::size).toList().equals(List.of(5, 5, 6)), "14 键等宽没有占位格");
        for (List<FourteenKeyLetterWidths.Cell> row : fourteen) {
            for (FourteenKeyLetterWidths.Cell cell : row) {
                float expected = cell.kind() == FourteenKeyLetterWidths.Kind.EDGE
                    ? KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT : FourteenKeyLayout.LETTER_WEIGHT;
                near(cell.weight(), expected, "14 键等宽：字母键 2 份、分词和 ⌫ 1.4 份");
            }
        }
        near(share(fourteen.get(2), 0), 1.4f / 10.8f, "14 键等宽分词约占 13%");
    }

    /** #11051 正文表格里的比例。 */
    private static void variedMatchesTheIssue() {
        List<List<FourteenKeyLetterWidths.Cell>> eighteen = eighteen(FourteenKeyLetterWidths.Mode.VARIED);
        check(kinds(eighteen.get(0)).stream().allMatch(kind -> kind == FourteenKeyLetterWidths.Kind.KEY),
            "18 键首排是最长的一排，没有占位格");
        near(FourteenKeyLetterWidths.total(eighteen.get(0)), 6f, "18 键首排四个单字母键、三个双字母键共 6 份");
        near(eighteen.get(0).get(0).weight(), 0.75f, "Q 0.75 份");
        near(eighteen.get(0).get(1).weight(), 1f, "WE 1 份");
        check(kinds(eighteen.get(1)).equals(List.of(FourteenKeyLetterWidths.Kind.PAD,
            FourteenKeyLetterWidths.Kind.KEY, FourteenKeyLetterWidths.Kind.KEY, FourteenKeyLetterWidths.Kind.KEY,
            FourteenKeyLetterWidths.Kind.KEY, FourteenKeyLetterWidths.Kind.KEY, FourteenKeyLetterWidths.Kind.KEY,
            FourteenKeyLetterWidths.Kind.PAD)), "18 键第二排两端各一格占位");
        near(share(eighteen.get(1), 0), 0.0625f, "18 键第二排两端各空出 6.25%（半个单字母键）");
        near(eighteen.get(1).get(0).weight(), 0.375f, "占位格正好半个单字母键");
        near(share(eighteen.get(2), 0), 0.875f / 6f, "18 键分词约占 14.6%");
        near(share(eighteen.get(2), eighteen.get(2).size() - 1), 0.875f / 6f, "18 键 ⌫ 与分词同宽");

        List<List<FourteenKeyLetterWidths.Cell>> fourteen = fourteen(FourteenKeyLetterWidths.Mode.VARIED);
        near(share(fourteen.get(1), 0), 0.025f, "14 键第二排两端各空出 2.5%");
        near(share(fourteen.get(2), 0), 0.125f, "14 键分词占 12.5%");
        near(fourteen.get(1).get(5).weight(), 1.5f, "14 键 L 是双字母键的 0.75");

        // 三排共用首排的单位宽度：每排份额之和相同，同一种键在三排里一样宽。
        for (List<List<FourteenKeyLetterWidths.Cell>> plan : List.of(eighteen, fourteen)) {
            float first = FourteenKeyLetterWidths.total(plan.get(0));
            for (List<FourteenKeyLetterWidths.Cell> row : plan)
                near(FourteenKeyLetterWidths.total(row), first, "宽窄三排份额之和相同");
        }
    }

    private static void everyLetterKeyIsPlacedOnce() {
        for (FourteenKeyLetterWidths.Mode mode : FourteenKeyLetterWidths.Mode.values()) {
            for (List<List<FourteenKeyLetterWidths.Cell>> plan : List.of(eighteen(mode), fourteen(mode))) {
                String all = plan.stream().flatMap(row -> letters(row).stream()).reduce("", String::concat);
                check(all.length() == 26 && all.chars().distinct().count() == 26, "26 个字母各一次：" + mode);
                List<FourteenKeyLetterWidths.Cell> last = plan.get(plan.size() - 1);
                check(last.get(0).kind() == FourteenKeyLetterWidths.Kind.EDGE
                    && last.get(last.size() - 1).kind() == FourteenKeyLetterWidths.Kind.EDGE,
                    "第三排两端是分词和 ⌫：" + mode);
            }
        }
        check(letters(eighteen(FourteenKeyLetterWidths.Mode.VARIED).get(1))
            .equals(List.of("a", "sd", "fg", "h", "jk", "l")), "18 键第二排键序不变");
    }

    /** 设置页不列的两种组合照样能算，以后改映射不用动排法。 */
    private static void unofferedCombinationsStillCompute() {
        // 等宽 + 不铺满：就是 #10401 之前 18 键的第二排半键缩进；14 键只让第三排的分词和 ⌫ 变窄到 10%。
        FourteenKeyLetterWidths.Geometry indented = new FourteenKeyLetterWidths.Geometry(1f, false);
        List<List<FourteenKeyLetterWidths.Cell>> eighteen =
            FourteenKeyLetterWidths.plan(EighteenKeyLayout.rows(), 1f, 1f, indented);
        near(eighteen.get(1).get(0).weight(), 0.5f, "等宽不铺满时 18 键第二排缩进半键");
        List<List<FourteenKeyLetterWidths.Cell>> fourteen = FourteenKeyLetterWidths.plan(FourteenKeyLayout.rows(),
            FourteenKeyLayout.LETTER_WEIGHT, KeyboardActionRow.DESIGN_LETTER_EDGE_WEIGHT, indented);
        check(fourteen.get(1).size() == 5, "14 键前两排一样长，没有占位格");
        near(share(fourteen.get(2), 0), 0.1f, "等宽不铺满时 14 键分词变窄到 10%");
        // 宽窄 + 铺满：每排各自铺满，第三排两端照旧是 edgeWeight。
        FourteenKeyLetterWidths.Geometry filled = new FourteenKeyLetterWidths.Geometry(0.75f, true);
        List<List<FourteenKeyLetterWidths.Cell>> variedFilled =
            FourteenKeyLetterWidths.plan(EighteenKeyLayout.rows(), 1f, 1f, filled);
        check(variedFilled.get(1).size() == 6, "铺满时没有占位格");
        near(variedFilled.get(2).get(0).weight(), 1f, "铺满时分词仍是 edgeWeight");
    }
}

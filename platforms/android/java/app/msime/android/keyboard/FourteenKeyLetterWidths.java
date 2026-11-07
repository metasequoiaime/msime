package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 14 键与 18 键三排字母键的宽度分法：共享偏好 `touch_fourteen_key_letter_widths`（crates/client-core 的 `TouchFourteenKeyLetterWidths`，#11051）。键盘（`ImeLetterRows#rebuildFourteenKeyRows`）和两个设置页预览（{@link KeyLayoutStylesPolicy}、`KeyboardPreview`）都从 {@link #plan} 取每一格的份额，不各自算。
 *
 * <p>宿主里拆成两个参数（{@link Geometry}）：单字母键相对双字母键的份额，和每排是否各自铺满。设置页只给两种组合：{@link Mode#EQUAL} 是原来的排法（单字母键与双字母键同宽、每排各自铺满），{@link Mode#VARIED} 是「宽窄」（单字母键 {@link #VARIED_SINGLE_LETTER_WEIGHT} 份、三排共用首排的单位宽度）。另外两种组合 {@link #plan} 照样能算，只是没有入口。
 *
 * <p>不铺满时：每排字母键的份额加起来比首排少的部分平分到两端。第一、二排两端是空出来的占位格（{@link Kind#PAD}），它不是死区，按下交给同一排挨着它的那个键（`KeyboardKeyArea` 的占位格让给邻键）；第三排两端是分词和 ⌫（{@link Kind#EDGE}），它们分掉剩下的宽度。铺满时第三排两端照旧是 `edgeWeight`。键距、行距、键盘高度、顶部数字行和底行都不在这里。
 */
public final class FourteenKeyLetterWidths {
    /** 共享偏好的键名。 */
    public static final String PREFERENCE = "touch_fourteen_key_letter_widths";

    /** 「宽窄」里单字母键相对双字母键的份额。取自 #11051 参考图实测的键距比 3 : 4。 */
    public static final float VARIED_SINGLE_LETTER_WEIGHT = 0.75f;

    /** 份额比这还小的空隙不画占位格（浮点误差，或首排本来就是最长的一排）。 */
    private static final float EPSILON = 1e-4f;

    private FourteenKeyLetterWidths() {}

    /** 设置页的两个选项，写法与 client-core 相同。 */
    public enum Mode {
        /** 等宽：原来的排法，缺省值。 */
        EQUAL("equal", "等宽", new Geometry(1f, true)),
        /** 宽窄：双字母键 1 份、单字母键 0.75 份，三排共用首排的单位宽度。 */
        VARIED("varied", "宽窄", new Geometry(VARIED_SINGLE_LETTER_WEIGHT, false));

        /** 设置页胶囊的先后。 */
        public static final List<Mode> CHOICES = List.of(EQUAL, VARIED);

        private final String wire;
        private final String title;
        private final Geometry geometry;

        Mode(String wire, String title, Geometry geometry) {
            this.wire = wire;
            this.title = title;
            this.geometry = geometry;
        }

        /** 文档和同步键里的写法。 */
        public String wire() { return wire; }

        /** 设置页胶囊上的名字。 */
        public String title() { return title; }

        /** 这一项对应的两个参数。 */
        public Geometry geometry() { return geometry; }

        /** 宽松读取：没有偏好、不是字符串或不认识的写法（较新版本加的分法）都按等宽，与 client-core 的宽松反序列化一致。 */
        public static Mode parse(String wire) {
            if (wire != null) {
                for (Mode mode : values()) if (mode.wire.equals(wire)) return mode;
            }
            return EQUAL;
        }
    }

    /**
     * 宿主内部的两个参数。
     *
     * @param singleLetterWeight 单字母键相对双字母键的份额，在 (0, 1] 之间
     * @param fillRows 每排各自铺满；为 false 时三排共用首排的单位宽度、短的排居中
     */
    public record Geometry(float singleLetterWeight, boolean fillRows) {
        public Geometry {
            if (!(singleLetterWeight > 0f && singleLetterWeight <= 1f))
                throw new IllegalArgumentException("singleLetterWeight must be in (0, 1]");
        }
    }

    /** 一格是什么。 */
    public enum Kind {
        /** 字母键。 */
        KEY,
        /** 第三排两端的分词键（左）和 ⌫（右）。 */
        EDGE,
        /** 不铺满时第一、二排两端空出来的地方；按下归同一排挨着它的键。 */
        PAD
    }

    /**
     * 一排里的一格。
     *
     * @param kind 是什么
     * @param key 字母键这一格的键；分词、⌫ 和占位格为 null
     * @param weight 在本行里的宽度份额（同一张 {@link #plan} 里各行份额的单位相同）
     */
    public record Cell(Kind kind, FourteenKeyLayout.Key key, float weight) {}

    /**
     * 把三排字母键排成每一格的份额，从左到右。最后一排两端加分词和 ⌫。
     *
     * @param rows {@link FourteenKeyLayout#rows()} 或 {@link EighteenKeyLayout#rows()}
     * @param letterWeight 双字母键的份额（14 键 {@link FourteenKeyLayout#LETTER_WEIGHT}，18 键 1）
     * @param edgeWeight 铺满时分词和 ⌫ 的份额（14 键 {@link KeyboardActionRow#DESIGN_LETTER_EDGE_WEIGHT}，18 键 1）
     */
    public static List<List<Cell>> plan(List<List<FourteenKeyLayout.Key>> rows, float letterWeight,
            float edgeWeight, Geometry geometry) {
        if (rows.isEmpty()) return List.of();
        float firstRow = letters(rows.get(0), letterWeight, geometry);
        List<List<Cell>> plan = new ArrayList<>(rows.size());
        for (int index = 0; index < rows.size(); index++) {
            List<FourteenKeyLayout.Key> keys = rows.get(index);
            boolean last = index == rows.size() - 1;
            // 不铺满时这一排比首排短出来的份额平分到两端；首排自己、或比首排还长的排没有空隙。
            float rest = geometry.fillRows() ? 0f
                : Math.max(0f, (firstRow - letters(keys, letterWeight, geometry)) / 2f);
            List<Cell> row = new ArrayList<>(keys.size() + 2);
            if (last) row.add(new Cell(Kind.EDGE, null, geometry.fillRows() ? edgeWeight : rest));
            else if (rest > EPSILON) row.add(new Cell(Kind.PAD, null, rest));
            for (FourteenKeyLayout.Key key : keys) row.add(new Cell(Kind.KEY, key, weight(key, letterWeight, geometry)));
            if (last) row.add(new Cell(Kind.EDGE, null, geometry.fillRows() ? edgeWeight : rest));
            else if (rest > EPSILON) row.add(new Cell(Kind.PAD, null, rest));
            plan.add(List.copyOf(row));
        }
        return List.copyOf(plan);
    }

    /** 一个字母键的份额：双字母键 `letterWeight`，单字母键再乘 {@link Geometry#singleLetterWeight}。 */
    public static float weight(FourteenKeyLayout.Key key, float letterWeight, Geometry geometry) {
        return key.letters().length() > 1 ? letterWeight : letterWeight * geometry.singleLetterWeight();
    }

    /**
     * 第 `rowIndex` 排的每个占位格朝外要补几个半键距的外边距。键（字母键、分词、⌫）各带左右两个半键距，行宽先扣掉外边距再按份额分；这一排比首排少几个键，就少扣几个键距，两端的占位格各补回同样多个半键距，三排的单位宽度才一样（18 键宽窄第二排是 1，14 键前两排键数相同是 0）。没有占位格的排也照算，调用方只套在占位格上。
     */
    public static int padHalfGaps(List<List<Cell>> plan, int rowIndex) {
        return Math.max(0, spaced(plan.get(0)) - spaced(plan.get(rowIndex)));
    }

    /** 一排里带键距外边距的格数：占位格不算。 */
    private static int spaced(List<Cell> row) {
        int count = 0;
        for (Cell cell : row) if (cell.kind() != Kind.PAD) count++;
        return count;
    }

    /** 一排各格份额之和。 */
    public static float total(List<Cell> row) {
        float sum = 0f;
        for (Cell cell : row) sum += cell.weight();
        return sum;
    }

    private static float letters(List<FourteenKeyLayout.Key> keys, float letterWeight, Geometry geometry) {
        float sum = 0f;
        for (FourteenKeyLayout.Key key : keys) sum += weight(key, letterWeight, geometry);
        return sum;
    }
}

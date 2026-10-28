package app.msime.android;

import java.util.List;

/**
 * 触屏键盘按键类型的底行样式与顶部数字行：共享偏好 `touch_twenty_six_key_style`、`touch_fourteen_key_style`、`touch_nine_key_style` 与 `touch_number_row`（crates/client-core 的 `Preferences`，决策见 .agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md）。三项样式缺省都是 `standard`，即原有底行；不认识的写法（较新版本加的样式）和这类键面画不出的写法一律按 `standard` 读，与 client-core 的宽松读取一致。
 *
 * <p>纯 Java，不碰 org.json，check-host 的 JVM 冒烟直接核对它。
 */
public final class TouchKeyStyle {
    public static final String TWENTY_SIX_KEY_PREFERENCE = "touch_twenty_six_key_style";
    public static final String FOURTEEN_KEY_PREFERENCE = "touch_fourteen_key_style";
    public static final String NINE_KEY_PREFERENCE = "touch_nine_key_style";
    public static final String NUMBER_ROW_PREFERENCE = "touch_number_row";

    /** 底行的一种排法；每类键面只提供其中几种（{@link #TWENTY_SIX_KEY_ROWS}、{@link #FOURTEEN_KEY_ROWS}、{@link #NINE_KEY_ROWS}）。 */
    public enum Row {
        STANDARD("standard"),
        /** 空格两侧各一个标点：26 键与九键。 */
        COMMA_PERIOD("comma_period"),
        /** 长空格：26 键与 14/18 键。 */
        LONG_SPACE("long_space"),
        /** 表情键：三类键面都有。 */
        EMOJI("emoji"),
        /** 右侧大回车：只有九键外框。回车跨右列下两行，符 移到符号栏下方，九宫格下方的底行只剩 123 |（地球）| 空格 | 中/英（{@link KeyboardActionRow#tallReturnEntries}）。 */
        TALL_RETURN("tall_return");

        public final String wire;

        Row(String wire) {
            this.wire = wire;
        }
    }

    /** 中文 26 键：标准、逗号句号在空格两侧、长空格、表情键。 */
    public static final List<Row> TWENTY_SIX_KEY_ROWS = List.of(Row.STANDARD, Row.COMMA_PERIOD, Row.LONG_SPACE, Row.EMOJI);
    /** 14 键与 18 键：本来就有独立逗号句号，没有 `comma_period`。 */
    public static final List<Row> FOURTEEN_KEY_ROWS = List.of(Row.STANDARD, Row.LONG_SPACE, Row.EMOJI);
    /** 九键：左右两列已占满，没有地方放长空格；右侧大回车排在表情键之后。 */
    public static final List<Row> NINE_KEY_ROWS = List.of(Row.STANDARD, Row.COMMA_PERIOD, Row.EMOJI, Row.TALL_RETURN);

    /** 三类键面各自选中的样式，缺省全是标准底行。 */
    public record Styles(Row twentySix, Row fourteen, Row nine) {
        public static final Styles DEFAULT = new Styles(Row.STANDARD, Row.STANDARD, Row.STANDARD);

        public Styles {
            twentySix = offered(twentySix, TWENTY_SIX_KEY_ROWS);
            fourteen = offered(fourteen, FOURTEEN_KEY_ROWS);
            nine = offered(nine, NINE_KEY_ROWS);
        }

        /** 从偏好里读到的三个写法（缺键时为 null）。 */
        public static Styles parse(String twentySix, String fourteen, String nine) {
            return new Styles(TouchKeyStyle.parse(twentySix, TWENTY_SIX_KEY_ROWS),
                TouchKeyStyle.parse(fourteen, FOURTEEN_KEY_ROWS), TouchKeyStyle.parse(nine, NINE_KEY_ROWS));
        }

        /**
         * 这一刻画出来的字母层底行用哪种样式。中文 26 键（标准 26 键键面上的中文方案，含全拼、双拼、五笔、粤拼等）读 26 键样式；英文模式、韩文和其他语言的 26 键保留标准底行；14 键与 18 键读 14 键样式；拼音九键与笔画（共用九键外框）读九键样式；其余键面（手写、注音、日文假名）没有可选样式。
         *
         * @param touchLayout {@link KeyboardLayout} 的界面常量
         * @param scheme View 报告的方案编号，未知时为 -1（按中文算，与数字九宫格的判断一致）
         * @param english 宿主处在英文模式
         */
        public Row forLayout(int touchLayout, int scheme, boolean english) {
            if (touchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT)
                return KeyboardLayout.numberPadLanguage(scheme, english) ? twentySix : Row.STANDARD;
            if (touchLayout == KeyboardLayout.FOURTEEN_KEY_LAYOUT || touchLayout == KeyboardLayout.EIGHTEEN_KEY_LAYOUT)
                return fourteen;
            if (touchLayout == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT || touchLayout == KeyboardLayout.STROKE_LAYOUT)
                return nine;
            return Row.STANDARD;
        }

        /**
         * 工具栏隐藏或按钮全关时，工具栏那一行要不要留着：中文 26 键的长空格样式把 符 放在工具栏上，所以这块键盘的中文键面是中文 26 键、用户给 26 键选了长空格时一直留着。只看用户的样式和这块键盘的中文键面，不看此刻的中/英：同一块键盘切到英文时英文 26 键画自己的标准底行、工具栏上没有 符，但这一行照样留着，来回切换时键盘高度不跳。
         *
         * @param chineseTouchLayout 这块键盘处在中文时画的键面（{@link KeyboardLayout} 的界面常量；英文模式下是切到英文之前的那块）
         * @param scheme View 报告的方案编号，未知时为 -1
         */
        public boolean keepsToolbarRow(int chineseTouchLayout, int scheme) {
            return chineseTouchLayout == KeyboardLayout.STANDARD_TOUCH_LAYOUT
                && forLayout(chineseTouchLayout, scheme, false) == Row.LONG_SPACE;
        }
    }

    /**
     * 本版本提供哪几组样式：中文 26 键（全拼、双拼、五笔、粤拼）、14 键与 18 键（全拼 14/18 键，双拼和五笔也有 14 键）、9 键（拼音九键、九键双拼、笔画）。与鸿蒙 `touchKeyLayoutGroups` 同一条规则：一组都没有时（例如日语版）键盘页不列「按键布局」入口，26 键与 14/18 键都没有时子页不列「顶部数字行」开关。
     */
    public record Groups(boolean twentySix, boolean fourteen, boolean nine) {
        public static Groups offeredBy(AppEdition edition) {
            return new Groups(
                offersAny(edition, KeyboardScheme.QUANPIN, KeyboardScheme.XIAOHE, KeyboardScheme.WUBI, KeyboardScheme.CANTONESE),
                offersAny(edition, KeyboardScheme.QUANPIN_FOURTEEN_KEY, KeyboardScheme.QUANPIN_EIGHTEEN_KEY,
                    KeyboardScheme.XIAOHE, KeyboardScheme.WUBI),
                offersAny(edition, KeyboardScheme.QUANPIN_NINE_KEY, KeyboardScheme.NINE_KEY_SHUANGPIN, KeyboardScheme.STROKE));
        }

        /** 至少有一组：键盘页列出「按键布局」入口。 */
        public boolean any() { return twentySix || fourteen || nine; }

        /** 顶部数字行只画在 26 键、14 键和 18 键上。 */
        public boolean numberRow() { return twentySix || fourteen; }

        private static boolean offersAny(AppEdition edition, KeyboardScheme... schemes) {
            for (KeyboardScheme scheme : schemes) if (scheme.offeredBy(edition)) return true;
            return false;
        }
    }

    private TouchKeyStyle() {}

    /** 严格写法里属于这类键面的那一种；缺键、不认识或这类键面不提供时是标准底行。 */
    public static Row parse(String wire, List<Row> offered) {
        if (wire != null) {
            for (Row row : offered) if (row.wire.equals(wire)) return row;
        }
        return Row.STANDARD;
    }

    private static Row offered(Row row, List<Row> offered) {
        return row != null && offered.contains(row) ? row : Row.STANDARD;
    }
}

package app.msime.android;

/**
 * 上屏后联想时候选栏右端放哪些按键（#10078）：共享偏好 `next_word_prediction_buttons`，写法与 client-core 的 `NextWordPredictionButtons` 相同，缺键或不认识的写法按现有的「只有展开键」读。
 *
 * <p>只管联想状态：普通组字时关闭键永远不出现，展开键照旧按「多于一页或九键面板可用」显示。关闭键送 Cancel，引擎清掉联想后报已处理，不上屏也不插入空格。
 */
public final class PredictionButtonsPolicy {
    public static final String PREFERENCE_KEY = "next_word_prediction_buttons";

    public enum Style {
        /** 现有样式：只有展开键，关闭联想靠继续输入。 */
        EXPAND_ONLY("expand_only"),
        /** 关闭键 × 加展开键；展开面板打开时 × 仍在 ∧ 左边。 */
        CLOSE_AND_EXPAND("close_and_expand"),
        /** 只有关闭键 ×，联想只能在候选栏里左右滑动选择。 */
        CLOSE_ONLY("close_only");

        private final String wire;

        Style(String wire) {
            this.wire = wire;
        }

        /** 偏好文档里的写法。 */
        public String wire() {
            return wire;
        }

        /** 宽松读取：null、空串和不认识的写法（较新版本加的样式）都按现有样式。 */
        public static Style parse(String value) {
            for (Style style : values()) if (style.wire.equals(value)) return style;
            return EXPAND_ONLY;
        }
    }

    private PredictionButtonsPolicy() { }

    /** 候选栏是否放关闭键：只在显示联想时，并且样式带 ×。 */
    public static boolean showsClose(Style style, boolean prediction) {
        return prediction && style != Style.EXPAND_ONLY;
    }

    /** 展开键能不能出现（出现时仍要满足原来的多页条件）：联想时「只有关闭」不放展开键，其余情况照旧。 */
    public static boolean allowsExpand(Style style, boolean prediction) {
        return !prediction || style != Style.CLOSE_ONLY;
    }
}

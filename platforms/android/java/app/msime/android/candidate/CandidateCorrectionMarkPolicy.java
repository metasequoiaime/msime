package app.msime.android;

/**
 * 双拼触点纠错坐上首选时的显式标记（#10018）。引擎只给按「触点支持且明显更优」规则坐上首选的纠错行带 `confident_correction: true`，其余候选不带这个字段，默认视图逐字节不变。宿主读到它就把这一行的候选字和注释画成强调色（{@link CandidateAppearance.Palette#correctionFor}），读屏描述末尾补「，已纠正」；已有的 corrected_from 注释照旧显示。
 */
public final class CandidateCorrectionMarkPolicy {
    /** View JSON 候选对象上的字段名，候选条和展开面板都从 `candidate.opt(FIELD)` 读。 */
    public static final String FIELD = "confident_correction";
    /** 读屏描述末尾补的一段。 */
    public static final String ACCESSIBILITY_SUFFIX = "，已纠正";

    private CandidateCorrectionMarkPolicy() {}

    /** `raw` 是 `candidate.opt(FIELD)` 的值：只有 JSON 布尔真才算；字段缺省（null）、false、字符串 "true"、数字 1 都不算，免得格式不对的视图把普通候选标成纠错。 */
    public static boolean confident(Object raw) {
        return JsonPolicy.strictTrue(raw);
    }

    /** 读屏描述里接在注释和音标后面的那一段；不是坐上首选的纠错行时为空。 */
    public static String accessibilitySuffix(boolean confident) {
        return confident ? ACCESSIBILITY_SUFFIX : "";
    }
}

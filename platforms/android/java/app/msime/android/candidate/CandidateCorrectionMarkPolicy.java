package app.msime.android;

/**
 * 双拼触点纠错坐上首选时的显式标记（#10018）。引擎只给按「触点支持且明显更优」规则坐上首选的纠错行带 `confident_correction: true`，其余候选不带这个字段，默认视图逐字节不变。宿主读到它就给这一行的候选字盖上标记色并加一道同色下划线、注释改成标记色实色（{@link CandidateAppearance.Palette#correctionFor}），读屏描述末尾补「，已纠正」；已有的 corrected_from 注释照旧显示。
 *
 * 下划线不能省：坐上首选的纠错行总在高亮的首选 chip 上，未用候选包时那里的候选字本来就是强调色，只换颜色的话候选字看不出任何变化，只剩 0.62 倍的注释从 58% 透明变成实色。
 */
public final class CandidateCorrectionMarkPolicy {
    /** View JSON 候选对象上的字段名，候选条和展开面板都从 `candidate.opt(FIELD)` 读。 */
    public static final String FIELD = "confident_correction";
    /** 读屏描述末尾补的一段。 */
    public static final String ACCESSIBILITY_SUFFIX = "，已纠正";
    /** 不带标记的注释沿用的调淡比例，和改动前的 candidateLabel 一致。 */
    static final float PLAIN_ANNOTATION_ALPHA = 0.58f;

    private CandidateCorrectionMarkPolicy() {}

    /** `raw` 是 `candidate.opt(FIELD)` 的值：只有 JSON 布尔真才算；字段缺省（null）、false、字符串 "true"、数字 1 都不算，免得格式不对的视图把普通候选标成纠错。 */
    public static boolean confident(Object raw) {
        return JsonPolicy.strictTrue(raw);
    }

    /** 读屏描述里接在注释和音标后面的那一段；不是坐上首选的纠错行时为空。 */
    public static String accessibilitySuffix(boolean confident) {
        return confident ? ACCESSIBILITY_SUFFIX : "";
    }

    /** 没有数字前缀、没有注释、也不是坐上首选的纠错行时，candidateLabel 把候选字原样交给 CandidateSymbolLabel，不建 SpannableString，和没有这个字段时逐字节一样。 */
    public static boolean plainLabel(String prefix, String annotation, boolean confident) {
        return prefix.isEmpty() && annotation.isEmpty() && !confident;
    }

    /** 盖在候选字上的标记色，candidateLabel 在同一段再加一道下划线；不是坐上首选的纠错行时返回 null，候选字不加任何 span，照旧用按钮自己的选中/未选中文字色。 */
    public static Integer wordMark(CandidateAppearance.Palette palette, boolean highlighted, boolean confident) {
        return confident ? palette.correctionFor(highlighted) : null;
    }

    /** 注释行的颜色：坐上首选的纠错行取标记色实色；其余行是改动前的规则，用了候选包取包的 secondaryFor，否则把候选字色调到 58%。 */
    public static int annotationColour(CandidateAppearance.Palette palette, boolean highlighted, boolean confident) {
        if (confident) return palette.correctionFor(highlighted);
        return palette.externalSkin() ? palette.secondaryFor(highlighted)
            : ColorPolicy.withAlpha(palette.textFor(highlighted), PLAIN_ANNOTATION_ALPHA);
    }
}

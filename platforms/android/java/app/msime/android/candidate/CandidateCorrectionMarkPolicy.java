package app.msime.android;

/**
 * 双拼触点纠错坐上首选时的显式标记（#10018）。引擎只给按「触点支持且明显更优」规则坐上首选的纠错行带 `confident_correction: true`，其余候选不带这个字段，默认视图逐字节不变。宿主读到它就给这一行的候选字盖上标记色、注释改成标记色实色（{@link CandidateAppearance.Palette#correctionFor}），读屏描述末尾补「，已纠正」。所有纠错行（`corrected`）的候选字后面都画 *，corrected_from 原串不再画在候选字下面（{@link #visibleAnnotation}），读屏描述里照旧念它。
 *
 * 与 iOS、鸿蒙同一条规则，只换颜色、不画下划线。坐上首选的纠错行总在高亮的首选 chip 上，未用候选包时那里的候选字本来就是强调色，看得出的标记是注释从 58% 透明变成标记色实色。
 */
public final class CandidateCorrectionMarkPolicy {
    /** View JSON 候选对象上的字段名，候选条和展开面板都从 `candidate.opt(FIELD)` 读。 */
    public static final String FIELD = "confident_correction";
    /** View JSON 候选上「引擎改过这一行的拼写」的字段（`corrected_from` 非空），所有纠错行都带，不只坐上首选的那一行。 */
    public static final String CORRECTED_FIELD = "corrected";
    /** 纠错行在候选字后面画的标记，与 Windows、macOS、Linux 候选窗的 `*` 相同。 */
    public static final String CORRECTED_MARK = "*";
    /** 读屏描述末尾补的一段。 */
    public static final String ACCESSIBILITY_SUFFIX = "，已纠正";
    /** 不带标记的注释沿用的调淡比例，和改动前的 candidateLabel 一致。 */
    static final float PLAIN_ANNOTATION_ALPHA = 0.58f;

    private CandidateCorrectionMarkPolicy() {}

    /** `raw` 是 `candidate.opt(FIELD)` 的值：只有 JSON 布尔真才算；字段缺省（null）、false、字符串 "true"、数字 1 都不算，免得格式不对的视图把普通候选标成纠错。 */
    public static boolean confident(Object raw) {
        return JsonPolicy.strictTrue(raw);
    }

    /** 候选条和展开面板上画的候选字：纠错行后面跟 {@link #CORRECTED_MARK}，其余原样。读屏描述不经过这里，仍念候选字本身。 */
    public static String markedWord(String text, boolean corrected) {
        return corrected ? text + CORRECTED_MARK : text;
    }

    /**
     * 候选字下面画的注释：纠错行没有辅助码时，引擎把用户打的原串（`corrected_from`，如 `zhognguo`）放进注释，它和读音行、输入框里的拼写是同一串，画在每个纠错候选下面只是重复，还让候选行多出一行。这里去掉它，纠错改由候选字后面的 * 标出。
     *
     * <p>辅助码以「(」开头，照旧画；五笔方案里混输拼音的纠错行是「原串 五笔码」，只去掉打头的原串。原串是折叠后的字母（九键是数字），不含空格。读屏描述仍用引擎的原注释（「，提示：zhognguo」），不经过这里。
     */
    public static String visibleAnnotation(String annotation, boolean corrected) {
        if (!corrected || annotation.isEmpty() || annotation.startsWith("(")) return annotation;
        int space = annotation.indexOf(' ');
        return space < 0 ? "" : annotation.substring(space + 1);
    }

    /** 读屏描述里接在注释和音标后面的那一段；不是坐上首选的纠错行时为空。 */
    public static String accessibilitySuffix(boolean confident) {
        return confident ? ACCESSIBILITY_SUFFIX : "";
    }

    /** 没有数字前缀、没有注释、也不是坐上首选的纠错行时，candidateLabel 把候选字原样交给 CandidateSymbolLabel，不建 SpannableString，和没有这个字段时逐字节一样。 */
    public static boolean plainLabel(String prefix, String annotation, boolean confident) {
        return prefix.isEmpty() && annotation.isEmpty() && !confident;
    }

    /** 盖在候选字上的标记色；不是坐上首选的纠错行时返回 null，候选字不加任何 span，照旧用按钮自己的选中/未选中文字色。 */
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

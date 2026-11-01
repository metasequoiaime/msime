import app.msime.android.CandidateAppearance;
import app.msime.android.CandidateCorrectionMarkPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.KeyboardSkin;
import java.util.Map;

/** 双拼触点纠错坐上首选的显式标记（#10018）：View JSON 候选上的 `confident_correction` 怎么读、读屏补什么、标记画什么颜色。 */
public final class CandidateCorrectionMarkPolicySmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        // 字段名就是契约里的那一个，宿主读的正是 candidate.opt(FIELD)。
        check("confident_correction".equals(CandidateCorrectionMarkPolicy.FIELD), "contract field name");

        // JVM 冒烟没有真的 org.json（android.jar 里只有抛 Stub! 的桩，见 check-host.sh），解析不了 View JSON，这里按 JSONObject.opt 会交出的值来测：`"confident_correction":true` 是 Boolean.TRUE，字段缺省是 null。
        check(CandidateCorrectionMarkPolicy.confident(Boolean.TRUE), "present and true marks the row");
        check(!CandidateCorrectionMarkPolicy.confident(null), "an absent field leaves the row unmarked");
        check(!CandidateCorrectionMarkPolicy.confident(Boolean.FALSE), "false leaves the row unmarked");
        check(!CandidateCorrectionMarkPolicy.confident("true"), "a string is not a JSON boolean");
        check(!CandidateCorrectionMarkPolicy.confident(1), "a number is not a JSON boolean");

        // 读屏：带标记的行在注释和音标之后补「，已纠正」，其余行描述不变。
        check(CandidateCorrectionMarkPolicy.accessibilitySuffix(true).equals("，已纠正"), "spoken mark");
        check(CandidateCorrectionMarkPolicy.accessibilitySuffix(false).isEmpty(), "no spoken mark otherwise");
        check(("候选 1：我国飞行，提示：mwgfxi" + CandidateCorrectionMarkPolicy.accessibilitySuffix(true))
                .equals("候选 1：我国飞行，提示：mwgfxi，已纠正"), "the mark follows the corrected_from hint");

        // 标记色：未用候选包时，选中的 chip 上是首选的字色（chipText，跟随系统皮肤浅色是加深到 WCAG AA 的 #267547），不在选中位置时是皮肤强调色。首选 chip 的字本来就是这个颜色，所以 candidateLabel 在候选字上另加同色下划线（check-host.sh 钉住），注释从调淡的强调色变成实色。
        CandidateAppearance.Palette system = CandidateAppearance.fromSkin(KeyboardSkin.system(false));
        check(system.correctionFor(true) == 0xff267547 && system.correctionFor(false) == 0xff2c7a4b,
            "built-in skins mark the chip with its text colour and other rows with the accent");
        CandidateAppearance.Palette dark = CandidateAppearance.fromSkin(KeyboardSkin.system(true));
        check(dark.correctionFor(true) == dark.accent() && dark.correctionFor(false) == dark.accent(),
            "the dark skin marks with its own accent");
        // 候选包把选中 chip 填成强调色、字配白色时，chip 上的标记取包的选中文字色，不能是和底同色的强调色；不在选中位置时仍取强调色。
        CandidateAppearance.Palette pack = CandidateAppearance.fromResolved(system, "pack",
            Map.of("accent", "#FF8800FF", "selected", "#FF8800FF", "selected_text", "#FFFFFFFF"));
        check(pack.correctionFor(true) == 0xffffffff, "a pack's selected chip keeps its readable text colour");
        check(pack.correctionFor(false) == 0xffff8800, "an unselected row takes the pack accent");

        // 只有标记行改变画法：不带标记时候选字不加 span（wordMark 为 null），注释颜色和改动前的 candidateLabel 一模一样（包取 secondaryFor，否则候选字色调到 58%）；没有前缀和注释时整段不建 span，逐字节同改动前。
        for (CandidateAppearance.Palette palette : new CandidateAppearance.Palette[] {system, dark, pack}) {
            for (boolean highlighted : new boolean[] {true, false}) {
                String where = palette.id() + (highlighted ? " highlighted" : " plain");
                check(CandidateCorrectionMarkPolicy.wordMark(palette, highlighted, false) == null,
                    "an unmarked word keeps the button's own colour: " + where);
                int before = palette.externalSkin() ? palette.secondaryFor(highlighted)
                    : ColorPolicy.withAlpha(palette.textFor(highlighted), 0.58f);
                check(CandidateCorrectionMarkPolicy.annotationColour(palette, highlighted, false) == before,
                    "an unmarked annotation keeps today's colour: " + where);
                Integer mark = CandidateCorrectionMarkPolicy.wordMark(palette, highlighted, true);
                check(mark != null && mark == palette.correctionFor(highlighted), "a marked word takes the mark: " + where);
                check(CandidateCorrectionMarkPolicy.annotationColour(palette, highlighted, true) == palette.correctionFor(highlighted),
                    "a marked annotation takes the solid mark: " + where);
            }
        }
        // 内置皮肤上标记行的注释确实和普通首选不同：普通首选的注释是 58% 的强调色，标记行是实色。
        check(CandidateCorrectionMarkPolicy.annotationColour(system, true, true)
                != CandidateCorrectionMarkPolicy.annotationColour(system, true, false), "the marked annotation is visibly different");
        check(CandidateCorrectionMarkPolicy.plainLabel("", "", false), "a bare word stays a plain label");
        check(!CandidateCorrectionMarkPolicy.plainLabel("", "", true), "a marked bare word gets its spans");
        check(!CandidateCorrectionMarkPolicy.plainLabel("1 ", "", false), "a numbered word gets its spans");
        check(!CandidateCorrectionMarkPolicy.plainLabel("", "mwgfxi", false), "an annotated word gets its spans");
        System.out.println("Android confident correction mark: field, spoken suffix, colour and unmarked rows passed");
    }
}

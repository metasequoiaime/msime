import app.msime.android.CandidateAppearance;
import app.msime.android.CandidateCorrectionMarkPolicy;
import app.msime.android.KeyboardSkin;
import java.util.Map;

/** 双拼触点纠错坐上首选的显式标记（#10018）：View JSON 候选上的 `confident_correction` 怎么读、读屏补什么、标记画什么颜色。 */
public final class CandidateCorrectionMarkPolicySmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    // 和 input-runtime 的 Candidate 序列化同形的两行：坐上首选的纠错行多一个 confident_correction，普通纠错行和其余候选不带这个字段（引擎用 skip_serializing_if，默认视图逐字节不变）。
    private static final String WITH_FIELD = "{\"id\":{\"session\":7,\"generation\":3,\"index\":0},\"text\":\"我国飞行\",\"code\":\"wogofeixk\",\"annotation\":\"mwgfxi\",\"source\":11,\"corrected\":true,\"fixed_position\":0,\"highlighted\":true,\"confident_correction\":true}";
    private static final String WITHOUT_FIELD = "{\"id\":{\"session\":7,\"generation\":3,\"index\":1},\"text\":\"没有\",\"code\":\"meiyb\",\"annotation\":\"mwgfxi\",\"source\":11,\"corrected\":true,\"fixed_position\":0,\"highlighted\":false}";

    public static void main(String[] args) {
        // 字段名就是契约里的那一个：带标记的夹具里出现且值为 true，不带的夹具里不出现，宿主读的正是 opt(FIELD)。
        check("confident_correction".equals(CandidateCorrectionMarkPolicy.FIELD), "contract field name");
        check(WITH_FIELD.contains("\"" + CandidateCorrectionMarkPolicy.FIELD + "\":true"),
            "the marked fixture carries the field");
        check(!WITHOUT_FIELD.contains(CandidateCorrectionMarkPolicy.FIELD),
            "the unmarked fixture omits the field");

        // JVM 冒烟没有真的 org.json（android.jar 里只有抛 Stub! 的桩，见 check-host.sh），这里按 JSONObject.opt 对两份夹具给出的值来测：带字段时是 Boolean.TRUE，缺省时是 null。
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

        // 标记色：未用候选包时选中与否都是皮肤强调色（首选 chip 的字本来就是强调色，注释从调淡的强调色变成实色）。
        CandidateAppearance.Palette system = CandidateAppearance.fromSkin(KeyboardSkin.system(false));
        check(system.correctionFor(true) == 0xff2c7a4b && system.correctionFor(false) == 0xff2c7a4b,
            "built-in skins mark with the accent");
        CandidateAppearance.Palette dark = CandidateAppearance.fromSkin(KeyboardSkin.system(true));
        check(dark.correctionFor(true) == dark.accent() && dark.correctionFor(false) == dark.accent(),
            "the dark skin marks with its own accent");
        // 候选包把选中 chip 填成强调色、字配白色时，chip 上的标记取包的选中文字色，不能是和底同色的强调色；不在选中位置时仍取强调色。
        CandidateAppearance.Palette pack = CandidateAppearance.fromResolved(system, "pack",
            Map.of("accent", "#FF8800FF", "selected", "#FF8800FF", "selected_text", "#FFFFFFFF"));
        check(pack.correctionFor(true) == 0xffffffff, "a pack's selected chip keeps its readable text colour");
        check(pack.correctionFor(false) == 0xffff8800, "an unselected row takes the pack accent");
        System.out.println("Android confident correction mark: field, spoken suffix and colour passed");
    }
}

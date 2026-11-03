import app.msime.android.CandidateAppearance;
import app.msime.android.CandidateSelectedTextPolicy;
import app.msime.android.KeyboardSkin;
import java.util.List;

/** #10740：候选选中文字色在 Android 候选条上的规则，以及设置页写它时文档跟着改什么。 */
public final class CandidateSelectedTextPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // 只有自定义主题、合法的 #RRGGBB 才画到候选条上。
        check(CandidateSelectedTextPolicy.stripColour("custom", "#7b3fbf") == 0xff7b3fbf);
        check(CandidateSelectedTextPolicy.stripColour("paper", "#7b3fbf") == null);
        check(CandidateSelectedTextPolicy.stripColour("custom", null) == null);
        check(CandidateSelectedTextPolicy.stripColour("custom", "purple") == null);
        check(CandidateSelectedTextPolicy.stripColour("custom", "#7b3fbf80") == null);
        check(CandidateSelectedTextPolicy.stripColour(null, "#7b3fbf") == null);

        // 没设时候选条照旧：首选候选的字是皮肤的首选字色；设了就换成它，未选中的候选和底色不变。
        List<String> fonts = List.of("Noto Sans SC", "Microsoft YaHei");
        CandidateAppearance.Palette plain = CandidateAppearance.fromSkin(KeyboardSkin.system(false),
            "Noto Sans SC", "", fonts, null);
        check(plain.chipText() == 0xff267547 && plain.textFor(true) == plain.accent());
        CandidateAppearance.Palette picked = CandidateAppearance.fromSkin(KeyboardSkin.system(false),
            "Noto Sans SC", "", fonts, 0xff7b3fbf);
        check(picked.chipText() == 0xff7b3fbf && picked.textFor(true) == 0xff7b3fbf);
        check(picked.correctionFor(true) == 0xff7b3fbf);
        check(picked.textFor(false) == plain.textFor(false) && picked.chip() == plain.chip());
        check(picked.surface() == plain.surface() && picked.accent() == plain.accent());
        check(!picked.key().equals(plain.key()));

        // 选颜色时选上自定义主题；原来不是自定义主题时，当时的主题作底并清掉两个皮肤槽位；交还给主题时别的都不动。
        CandidateSelectedTextPolicy.Change fromNight = CandidateSelectedTextPolicy.change("night", false);
        check(fromNight.selectCustom() && fromNight.resetSkins() && "night".equals(fromNight.base()));
        CandidateSelectedTextPolicy.Change fromNative = CandidateSelectedTextPolicy.change("native", false);
        check("system".equals(fromNative.base()));
        CandidateSelectedTextPolicy.Change again = CandidateSelectedTextPolicy.change("custom", false);
        check(again.selectCustom() && !again.resetSkins() && again.base() == null);
        CandidateSelectedTextPolicy.Change clear = CandidateSelectedTextPolicy.change("night", true);
        check(!clear.selectCustom() && !clear.resetSkins() && clear.base() == null);

        check("跟随主题".equals(CandidateSelectedTextPolicy.label(null)));
        check("紫色".equals(CandidateSelectedTextPolicy.label("#7b3fbf")));
        check("#123456".equals(CandidateSelectedTextPolicy.label("#123456")));
    }
}

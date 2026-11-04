import app.msime.android.CandidateAppearance;
import app.msime.android.CandidateSelectedTextPolicy;
import app.msime.android.KeyboardSkin;
import java.util.List;

/** #10740：共享候选选中文字色的 Android 颜色适配与候选条绘制。主题和写入规则由 Rust 测试覆盖。 */
public final class CandidateSelectedTextPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // 主题是否生效由共享层决定，宿主只把合法 RGB 转成平台颜色。
        check(CandidateSelectedTextPolicy.resolvedColour("#7b3fbf") == 0xff7b3fbf);
        check(CandidateSelectedTextPolicy.resolvedColour(null) == null);
        check(CandidateSelectedTextPolicy.resolvedColour("purple") == null);
        check(CandidateSelectedTextPolicy.resolvedColour("#7b3fbf80") == null);

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

        check("跟随主题".equals(CandidateSelectedTextPolicy.label(null)));
        check("紫色".equals(CandidateSelectedTextPolicy.label("#7b3fbf")));
        check("#123456".equals(CandidateSelectedTextPolicy.label("#123456")));
    }
}

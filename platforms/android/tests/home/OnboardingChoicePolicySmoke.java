import app.msime.android.AppEdition;
import app.msime.android.KeyboardScheme;
import app.msime.android.home.OnboardingChoicePolicy;
import app.msime.android.home.OnboardingChoicePolicy.Footnote;

/**
 * 引导页在词库准备完成前记下的方案和「显示译文」：卡片怎么显示、偏好可读后写什么、脚注说什么。读写共享偏好的那一半（`OnboardingChoices`）要 org.json 和 SharedPreferences，这里跑不了。
 */
public final class OnboardingChoicePolicySmoke {
    public static void main(String[] arguments) {
        AppEdition full = AppEdition.FULL;
        AppEdition pinyin = AppEdition.of("pinyin", "quanpin,shuangpin", "quanpin", false);

        // 偏好读不到时卡片按待保存的选择显示；都没有时一张都不选，偏好可读后按存的显示。
        check(OnboardingChoicePolicy.displayedScheme(KeyboardScheme.WUBI, null) == KeyboardScheme.WUBI,
            "a pending choice shows before preferences are readable");
        check(OnboardingChoicePolicy.displayedScheme(KeyboardScheme.WUBI, KeyboardScheme.QUANPIN) == KeyboardScheme.WUBI,
            "a pending choice wins over the stored scheme until it is written");
        check(OnboardingChoicePolicy.displayedScheme(null, KeyboardScheme.QUANPIN) == KeyboardScheme.QUANPIN,
            "without a pending choice the stored scheme shows");
        check(OnboardingChoicePolicy.displayedScheme(null, null) == null, "nothing selected while nothing is known");

        // 待保存的 id：不认识的和本版本不提供的都当作没选，版本规则不因为提前选择而被绕过。
        check(OnboardingChoicePolicy.pendingScheme("nine_key", full) == KeyboardScheme.QUANPIN_NINE_KEY, "known id");
        check(OnboardingChoicePolicy.pendingScheme(null, full) == null, "no pending id");
        check(OnboardingChoicePolicy.pendingScheme("not-a-scheme", full) == null, "unknown id");
        check(OnboardingChoicePolicy.pendingScheme("wubi", pinyin) == null, "a scheme the edition lacks is dropped");
        check(OnboardingChoicePolicy.schemeToWrite("wubi", KeyboardScheme.QUANPIN, pinyin) == null,
            "a scheme the edition lacks is never written");

        // 偏好可读后要写的方案：与存的相同不写；不同就写。首次安装存的是出厂默认全拼，用户提前选的其它方案不能被它盖住。
        check(OnboardingChoicePolicy.schemeToWrite("quanpin", KeyboardScheme.QUANPIN, full) == null,
            "the factory default needs no write when it was chosen");
        check(OnboardingChoicePolicy.schemeToWrite("wubi", KeyboardScheme.QUANPIN, full) == KeyboardScheme.WUBI,
            "an early pick over the factory default is written");
        check(OnboardingChoicePolicy.schemeToWrite("nine_key", KeyboardScheme.QUANPIN, full) == KeyboardScheme.QUANPIN_NINE_KEY,
            "nine-key is a different entry from 26-key quanpin");
        check(OnboardingChoicePolicy.schemeToWrite("xiaohe", KeyboardScheme.QUANPIN, full) == KeyboardScheme.XIAOHE,
            "a first double-pinyin pick lands on xiaohe");
        check(OnboardingChoicePolicy.schemeToWrite(null, KeyboardScheme.QUANPIN, full) == null, "nothing pending");

        // 偏好没读到时「双拼」卡片只能代表小鹤；偏好里已经是另一种双拼时，提前点的双拼不把它改回小鹤。
        check(OnboardingChoicePolicy.schemeToWrite("xiaohe", KeyboardScheme.ZIRANMA, full) == null,
            "an early double-pinyin pick keeps the chosen profile");
        check(OnboardingChoicePolicy.shuangpinCard(null, null) == KeyboardScheme.XIAOHE, "default xiaohe card");
        check(OnboardingChoicePolicy.shuangpinCard(null, KeyboardScheme.ZIRANMA) == KeyboardScheme.ZIRANMA,
            "the card follows the stored profile");
        check(OnboardingChoicePolicy.shuangpinCard(KeyboardScheme.XIAOHE, KeyboardScheme.ZIRANMA) == KeyboardScheme.XIAOHE,
            "the card the user tapped stays selected until the choice is resolved");
        check(OnboardingChoicePolicy.shuangpinCard(KeyboardScheme.WUBI, KeyboardScheme.MICROSOFT) == KeyboardScheme.MICROSOFT,
            "a non-double-pinyin pick leaves the card on the stored profile");
        // 卡片与显示的选择必须对得上，否则用户点了双拼却看不到它被选中。
        KeyboardScheme card = OnboardingChoicePolicy.shuangpinCard(KeyboardScheme.XIAOHE, null);
        check(card == OnboardingChoicePolicy.displayedScheme(KeyboardScheme.XIAOHE, null),
            "the double-pinyin card shows as selected right after the tap");

        // 显示译文：没有待保存的值或与存的相同都不写。
        check(OnboardingChoicePolicy.glossToWrite(null, false) == null, "no pending gloss");
        check(OnboardingChoicePolicy.glossToWrite(Boolean.TRUE, true) == null, "same gloss needs no write");
        check(Boolean.TRUE.equals(OnboardingChoicePolicy.glossToWrite(Boolean.TRUE, false)), "gloss turned on early");
        check(Boolean.FALSE.equals(OnboardingChoicePolicy.glossToWrite(Boolean.FALSE, true)), "gloss turned off early");

        // 脚注：准备失败时先说失败（能重试），而不是一直「正在读取」或「还在准备」。
        check(OnboardingChoicePolicy.footnote(true, true, false, false) == Footnote.NONE, "readable and settled");
        check(OnboardingChoicePolicy.footnote(true, true, false, true) == Footnote.SAVING, "readable with a write queued");
        check(OnboardingChoicePolicy.footnote(true, true, true, false) == Footnote.NONE,
            "a stale failure state does not hide readable preferences");
        check(OnboardingChoicePolicy.footnote(false, false, false, false) == Footnote.READING, "first read in flight");
        check(OnboardingChoicePolicy.footnote(false, false, true, true) == Footnote.FAILED, "failure before the first read");
        check(OnboardingChoicePolicy.footnote(false, true, true, false) == Footnote.FAILED, "preparation failed");
        check(OnboardingChoicePolicy.footnote(false, true, false, true) == Footnote.CHOSEN_WAITING, "chosen, waiting");
        check(OnboardingChoicePolicy.footnote(false, true, false, false) == Footnote.WAITING, "waiting, nothing chosen");
        check(OnboardingChoicePolicy.footnote(false, false, false, true) == Footnote.READING,
            "a choice made during the first read still says reading");

        System.out.println("Android onboarding choice policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

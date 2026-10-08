import app.msime.android.InputFeatureToggle;
import app.msime.android.InputFeatureToggle.Group;
import java.util.ArrayList;
import java.util.List;

public final class InputFeatureToggleSmoke {
    public static void main(String[] args) {
        List<InputFeatureToggle> all = List.of(InputFeatureToggle.values());
        check(!all.isEmpty(), "there are toggles to show");

        // A duplicated key would make two switches fight over one preference, and the second one
        // rendered would silently be the only one that matters.
        List<String> keys = new ArrayList<>();
        for (InputFeatureToggle toggle : all) {
            check(!toggle.key().isEmpty() && toggle.key().matches("[a-z_]+"),
                "a shared preference key: " + toggle.key());
            check(!keys.contains(toggle.key()), "no key appears twice: " + toggle.key());
            keys.add(toggle.key());
            check(!toggle.title().isEmpty() && !toggle.description().isEmpty(),
                "every switch says what it is: " + toggle.key());
        }

        // Every toggle reaches a sheet: one grouped into nothing is a setting nobody can change.
        int grouped = 0;
        for (Group group : InputFeatureToggle.groups()) {
            check(!InputFeatureToggle.of(group).isEmpty(), "a listed group has entries");
            check(!group.title().isEmpty(), "a group is titled");
            grouped += InputFeatureToggle.of(group).size();
        }
        check(grouped == all.size(), "every toggle belongs to a listed group");

        // 默认值必须跟 crates/client-core 的 Preferences 一致：一个默认开的设置在界面上画成关，
        // 说的就是键盘行为的反面，而「打开」它写进去的又是本来就在的值。
        check(enabledByDefault("learning"), "learning is on by default");
        // 自动纠错 is the two nested `quanpin` corrections, each with its own row in the sheet, not one top-level switch; scripts/test-android-preference-keys.py checks every key the sheets write against the shared schema.
        check(!hasToggle("autocorrect"), "there is no all-types autocorrect switch");
        // 云候选 sends the composing spelling off the device, so new installs leave it off (preferences.rs `cloud_candidates: false`); candidate translations stay on because they only go online through the opt-in translation_account below.
        check(!enabledByDefault("cloud_candidates"), "cloud candidates are off by default");
        check(enabledByDefault("candidate_translations"), "candidate translations are on by default");
        check(enabledByDefault("english_suggestions") && enabledByDefault("chinese_punctuation"),
            "suggestions and Chinese punctuation are on by default");
        check(enabledByDefault("smart_punctuation") && enabledByDefault("paired_punctuation"),
            "both punctuation aids are on by default off Windows");
        check(!enabledByDefault("traditional_chinese_output"), "simplified output is the default");
        check(!enabledByDefault("clipboard_history"), "clipboard history is off by default");
        check(!enabledByDefault("candidate_english_gloss"), "the gloss is off by default");
        check(!enabledByDefault("translation_account"), "the account translation endpoint is opt-in");

        check(InputFeatureToggle.of(Group.PRIVACY).stream()
                .anyMatch(toggle -> "clipboard_history".equals(toggle.key())),
            "clipboard history is grouped where its consequence is stated");
        check(InputFeatureToggle.of(Group.PRIVACY).stream()
                .anyMatch(toggle -> "translation_account".equals(toggle.key())),
            "the account translation switch is grouped where its consequence is stated");
        System.out.println("Android input feature toggles: keys, grouping and shared defaults passed");
    }

    private static boolean hasToggle(String key) {
        for (InputFeatureToggle toggle : InputFeatureToggle.values()) {
            if (toggle.key().equals(key)) return true;
        }
        return false;
    }

    private static boolean enabledByDefault(String key) {
        for (InputFeatureToggle toggle : InputFeatureToggle.values()) {
            if (toggle.key().equals(key)) return toggle.enabledByDefault();
        }
        throw new AssertionError("Missing toggle: " + key);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

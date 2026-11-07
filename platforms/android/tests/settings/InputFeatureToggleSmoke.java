import app.msime.android.InputFeatureToggle;
import app.msime.android.InputFeatureToggle.Group;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public final class InputFeatureToggleSmoke {
    public static void main(String[] args) throws IOException {
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
        check(enabledByDefault("backspace_reverts_selection"), "selection retreat stays enabled for existing users");
        check(enabledByDefault("learning"), "learning is on by default");
        check(enabledByDefault("touch_japanese_toggle_input"), "Japanese toggle input keeps its legacy default");
        check(all.stream().anyMatch(toggle -> "touch_japanese_toggle_input".equals(toggle.key())
                && "日语".equals(toggle.group().title())), "Japanese input has its own settings group");
        // 自动纠错 is the two nested `quanpin` corrections, each with its own row in the sheet, not one top-level switch; scripts/test-android-preference-keys.py checks every key the sheets write against the shared schema.
        check(!hasToggle("autocorrect"), "there is no all-types autocorrect switch");
        // 新装默认关闭云候选和候选翻译；账号翻译仍是下面独立的主动开关。
        check(!enabledByDefault("cloud_candidates"), "cloud candidates are off by default");
        check(!enabledByDefault("candidate_translations"), "candidate translations are off on a new install");
        check(enabledByDefault("english_suggestions") && enabledByDefault("chinese_punctuation"),
            "suggestions and Chinese punctuation are on by default");
        check(enabledByDefault("smart_punctuation") && enabledByDefault("paired_punctuation"),
            "both punctuation aids are on by default off Windows");
        check(!enabledByDefault("traditional_chinese_output"), "simplified output is the default");
        check(!enabledByDefault("chinese_english_spacing"), "no space is added between Chinese and English by default");
        check(InputFeatureToggle.of(Group.OUTPUT).stream()
                .anyMatch(toggle -> "chinese_english_spacing".equals(toggle.key())),
            "Chinese-English spacing sits with the other output switches");
        check(!enabledByDefault("single_character_only"), "phrases are offered by default");
        check(!enabledByDefault("clipboard_history"), "clipboard history is off by default");
        check(!enabledByDefault("candidate_english_gloss"), "the gloss is off by default");
        check(!enabledByDefault("candidate_pronunciation"), "the pronunciation is off by default");
        check(!enabledByDefault("translation_account"), "the account translation endpoint is opt-in");

        check(InputFeatureToggle.of(Group.PRIVACY).stream()
                .anyMatch(toggle -> "clipboard_history".equals(toggle.key())),
            "clipboard history is grouped where its consequence is stated");
        check(InputFeatureToggle.of(Group.PRIVACY).stream()
                .anyMatch(toggle -> "translation_account".equals(toggle.key())),
            "the account translation switch is grouped where its consequence is stated");
        wubiGroup();
        System.out.println("Android input feature toggles: keys, grouping and shared defaults passed");
    }

    /** #10830：输入页五笔组的四个开关。默认值跟 client-core 的 `Preferences` 一致（混输缺键时跟键盘服务的读法一样按关），标题和说明与共享设置页 wubi-section.tsx 一字不差，Windows 和 macOS 的原生设置页用的也是同一套字。 */
    private static void wubiGroup() throws IOException {
        List<String> keys = new ArrayList<>();
        for (InputFeatureToggle toggle : InputFeatureToggle.of(Group.WUBI)) keys.add(toggle.key());
        check(keys.equals(List.of("wubi_mixed_pinyin", "wubi_code_hint", "wubi_fifth_code_top_commit", "wubi_auto_commit_unique")),
            "the Wubi group lists mixed input, code hint, fifth-code top commit and unique auto commit in the shared order: " + keys);
        check(!enabledByDefault("wubi_mixed_pinyin"), "a snapshot without wubi_mixed_pinyin reads as off, as MSIMEInputService reads it");
        check(enabledByDefault("wubi_code_hint"), "the remaining-code hint is on by default");
        check(enabledByDefault("wubi_fifth_code_top_commit"), "the fifth code commits the first candidate by default");
        check(enabledByDefault("wubi_auto_commit_unique"), "a unique four-code candidate commits by default");
        String shared = new String(Files.readAllBytes(repoRoot().resolve("packages/ui/src/settings/wubi-section.tsx")), StandardCharsets.UTF_8);
        Matcher row = Pattern.compile("title=\"([^\"]+)\"\\s+description=\"([^\"]+)\"\\s+checked=\\{state\\.(\\w+)\\}").matcher(shared);
        int matched = 0;
        while (row.find()) {
            InputFeatureToggle toggle = toggle(row.group(3));
            if (toggle == null || toggle.group() != Group.WUBI) continue;
            check(toggle.title().equals(row.group(1)), "the Wubi switch is titled as in wubi-section.tsx: " + row.group(1));
            check(toggle.description().equals(row.group(2)), "the Wubi switch is described as in wubi-section.tsx: " + row.group(3));
            matched++;
        }
        check(matched == keys.size(), "every Android Wubi switch has its row in wubi-section.tsx, found " + matched);
    }

    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve("packages/ui/src/settings/wubi-section.tsx"))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; wubi-section.tsx not found above user.dir");
    }

    private static InputFeatureToggle toggle(String key) {
        for (InputFeatureToggle toggle : InputFeatureToggle.values()) {
            if (toggle.key().equals(key)) return toggle;
        }
        return null;
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

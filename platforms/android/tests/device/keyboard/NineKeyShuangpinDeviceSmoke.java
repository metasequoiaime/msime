package app.msime.android.test;

import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 九键双拼（#6572）的设备验收：键面印着米旮旯 3 每个数字键上的声母和韵母（2 键是 b c 和零声母、a ai iang，0 键是 u ü ei）；6 4 4 7 是 ni'hao，打出「你好」，输入框里写拼音读音而不是数字；0 是组码（5 0 是 lu）；最后一行的分词键把 8 和 4 切成两个声母，按简拼出「中国」；换成米旮旯 2 后键面跟着换（零声母在 6 键，0 键不再是组码）。
 */
public final class NineKeyShuangpinDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "nine-key shuangpin faces from the table, two digits per syllable, 0 as a code key, separator initials and a table switch";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()),
            StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin")
            .put("touch_keyboard_layout", "nine_key_shuangpin")
            .put("nine_key_shuangpin_profile", "migala3")
            .put("traditional_chinese_output", false)
            .put("candidate_english_gloss", false)
            .put("mixed_input", new JSONObject().put("english", false)
                .put("minimum_prefix", 2).put("emoji", false).put("kaomoji", false))
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin").put("nine_key_shuangpin"))
                .put("selected", "nine_key_shuangpin"));
        try {
            stage = "enable shared nine-key shuangpin preferences";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));

            stage = "Mi Ga La 3 faces";
            await(described("按键 2，声母 b c 和零声母，韵母 a ai iang"));
            await(described("按键 0，韵母 u ü ei"));
            await(describedPrefix("按键 8，声母 t sh zh"));
            await(described("按键 符号"));

            // 你好：ni 是 6 4，hao 是 4 7。按下的是数字，输入框里写的是引擎给的拼音读音，没有数字。
            stage = "nihao in two digits per syllable";
            for (String digit : new String[] {"6", "4", "4", "7"}) tap(describedPrefix("按键 " + digit + "，"));
            await(described("按键 分词，在这里断开音节"));
            await(field("msime-test-plain").and(node -> !emptyField(node)
                && !node.getText().toString().matches(".*[0-9].*")));
            stage = "nihao commit";
            tap(candidate("你好"));
            await(field("msime-test-plain").and(node -> equalsText("你好", node.getText())));

            // 0 是组码：lu 是 5 0。
            stage = "zero key";
            tap(describedPrefix("按键 5，"));
            tap(described("按键 0，韵母 u ü ei"));
            tap(candidate("路"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().endsWith("你好路")));

            // 分词键把 8 和 4 切成两个声母：zh 和 g，按简拼出「中国」。
            stage = "separator splits two initials";
            tap(describedPrefix("按键 8，"));
            tap(described("按键 分词，在这里断开音节"));
            tap(describedPrefix("按键 4，"));
            tap(candidate("中国"));
            await(field("msime-test-plain").and(node -> node.getText() != null
                && node.getText().toString().endsWith("路中国")));

            // 换成米旮旯 2：键面跟着表换，零声母在 6 键，0 键不再是组码。
            stage = "switch to Mi Ga La 2";
            JSONObject migala2 = new JSONObject(base.toString()).put("nine_key_shuangpin_profile", "migala2");
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 2).put("preferences", migala2).toString()
                .getBytes(StandardCharsets.UTF_8));
            await(describedPrefix("按键 6，声母 m n 和零声母"));
            await(described("按键 数字 0"));
            awaitGone(described("按键 0，韵母 u ü ei"));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }

    private static boolean emptyField(AccessibilityNodeInfo node) {
        return node.getText() == null || node.getText().length() == 0 || node.isShowingHintText();
    }

    /** 候选条上写着 `text` 的候选（描述是「候选 N：text；…」）。 */
    private Predicate<AccessibilityNodeInfo> candidate(String text) {
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || !node.isClickable() || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equals(text);
        };
    }

    private void awaitGone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean present = false;
            for (AccessibilityWindowInfo window : automation.getWindows()) {
                if (find(window.getRoot(), match) != null) {
                    present = true;
                    break;
                }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic UI state did not go away; IME showed " + imeTexts());
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }
}

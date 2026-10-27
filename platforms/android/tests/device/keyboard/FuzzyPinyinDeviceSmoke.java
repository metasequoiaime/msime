package app.msime.android.test;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import org.json.JSONArray;
import org.json.JSONObject;

/** 验证引擎生成模糊拼音候选，以及关闭开关后保留规则的行为。 */
public final class FuzzyPinyinDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "fuzzy-pinyin candidate generation, native rule selection and persistence";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(
            Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString());
        base.put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("quanpin", new JSONObject().put("autocorrect_transposition", false)
                .put("autocorrect_neighbor", false));
        try {
            stage = "enable fuzzy pinyin in shared preferences";
            JSONObject enabled = new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", new JSONObject(base.toString()).put("fuzzy_pinyin",
                    new JSONObject().put("enabled", true).put("rules", new JSONArray().put("z-zh"))));
            publish(preferences, enabled.toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            stage = "fuzzy candidate generation";
            for (String key : new String[] {"z", "o", "n", "g", "g", "u", "o"}) tap(key(key));
            await(candidate("中国"));

            stage = "disable fuzzy pinyin while retaining selected rule";
            JSONObject disabled = new JSONObject().put("format_version", 1).put("revision", revision + 2)
                .put("preferences", new JSONObject(base.toString()).put("fuzzy_pinyin",
                    new JSONObject().put("enabled", false).put("rules", new JSONArray().put("z-zh"))));
            publish(preferences, disabled.toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            openEditor();
            tap(field("msime-test-plain"));
            for (String key : new String[] {"z", "o", "n", "g", "g", "u", "o"}) tap(key(key));
            if (findVisible(candidate("中国")) != null) throw new AssertionError("disabled fuzzy rule remained active");
            JSONObject persisted = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8));
            JSONObject fuzzy = persisted.getJSONObject("preferences").getJSONObject("fuzzy_pinyin");
            if (fuzzy.getBoolean("enabled") || !fuzzy.getJSONArray("rules").toString().contains("z-zh")) {
                throw new AssertionError("disabled fuzzy rule was not retained");
            }

            stage = "native fuzzy rules select independently";
            openRuleSettings();
            tapSetting("z ↔ zh");
            awaitFuzzy(preferences, value -> !hasRule(value, "z-zh") && value.optBoolean("seeded", false));
            tapSetting("an ↔ ang");
            awaitFuzzy(preferences, value -> hasRule(value, "an-ang") && !hasRule(value, "z-zh"));
            tapSetting("en ↔ eng");
            awaitFuzzy(preferences, value -> hasRule(value, "an-ang") && hasRule(value, "en-eng"));
            tapSetting("en ↔ eng");
            awaitFuzzy(preferences, value -> hasRule(value, "an-ang") && !hasRule(value, "en-eng"));

            stage = "native fuzzy rules persist through first enable and reopen";
            openRuleSettings();
            tapSetting("启用模糊音");
            awaitFuzzy(preferences, value -> value.optBoolean("enabled", false)
                && hasRule(value, "an-ang") && !hasRule(value, "z-zh"));
            openRuleSettings();
            await(setting("z ↔ zh").and(node -> !node.isChecked()));
            scrollToSetting("an ↔ ang");
            await(setting("an ↔ ang").and(AccessibilityNodeInfo::isChecked));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
        }
    }

    private Predicate<android.view.accessibility.AccessibilityNodeInfo> candidate(String text) {
        // 规则只保证候选出现；词库更新或学习可以改变其排名。
        return node -> {
            if (!equalsText("app.msime.android", node.getPackageName())
                    || node.getContentDescription() == null) return false;
            String description = node.getContentDescription().toString();
            int delimiter = description.indexOf('：');
            if (!description.startsWith("候选 ") || delimiter < 0) return false;
            String value = description.substring(delimiter + 1);
            String head = value.contains("；") ? value.substring(0, value.indexOf('；')) : value;
            return head.equals(text);
        };
    }

    private android.view.accessibility.AccessibilityNodeInfo findVisible(
            Predicate<android.view.accessibility.AccessibilityNodeInfo> predicate) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
            android.view.accessibility.AccessibilityNodeInfo found = find(window.getRoot(), predicate);
            if (found != null) return found;
        }
        return null;
    }


    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private void openRuleSettings() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page TYPING");
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page FUZZY_PINYIN");
        await(node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("启用模糊音", node.getText()));
    }

    private Predicate<AccessibilityNodeInfo> setting(String label) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.isCheckable()
            && find(node, child -> equalsText(label, child.getText())) != null;
    }

    private void scrollToSetting(String label) throws Exception {
        for (int attempt = 0; attempt < 8; attempt++) {
            if (findVisible(setting(label).and(AccessibilityNodeInfo::isVisibleToUser)) != null) return;
            await(node -> equalsText("app.msime.android", node.getPackageName()) && node.isScrollable())
                .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
        }
        throw new AssertionError("Fuzzy rule setting is not reachable: " + label);
    }

    private void tapSetting(String label) throws Exception {
        scrollToSetting(label);
        tap(setting(label));
    }

    private void awaitFuzzy(File preferences, Predicate<JSONObject> expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        while (SystemClock.uptimeMillis() < deadline) {
            JSONObject saved = new JSONObject(new String(Files.readAllBytes(preferences.toPath()), StandardCharsets.UTF_8));
            if (expected.test(saved.getJSONObject("preferences").getJSONObject("fuzzy_pinyin"))) return;
            SystemClock.sleep(100);
        }
        throw new AssertionError("Fuzzy rule preference was not saved");
    }

    private boolean hasRule(JSONObject fuzzy, String id) {
        JSONArray rules = fuzzy.optJSONArray("rules");
        if (rules == null) return false;
        for (int index = 0; index < rules.length(); index++) if (id.equals(rules.optString(index))) return true;
        return false;
    }


}

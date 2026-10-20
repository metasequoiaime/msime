package app.msime.android.test;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 中文全拼触屏分词、空闲点击、连续输入和英文大小写的设备验收。 */
public final class QuanpinSeparatorDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Quanpin separator, idle no-op, xi'an commit, repeated taps and English Shift";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false).put("candidate_english_gloss", false)
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin").put("xiaohe"))
                .put("selected", "quanpin"));
        try {
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", base).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            stage = "idle separator keeps Chinese and inserts nothing";
            await(key("分词"));
            for (int i = 0; i < 3; i++) tap(key("分词"));
            await(field("msime-test-plain").and(node -> node.getText() == null
                || node.getText().length() == 0 || node.isShowingHintText()));
            await(key("中"));

            stage = "separator reaches the current pinyin composition";
            String committedPrefix = "";
            for (int i = 0; i < 3; i++) {
                tap(described("字母 X"));
                tap(described("字母 I"));
                tap(key("分词"));
                tap(described("字母 A"));
                tap(described("字母 N"));
                await(field("msime-test-plain").and(node -> node.getText() != null
                    && node.getText().toString().endsWith("xi'an")));
                tap(node -> node.isClickable() && node.getContentDescription() != null
                    && node.getContentDescription().toString().matches("候选 [0-9]+：西安(?:；.*)?"));
                committedPrefix += "西安";
                final String committed = committedPrefix;
                await(field("msime-test-plain").and(node -> equalsText(committed, node.getText())));
            }
            stage = "Chinese lowercase state still displays uppercase faces";
            await(key("Q"));
            await(key("A"));
            await(key("Z"));
            tap(key("中"));
            stage = "English switch refreshes every letter row without Shift";
            await(key("q"));
            await(key("a"));
            await(key("z"));
            tap(key("q"));
            await(field("msime-test-plain").and(node -> equalsText("西安西安西安q", node.getText())));
            tap(key("英"));
            stage = "Chinese switch restores uppercase faces without Shift";
            await(key("Q"));
            await(key("A"));
            await(key("Z"));
            stage = "English restores Shift and uppercase input";
            tap(key("中"));
            await(key("⇧"));
            tap(key("⇧"));
            tap(described("大写 A"));
            await(field("msime-test-plain").and(node -> equalsText("西安西安西安qA", node.getText())));
            stage = "Chinese restores the separator";
            tap(key("英"));
            await(key("分词"));
            stage = "password field keeps Shift";
            tap(field("msime-test-password"));
            await(key("⇧"));
            tap(field("msime-test-plain"));
            await(key("分词"));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
        }
    }
}

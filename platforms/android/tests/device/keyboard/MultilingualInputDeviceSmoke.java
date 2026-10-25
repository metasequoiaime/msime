package app.msime.android.test;

import android.view.KeyEvent;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在产品 IME 和合成编辑器中验证连续输入及逐段简繁投影。 */
public final class MultilingualInputDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "multilingual IME: Chinese, English and Japanese candidate, live script toggle and matching editor commit";
    }

    /** 后台模型只在下一次输入时参与；冷设备按实际候选状态重试最后一个合成按键。 */
    private void awaitPreparedCandidate(String text, String lastLetter) throws Exception {
        long deadline = android.os.SystemClock.uptimeMillis() + 15000;
        do {
            for (var window : getUiAutomation().getWindows()) {
                if (find(window.getRoot(), imeTextContains(text)) != null) return;
            }
            tap(key("⌫"));
            tap(key(lastLetter));
        } while (android.os.SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic multilingual candidate was not ready");
    }

    @Override protected void runChecks() throws Exception {
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(getTargetContext().getFilesDir(), "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        JSONObject configured = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("traditional_chinese_output", false).put("chinese_english_spacing", false)
            .put("quanpin_helpcode", new JSONObject().put("enabled", false).put("schema", "ziranma"))
            .put("multilingual_input", new JSONObject().put("enabled", true)
                .put("languages", new JSONArray().put("chinese").put("english").put("japanese")));
        JSONObject mixed = configured.optJSONObject("mixed_input");
        if (mixed == null) {
            mixed = new JSONObject().put("english", true).put("minimum_prefix", 5)
                .put("emoji", false).put("kaomoji", false);
            configured.put("mixed_input", mixed);
        }
        mixed.put("enabled", true);
        JSONObject keybindings = configured.optJSONObject("keybindings");
        if (keybindings == null) {
            keybindings = new JSONObject();
            configured.put("keybindings", keybindings);
        }
        keybindings.put("toggle_character_set_ctrl_shift_f", true);
        try {
            stage = "enable multilingual input";
            publish(preferences, new JSONObject().put("format_version", 1)
                .put("revision", revision + 1).put("preferences", configured).toString()
                .getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            stage = "multilingual simplified candidate";
            for (char letter : "hellowangluogakkou".toCharArray()) tap(key(String.valueOf(letter)));
            awaitPreparedCandidate("hello网络学校", "u");
            stage = "live traditional projection keeps Japanese Han";
            // 组字时工具栏隐藏，使用产品既有快捷键切换，不结束当前组字。
            shellDiscard("input keycombination " + KeyEvent.KEYCODE_CTRL_LEFT + " "
                + KeyEvent.KEYCODE_SHIFT_LEFT + " " + KeyEvent.KEYCODE_F);
            await(imeTextContains("hello網絡学校"));
            stage = "select projected multilingual candidate";
            tap(imeTextContains("hello網絡学校").and(node -> node.isClickable()));
            await(field("msime-test-plain").and(node -> equalsText("hello網絡学校", node.getText())));
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            restartIme();
        }
    }
}

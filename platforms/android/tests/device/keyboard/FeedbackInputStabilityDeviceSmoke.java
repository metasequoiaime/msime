package app.msime.android.test;

import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 只在真实原生反馈页输入合成内容，覆盖文字监听与计数更新，不提交反馈。 */
public final class FeedbackInputStabilityDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "native feedback continuous composition, commits, deletion and hide/show without submission";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.isFile() ? Files.readAllBytes(preferences.toPath()) : null;
        String originalIme = shellText("settings get secure default_input_method").trim();
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        android.content.SharedPreferences modes = getTargetContext().getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        try {
            stage = "prepare feedback input preferences";
            JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                .put("traditional_chinese_output", false).put("ime_mode_scope", "global")
                .put("default_ime_mode", "chinese").put("cloud_candidates", false)
                .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                    .put("selected", "quanpin"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            if (!modes.edit().clear().commit()) throw new AssertionError("Input mode preparation failed");
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page FEEDBACK");
            tap(feedbackField());
            stage = "feedback Chinese mode ready";
            AccessibilityNodeInfo mode = await(describedPrefix("切换到英文").or(describedPrefix("切换到所选输入方案")));
            if (mode.getContentDescription().toString().startsWith("切换到所选")) tap(key("英"));
            stage = "feedback letter keyboard ready";
            await(key("n"));
            stage = "feedback empty field ready";
            // 空 EditText 的无障碍文字可能是提示语；提示语不属于反馈草稿。
            await(feedbackField().and(node -> node.isShowingHintText()
                || node.getText() == null || node.getText().length() == 0));
            String expected = "";
            for (int block = 0; block < 5; block++) {
                for (int phrase = 0; phrase < 10; phrase++) {
                    stage = "feedback phrase " + block + ":" + phrase;
                    for (String letter : new String[] {"n", "i", "h", "a", "o"}) tap(key(letter));
                    tap(key("空格"));
                    expected += "你好";
                    String result = expected;
                    await(feedbackField().and(node -> equalsText(result, node.getText())));
                }
                stage = "feedback deletion " + block;
                tap(key("⌫"));
                expected = expected.substring(0, expected.length() - 1);
                String result = expected;
                await(feedbackField().and(node -> equalsText(result, node.getText())));
                stage = "feedback counter " + block;
                awaitAny(node -> equalsText("app.msime.android", node.getPackageName())
                    && equalsText(result.length() + " / 500", node.getText()));
                stage = "feedback hide/show " + block;
                shellDiscard("input keyevent 4");
                tap(feedbackField());
                await(key("n"));
                SystemClock.sleep(300);
                android.os.Bundle progress = new android.os.Bundle();
                progress.putString("stream", "Synthetic feedback block " + (block + 1) + "/5 passed\n");
                sendStatus(0, progress);
            }
        } finally {
            // 离开合成反馈草稿后再恢复偏好，避免任何恢复动作被当成反馈提交。
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity --ei msime_open_tab 0");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("Input mode restore failed");
            restartIme();
            if (!originalIme.isEmpty() && !originalIme.equals("null")) shellDiscard("ime set " + originalIme);
        }
    }

    private Predicate<AccessibilityNodeInfo> feedbackField() {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && equalsText("描述", node.getContentDescription()) && node.isEditable();
    }

    // 只输出固定阶段名；反馈页与输入法同包，不能沿用会收集页面文字的默认错误摘要。
    @Override protected String imeTexts() { return "synthetic feedback input state"; }

    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }

    @Override protected AccessibilityNodeInfo findAny(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.findAny(node, match);
    }

    private String shellText(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            for (int count; (count = input.read(buffer)) != -1;) output.write(buffer, 0, count);
            return output.toString(StandardCharsets.UTF_8.name());
        }
    }

    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
}

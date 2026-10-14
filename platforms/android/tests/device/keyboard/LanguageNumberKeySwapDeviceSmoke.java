package app.msime.android.test;

import android.graphics.Rect;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 真实输入法键位交换：中文、英文、数字层及外语九键都使用同一个本机设置。 */
public final class LanguageNumberKeySwapDeviceSmoke extends DeviceSmoke {
    private static final String SETTING = "platform.android.swap_language_number_keys";

    @Override protected String successDescription() {
        return "language/number key positions, English and foreign layouts, restore and persistence";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath())) throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        File local = new File(new File(new File(root, "bootstrap"), "state"), "android-settings.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.exists() ? Files.readAllBytes(local.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (String[] layout : new String[][] {{"quanpin", "quanpin", "twenty_six_key"},
                    {"nine_key", "quanpin", "nine_key"}, {"korean", "korean", "twenty_six_key"},
                    {"japanese_nine_key", "japanese", "nine_key"}, {"zhuyin_nine_key", "zhuyin", "nine_key"}}) {
                stage = "prepare " + layout[0];
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", layout[1]).put("touch_keyboard_layout", layout[2])
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put(layout[0])).put("selected", layout[0]));
                publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
                    .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
                for (boolean swapped : new boolean[] {false, true, false}) {
                    stage = layout[0] + (swapped ? " swapped positions" : " default positions");
                    publish(local, new JSONObject().put("version", 1).put("settings", new JSONObject().put(SETTING, swapped))
                        .toString().getBytes(StandardCharsets.UTF_8));
                    restartIme();
                    openEditor();
                    checkPositions(key("123"), swapped);
                    if (layout[0].equals("quanpin")) {
                        // 英文切换和数字层按键动作仍工作，位置保持交换状态。
                        tap(languageKey());
                        checkPositions(key("123"), swapped);
                        tap(key("123"));
                        checkPositions(described("切换到字母键盘"), swapped);
                    }
                }
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
            if (originalLocal == null) Files.deleteIfExists(local.toPath()); else publish(local, originalLocal);
            restartIme();
        }
    }

    private Predicate<AccessibilityNodeInfo> languageKey() {
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.isClickable()
            && node.getContentDescription() != null && (node.getContentDescription().toString().startsWith("切换到英文")
                || node.getContentDescription().toString().startsWith("切换到中文")
                || node.getContentDescription().toString().startsWith("中英切换暂不可用"));
    }

    private void checkPositions(Predicate<AccessibilityNodeInfo> layerKey, boolean swapped) throws Exception {
        Rect language = new Rect(), layer = new Rect();
        awaitStableBounds(languageKey()).getBoundsInScreen(language);
        awaitStableBounds(layerKey).getBoundsInScreen(layer);
        if (swapped ? language.centerX() >= layer.centerX() : language.centerX() <= layer.centerX())
            throw new AssertionError("Language/number keys did not use selected order");
    }

    private void openEditor() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field("msime-test-plain"));
    }
}

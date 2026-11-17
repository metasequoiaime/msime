package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在真实合成编辑器中验证候选区收紧后窗口仍稳定，并保留真实尺寸截图。 */
final class CompactCandidateToolbarDeviceChecks {
    private final DeviceSmoke t;

    CompactCandidateToolbarDeviceChecks(DeviceSmoke test) { t = test; }

    void run(String mode) throws Exception {
        boolean before = "before".equals(mode);
        File root = t.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = t.sharedPreferencesFile();
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = bytes(preferences), originalLocal = bytes(local);
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
            .put("touch_keyboard_layout", "twenty_six_key").put("touch_spelling_position", "candidate_bar")
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"))
            .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
            .put("cloud_candidates", false).put("learning", false).put("candidate_pronunciation", false)
            .put("candidate_translations", false).put("translation_target_language", "en");
        try {
            if (!before) settings(local);
            if ("settings".equals(mode)) return;
            int[] percents = before ? new int[] {90} : new int[] {50, 70, 90, 100, 150, 0};
            for (int percent : percents) {
                for (int variant = 0; variant < (before ? 1 : 3); variant++) {
                    boolean gloss = variant == 1;
                    int font = variant == 2 ? 32 : 18;
                    t.stage = "compact toolbar " + percent + "% gloss=" + gloss + " font=" + font;
                    JSONObject values = new JSONObject(base.toString()).put("candidate_english_gloss", gloss)
                        .put("candidate_font_size", font).put("candidate_scale_percent", 100);
                    t.publish(preferences, encoded(new JSONObject().put("format_version", 1)
                        .put("revision", ++revision).put("preferences", values)));
                    JSONObject settings = new JSONObject().put("platform.android.clipboard_suggestion", false);
                    if (percent != 0) settings.put("platform.android.toolbar_size_percent", percent);
                    t.publish(local, encoded(new JSONObject().put("version", 1).put("settings", settings)));
                    t.restartIme();
                    t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                    t.tap(t.field("msime-test-plain"));
                    t.selectChineseKeyboard();
                    t.awaitAny(t.describedPrefix("输入方案：").and(AccessibilityNodeInfo::isEnabled));
                    t.awaitStableBounds(t.key("中").and(AccessibilityNodeInfo::isEnabled));
                    Rect idle = stableWindow(null);
                    if (!gloss && font == 18 && (percent == 90 || percent == 50))
                        screenshot((before ? "before" : "after") + "-idle-" + percent);
                    t.stage = "compact composing " + percent + "% gloss=" + gloss + " font=" + font;
                    for (String letter : new String[] {"n", "i", "h", "a", "o"}) t.tap(t.key(letter));
                    t.await(t.describedPrefix("候选 1：你好"));
                    stableWindow(idle);
                    if (!gloss && font == 18 && (percent == 90 || percent == 50))
                        screenshot((before ? "before" : "after") + "-composing-" + percent);
                    t.stage = "compact commit " + percent + "% gloss=" + gloss + " font=" + font;
                    t.tap(t.describedPrefix("候选 1：你好"));
                    t.await(t.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你好", node.getText())));
                    stableWindow(idle);
                    t.tap(t.key("⌫"));
                    stableWindow(idle);
                }
            }
        } finally {
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restore(preferences, original);
            restore(local, originalLocal);
            t.restartIme();
        }
    }

    private void settings(File local) throws Exception {
        t.stage = "compact toolbar settings";
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        AccessibilityNodeInfo ready = t.await(t.key("临时说明（首次运行）").or(t.key("工具栏")));
        if (DeviceSmoke.equalsText("临时说明（首次运行）", ready.getText())) t.tap(t.key("知道了"));
        t.tap(t.key("工具栏"));
        for (int percent : new int[] {50, 70, 90, 100, 150}) {
            t.tap(t.key("工具栏尺寸"));
            if (percent == 50) {
                t.awaitAny(t.key("50%"));
                screenshot("after-settings");
            }
            t.tap(t.key(percent + "%"));
            long deadline = SystemClock.uptimeMillis() + 5000;
            while (SystemClock.uptimeMillis() < deadline
                    && read(local).getJSONObject("settings").optInt("platform.android.toolbar_size_percent") != percent)
                SystemClock.sleep(100);
            if (read(local).getJSONObject("settings").getInt("platform.android.toolbar_size_percent") != percent)
                throw new AssertionError("工具栏尺寸未从设置页保存：" + percent);
        }
        t.tap(t.key("工具栏尺寸"));
        t.tap(t.key("跟随候选栏"));
        long deadline = SystemClock.uptimeMillis() + 5000;
        while (SystemClock.uptimeMillis() < deadline
                && read(local).getJSONObject("settings").has("platform.android.toolbar_size_percent"))
            SystemClock.sleep(100);
        if (read(local).getJSONObject("settings").has("platform.android.toolbar_size_percent"))
            throw new AssertionError("跟随候选栏未移除独立工具栏尺寸");
    }

    private Rect stableWindow(Rect expected) {
        Rect initial = expected;
        long deadline = SystemClock.uptimeMillis() + 600;
        do {
            t.dropStaleNodes();
            Rect actual = null;
            for (AccessibilityWindowInfo window : t.automation.getWindows()) {
                if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
                actual = new Rect();
                window.getBoundsInScreen(actual);
                break;
            }
            if (actual == null || actual.isEmpty()) throw new AssertionError("输入法窗口短暂消失：" + t.stage);
            if (initial == null) initial = actual;
            if (!initial.equals(actual)) throw new AssertionError("输入法窗口变化：" + t.stage
                + " " + initial.toShortString() + " -> " + actual.toShortString());
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        return initial;
    }

    private void screenshot(String name) throws Exception {
        t.automation.waitForIdle(500, 5000);
        File directory = new File(t.getTargetContext().getExternalFilesDir(null), "compact-candidate-toolbar");
        if (!directory.isDirectory() && !directory.mkdirs()) throw new AssertionError("截图目录创建失败");
        Bitmap bitmap = t.automation.takeScreenshot();
        if (bitmap == null) throw new AssertionError("无法截取验收画面");
        try (FileOutputStream output = new FileOutputStream(new File(directory, name + ".png"))) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) throw new AssertionError("截图编码失败");
        } finally { bitmap.recycle(); }
    }

    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else t.publish(file, original);
    }
    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
    private static byte[] bytes(File file) throws Exception {
        return file.isFile() ? Files.readAllBytes(file.toPath()) : null;
    }
    private static byte[] encoded(JSONObject value) { return value.toString().getBytes(StandardCharsets.UTF_8); }
}

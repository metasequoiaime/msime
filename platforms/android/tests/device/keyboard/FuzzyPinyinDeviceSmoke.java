package app.msime.android.test;

import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.util.AtomicFile;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** Android host acceptance for Engine-owned fuzzy-pinyin candidates and disabled rules. */
public final class FuzzyPinyinDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "fuzzy-pinyin candidate generation, disabled-rule retention and deferred update";
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
        } finally {
            shell("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath()); else publish(preferences, original);
        }
    }

    private Predicate<android.view.accessibility.AccessibilityNodeInfo> candidate(String text) {
        return node -> equalsText("app.msime.android", node.getPackageName())
            && node.getContentDescription() != null
            && node.getContentDescription().toString().startsWith("候选 1：" + text);
    }

    private android.view.accessibility.AccessibilityNodeInfo findVisible(
            Predicate<android.view.accessibility.AccessibilityNodeInfo> predicate) {
        for (android.view.accessibility.AccessibilityWindowInfo window : automation.getWindows()) {
            android.view.accessibility.AccessibilityNodeInfo found = find(window.getRoot(), predicate);
            if (found != null) return found;
        }
        return null;
    }

    private void restartIme() throws Exception {
        shell("ime disable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime enable app.msime.android/app.msime.android.MSIMEInputService");
        shell("ime set app.msime.android/app.msime.android.MSIMEInputService");
        SystemClock.sleep(1000);
    }

    private void openEditor() throws Exception {
        shell("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
    }

    private void shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(automation.executeShellCommand(command))) {
            byte[] buffer = new byte[1024];
            while (input.read(buffer) != -1) { }
        }
    }

    private void publish(File file, byte[] contents) throws Exception {
        AtomicFile target = new AtomicFile(file);
        FileOutputStream output = target.startWrite();
        try { output.write(contents); target.finishWrite(output); }
        catch (Exception error) { target.failWrite(output); throw error; }
    }
}

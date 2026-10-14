package app.msime.android.test;

import android.os.Build;
import android.os.SystemClock;
import android.os.ParcelFileDescriptor;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 日语使用和输入框重建后，中英键仍返回进入外语前的中文入口。 */
public final class ChineseLanguageReturnDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "Chinese nine/fourteen-key return after English, Japanese typing and editor refocus";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped test sandbox");
        File preferences = new File(directory, "preferences.json");
        File resources = new File(options.getString("resources")).getCanonicalFile();
        if (!resources.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Resources escaped test sandbox");
        java.util.List<File> stagedResources = new java.util.ArrayList<>();
        File local = new File(new File(root, "bootstrap/state"), "android-settings.json");
        byte[] original = preferences.isFile() ? Files.readAllBytes(preferences.toPath()) : null;
        byte[] originalLocal = local.isFile() ? Files.readAllBytes(local.toPath()) : null;
        String originalIme = shellText("settings get secure default_input_method").trim();
        android.content.SharedPreferences modes = getTargetContext()
            .getSharedPreferences("android-input-modes", 0);
        java.util.Map<String, ?> originalModes = modes.getAll();
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        try {
            stage = "prepare bundled Japanese resources";
            // 专用 AVD 可能保留瘦包的资源目录；只临时补齐当前完整 APK 随包的日文文件。
            for (String name : new String[] {"msime-japanese.dat", "msime-mozc_dictionary_oss_README.txt", "msime-mozc_LICENSE.txt"}) {
                File target = new File(resources, name);
                if (target.exists()) continue;
                stagedResources.add(target);
                try (var input = getTargetContext().getAssets().open("dictionary/" + name)) {
                    Files.copy(input, target.toPath());
                }
            }
            JSONObject settings = originalLocal == null ? new JSONObject().put("version", 1)
                .put("settings", new JSONObject()) : read(local);
            settings.getJSONObject("settings").put("platform.android.language_key_cycle", true);
            publish(local, bytes(settings));
            for (String[] layout : new String[][] {{"nine_key", "全拼 9 键"}, {"fourteen_key", "全拼 14 键"}}) {
                stage = "prepare " + layout[0];
                JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                    .put("touch_keyboard_layout", layout[0])
                    .put("ime_mode_scope", "global").put("default_ime_mode", "chinese")
                    .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray()
                        .put("quanpin").put("nine_key").put("fourteen_key")
                        .put("japanese_nine_key").put("japanese")).put("selected", layout[0]));
                publish(preferences, bytes(new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", values)));
                modes.edit().clear().commit();
                restartIme();
                shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
                tap(field("msime-test-plain"));
                await(scheme(layout[1]));
                for (int refocus = 0; refocus < 3; refocus++) {
                    stage = layout[0] + " English transition " + refocus;
                    tap(language("切换到英文输入"));
                    await(key("英"));
                    stage = layout[0] + " Japanese transition " + refocus;
                    AccessibilityNodeInfo nextLanguage = await(node -> equalsText("app.msime.android", node.getPackageName())
                        && equalsText("英", node.getText()) && node.getContentDescription() != null
                        && node.getContentDescription().toString().contains("切换到"));
                    if (!nextLanguage.getContentDescription().toString().contains("日语 9 键")) {
                        boolean bundled = new File(options.getString("resources"), "msime-japanese.dat").isFile();
                        boolean cycle = read(local).getJSONObject("settings").getBoolean("platform.android.language_key_cycle");
                        String target = nextLanguage.getContentDescription().toString().contains("日语 26 键") ? "Japanese26" : "other";
                        throw new AssertionError("Japanese9 target unavailable: " + target + ", bundled=" + bundled + ", cycle=" + cycle);
                    }
                    tap(language("切换到日语 9 键"));
                    stage = layout[0] + " Japanese layout ready " + refocus;
                    await(scheme("日语 9 键"));
                    stage = layout[0] + " Japanese kana input " + refocus;
                    tap(key("あ"));
                    stage = layout[0] + " Japanese commit " + refocus;
                    tap(key("確定"));
                    // 不重启服务：真实的输入框切换会再次读取出厂副本，并异步恢复日语实时偏好。
                    stage = layout[0] + " Japanese editor refocus " + refocus;
                    tap(field(refocus % 2 == 0 ? "msime-test-cursor-at-end" : "msime-test-plain"));
                    await(scheme("日语 9 键"));
                    SystemClock.sleep(1000);
                    stage = layout[0] + " Chinese return target " + refocus;
                    AccessibilityNodeInfo chineseTarget = await(node -> equalsText("app.msime.android", node.getPackageName())
                        && equalsText("中", node.getText()) && node.isClickable()
                        && node.getContentDescription() != null && node.getContentDescription().toString().contains("切换到"));
                    if (!chineseTarget.getContentDescription().toString().contains(layout[1]))
                        throw new AssertionError(chineseTarget.getContentDescription().toString().contains("全拼 26 键")
                            ? "Chinese return changed to Quanpin26 after editor refocus" : "Chinese return lost the original entry");
                    tap(language("切换到" + layout[1]));
                    stage = layout[0] + " returned layout " + refocus;
                    await(scheme(layout[1]));
                    if (layout[0].equals("nine_key")) await(key("ABC"));
                    else await(described("按键 Q W"));
                    JSONObject saved = read(preferences).getJSONObject("preferences");
                    if (!layout[0].equals(saved.getJSONObject("touch_keyboard_schemes").getString("selected"))
                            || !layout[0].equals(saved.getString("touch_keyboard_layout")))
                        throw new AssertionError("Returned Chinese selection was not saved");
                }
                revision = read(preferences).getLong("revision");
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restore(preferences, original);
            restore(local, originalLocal);
            for (File file : stagedResources) Files.deleteIfExists(file.toPath());
            android.content.SharedPreferences.Editor editor = modes.edit().clear();
            for (var entry : originalModes.entrySet()) editor.putBoolean(entry.getKey(), (Boolean) entry.getValue());
            if (!editor.commit()) throw new AssertionError("Input modes restore failed");
            restartIme();
            if (!originalIme.isEmpty() && !originalIme.equals("null")) shellDiscard("ime set " + originalIme);
        }
    }

    private Predicate<AccessibilityNodeInfo> scheme(String title) {
        return describedPrefix("输入方案：" + title);
    }

    private Predicate<AccessibilityNodeInfo> language(String description) {
        return node -> equalsText("app.msime.android", node.getPackageName()) && node.isClickable()
            && (equalsText(description, node.getContentDescription())
                || equalsText("按键 " + description, node.getContentDescription()));
    }

    // API 28 的节点缓存不会自动失效，必须刷新后再判断重建后的键盘。
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

    private static byte[] bytes(JSONObject value) { return value.toString().getBytes(StandardCharsets.UTF_8); }

    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else publish(file, original);
    }
}

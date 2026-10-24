package app.msime.android.test;

import android.content.res.Configuration;
import android.graphics.Rect;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.io.ByteArrayOutputStream;
import java.util.function.Predicate;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在专用模拟器里切换窗口尺寸，经真实键盘和原生设置验证竖屏分离；结束后恢复显示配置与偏好。 */
final class PortraitSplitDeviceChecks {
    private static final String PREFERENCE = "touch_keyboard_split_portrait";
    private static final String SPLIT = "platform.android.split_keyboard";
    private final DeviceSmoke test;
    private File preferences;
    private JSONObject snapshot;
    private long revision;

    private PortraitSplitDeviceChecks(DeviceSmoke test) { this.test = test; }

    static void run(DeviceSmoke test) throws Exception { new PortraitSplitDeviceChecks(test).run(); }

    private void run() throws Exception {
        File root = test.getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = json(new File(root, "runtime-options.json"));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.toPath())) throw new AssertionError("Invalid synthetic sandbox");
        preferences = new File(directory, "preferences.json");
        File local = new File(new File(root, "bootstrap/state"), "android-settings.json");
        byte[] saved = read(preferences), savedLocal = read(local);
        String size = override("wm size", "size", "[0-9]+x[0-9]+");
        String density = override("wm density", "density", "[0-9]+");
        String rotation = shell("settings get system user_rotation").trim();
        String automatic = shell("settings get system accelerometer_rotation").trim();
        revision = saved == null ? 0 : new JSONObject(new String(saved, StandardCharsets.UTF_8)).getLong("revision");
        snapshot = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
            .put("learning", false).put("cloud_candidates", false).put("touch_spelling_position", "editor")
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        JSONObject localDocument = savedLocal == null ? new JSONObject().put("version", 1).put("settings", new JSONObject())
            : new JSONObject(new String(savedLocal, StandardCharsets.UTF_8));
        // 本地设置文档沿用宿主现有格式，不把测试文字写入本地设置。
        JSONObject localValues = localDocument.getJSONObject("settings");
        localValues.put(SPLIT, true).put("platform.android.floating_keyboard", false)
            .put("platform.android.one_handed", "off");
        try {
            test.stage = "large portrait window preparation";
            test.publish(local, localDocument.toString().getBytes(StandardCharsets.UTF_8));
            setPortrait(false);
            test.shellDiscard("settings put system accelerometer_rotation 0");
            test.shellDiscard("settings put system user_rotation 0");
            test.shellDiscard("wm density 240");
            resize("1600x2000", true, false);
            test.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            test.restartIme();
            openEditor();
            expectSplit(false);
            if (test.await(test.key("中").or(test.key("英"))).getText().toString().equals("英"))
                test.tap(test.key("英"));
            test.tap(test.key("n"));
            test.tap(test.key("i"));
            expectText("ni");

            test.stage = "portrait setting applies without finishing composition";
            setPortrait(true);
            expectSplit(true);
            expectText("ni");
            setPortrait(false);
            expectSplit(false);
            expectText("ni");

            test.stage = "preserved composition can still select a Chinese candidate";
            test.tap(test.key("空格"));
            expectText("你");

            test.stage = "native portrait split control saves a boolean";
            test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
            AccessibilityNodeInfo row = test.awaitAny(portraitControl());
            if (!row.isEnabled() || !row.isCheckable() || row.isChecked())
                throw new AssertionError("Portrait split control has the wrong initial state");
            test.tap(portraitControl());
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (!json(preferences).getJSONObject("preferences")
                    .optBoolean(PREFERENCE, false) && SystemClock.uptimeMillis() < deadline) SystemClock.sleep(100);
            JSONObject written = json(preferences);
            if (!Boolean.TRUE.equals(written.getJSONObject("preferences").get(PREFERENCE)))
                throw new AssertionError("Native control did not persist the portrait setting");
            revision = written.getLong("revision");
            snapshot = written.getJSONObject("preferences");
            openEditor();
            expectSplit(true);

            test.stage = "Chinese numeric grid remains a full panel";
            test.tap(test.key("123"));
            expectGap("5", "6", "4", "6", false);
            test.stage = "extended symbol row splits in portrait";
            test.tap(test.key("#+="));
            expectGap("#", "%", "[", "=", true);
            test.tap(test.described("切换到字母键盘"));
            expectSplit(true);

            test.stage = "narrow cover window returns to a full keyboard";
            resize("700x1500", false, false);
            focusEditor();
            expectSplit(false);
            test.stage = "expanded portrait window restores split keyboard";
            resize("1600x2000", true, false);
            focusEditor();
            expectSplit(true);
            test.stage = "landscape still splits with portrait setting disabled";
            setPortrait(false);
            resize("2000x1600", true, true);
            focusEditor();
            expectSplit(true);
        } finally {
            restore(preferences, saved);
            restore(local, savedLocal);
            test.shellDiscard("wm size " + size);
            test.shellDiscard("wm density " + density);
            restoreSystem("user_rotation", rotation);
            restoreSystem("accelerometer_rotation", automatic);
            test.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
        }
    }

    private void openEditor() throws Exception {
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EnglishCorrectionEditorActivity");
        focusEditor();
    }

    /** 尺寸变化会重建合成编辑器；重新聚焦，让系统再次展示输入法后再测量。 */
    private void focusEditor() throws Exception {
        test.tap(test.field("correction-plain"));
        test.await(test.key("q"));
    }

    private void setPortrait(boolean enabled) throws Exception {
        snapshot.put(PREFERENCE, enabled);
        test.publish(preferences, new JSONObject().put("format_version", 1).put("revision", ++revision)
            .put("preferences", snapshot).toString().getBytes(StandardCharsets.UTF_8));
    }

    private void resize(String size, boolean large, boolean landscape) throws Exception {
        test.shellDiscard("wm size " + size);
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            Configuration value = test.getTargetContext().getResources().getConfiguration();
            if ((value.smallestScreenWidthDp >= 600) == large
                    && (value.orientation == Configuration.ORIENTATION_LANDSCAPE) == landscape) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic display configuration did not settle");
    }

    private void expectSplit(boolean split) { expectGap("t", "y", "q", "p", split); }

    private void expectGap(String left, String right, String first, String last, boolean split) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            test.dropStaleNodes();
            Rect a = bounds(test.await(test.key(left))), b = bounds(test.await(test.key(right)));
            Rect start = bounds(test.await(test.key(first))), end = bounds(test.await(test.key(last)));
            int width = end.right - start.left;
            if (width > 0 && ((b.left - a.right) > width * .15f) == split) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError(split ? "Large portrait keyboard did not split" : "Full keyboard retained a split gap");
    }

    private void expectText(String value) {
        test.await(test.field("correction-plain").and(node -> DeviceSmoke.equalsText(value, node.getText())));
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect(); node.getBoundsInScreen(result); return result;
    }

    private String override(String command, String name, String pattern) throws Exception {
        var match = Pattern.compile("Override " + name + ": (" + pattern + ")").matcher(shell(command));
        return match.find() ? match.group(1) : "reset";
    }

    private String shell(String command) throws Exception {
        try (var input = new ParcelFileDescriptor.AutoCloseInputStream(test.automation.executeShellCommand(command))) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            int length;
            while ((length = input.read(buffer)) != -1) output.write(buffer, 0, length);
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }

    private void restoreSystem(String key, String value) throws Exception {
        test.shellDiscard(value.matches("[0-9]+") ? "settings put system " + key + " " + value
            : "settings delete system " + key);
    }

    private Predicate<AccessibilityNodeInfo> portraitControl() {
        return node -> node.isCheckable() && test.findAny(node,
            child -> DeviceSmoke.equalsText("竖屏也分离", child.getText())) != null;
    }

    private static JSONObject json(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private static byte[] read(File file) throws Exception { return file.exists() ? Files.readAllBytes(file.toPath()) : null; }
    private void restore(File file, byte[] saved) throws Exception {
        if (saved == null) Files.deleteIfExists(file.toPath()); else test.publish(file, saved);
    }
}

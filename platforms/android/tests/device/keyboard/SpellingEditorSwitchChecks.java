package app.msime.android.test;

import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用真实设置和合成编辑器验证开关、持久化、关闭已开的面板与空行释放。 */
final class SpellingEditorSwitchChecks {
    private static final String KEY = "touch_spelling_editor";

    static void run(DeviceSmoke s) throws Exception {
        File preferences = s.sharedPreferencesFile();
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0
            : new JSONObject(new String(original, StandardCharsets.UTF_8)).getLong("revision");
        File runtime = new File(s.getTargetContext().getFilesDir(), "runtime-options.json");
        JSONObject options = new JSONObject(new String(Files.readAllBytes(runtime.toPath()), StandardCharsets.UTF_8));
        JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
            .put(KEY, true).put("touch_spelling_position", "editor")
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("default_ime_mode", "chinese").put("ime_mode_scope", "global")
            .put("learning", false).put("cloud_candidates", false)
            .put("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
        try {
            publish(s, preferences, base, ++revision);
            s.restartIme();
            editor(s);
            s.stage = "spelling editor enabled entry and baseline";
            s.tap(s.key("n"));
            s.tap(s.key("i"));
            s.await(s.described("编辑拼写"));
            Rect enabled = keyboard(s);
            screenshot(s, "enabled");
            s.tap(s.described("编辑拼写"));
            s.await(s.described("拼写编辑面板"));

            s.stage = "live disable closes the spelling editor and frees its row";
            publish(s, preferences, base.put(KEY, false), ++revision);
            long deadline = SystemClock.uptimeMillis() + 10000;
            while (visible(s, "编辑拼写") || visible(s, "拼写编辑面板")
                    || keyboard(s).height() >= enabled.height() - 8 * density(s)) {
                if (SystemClock.uptimeMillis() > deadline)
                    throw new AssertionError("关闭开关没有撤销编辑面板并释放笔图标空行");
                SystemClock.sleep(80);
            }

            s.stage = "native setting persists on and off";
            setting(s, false);
            s.tap(node -> toggle(s, node));
            awaitSaved(preferences, true);
            setting(s, true);
            s.tap(node -> toggle(s, node));
            awaitSaved(preferences, false);
            setting(s, false);
            screenshot(s, "setting");

            s.stage = "disabled editor keeps a compact stable keyboard while typing";
            editor(s);
            Rect compact = keyboard(s);
            if (compact.height() >= enabled.height() - 8 * density(s))
                throw new AssertionError("重新打开键盘又为禁用入口预留了空行");
            AtomicBoolean monitoring = new AtomicBoolean(true);
            AtomicReference<AssertionError> failure = new AtomicReference<>();
            Thread monitor = new Thread(() -> {
                while (monitoring.get()) {
                    try {
                        Rect actual = keyboard(s);
                        if (!compact.equals(actual)) throw new AssertionError(
                            "禁用拼写编辑时键盘跳动：" + compact + " -> " + actual);
                    } catch (AssertionError changed) { failure.compareAndSet(null, changed); }
                    SystemClock.sleep(25);
                }
            }, "synthetic-spelling-window-check");
            monitor.start();
            try {
                s.tap(s.key("n"));
                s.tap(s.key("i"));
                s.await(s.describedPrefix("候选 1："));
                if (visible(s, "编辑拼写") || visible(s, "拼写编辑面板"))
                    throw new AssertionError("禁用状态仍出现拼写编辑入口或面板");
                screenshot(s, "disabled");
                s.tap(s.key("空格"));
                s.await(s.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("你", node.getText())));
                SystemClock.sleep(500);
            } finally {
                monitoring.set(false);
                monitor.join();
            }
            if (failure.get() != null) throw failure.get();
            s.stage = "disabled editor selection does not open the magnifier";
            s.tap(s.key("n"));
            s.tap(s.key("i"));
            Bundle selection = new Bundle();
            selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 2);
            selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 2);
            if (!s.await(s.field("msime-test-plain")).performAction(
                    AccessibilityNodeInfo.ACTION_SET_SELECTION, selection))
                throw new AssertionError("合成编辑器拒绝组字内选择");
            SystemClock.sleep(500);
            if (visible(s, "拼写编辑面板")) throw new AssertionError("禁用状态下输入框选择仍打开编辑条");
        } finally {
            s.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else s.publish(preferences, original);
        }
    }

    private static void publish(DeviceSmoke s, File file, JSONObject preferences, long revision) throws Exception {
        s.publish(file, new JSONObject().put("format_version", 1).put("revision", revision)
            .put("preferences", preferences).toString().getBytes(StandardCharsets.UTF_8));
    }

    private static void editor(DeviceSmoke s) throws Exception {
        s.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity");
        s.tap(s.field("msime-test-plain"));
        s.selectChineseKeyboard();
        SystemClock.sleep(500);
    }

    private static void setting(DeviceSmoke s, boolean checked) throws Exception {
        Bundle args = new Bundle();
        args.putString("row", "拼写编辑");
        Intent intent = new Intent().setClassName("app.msime.android", "app.msime.android.home.HomeActivity")
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP)
            .putExtra("msime_open_page", "KEYBOARD_OPTIONS").putExtra("msime_page_args", args);
        s.getTargetContext().startActivity(intent);
        s.await(node -> toggle(s, node) && node.isChecked() == checked);
    }

    private static boolean toggle(DeviceSmoke s, AccessibilityNodeInfo node) {
        return node.isCheckable() && DeviceSmoke.equalsText("app.msime.android", node.getPackageName())
            && s.find(node, child -> DeviceSmoke.equalsText("拼写编辑", child.getText())) != null;
    }

    private static void awaitSaved(File file, boolean enabled) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 10000;
        do {
            JSONObject value = new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8)).getJSONObject("preferences");
            if (value.optBoolean(KEY, true) == enabled) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("原生开关没有持久化选择 " + enabled);
    }

    private static boolean visible(DeviceSmoke s, String label) {
        s.dropStaleNodes();
        for (var window : s.automation.getWindows())
            if (s.find(window.getRoot(), s.described(label)) != null) return true;
        return false;
    }

    private static Rect keyboard(DeviceSmoke s) {
        for (var window : s.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect bounds = new Rect();
            window.getBoundsInScreen(bounds);
            return bounds;
        }
        throw new AssertionError("输入法窗口消失");
    }

    private static float density(DeviceSmoke s) { return s.getTargetContext().getResources().getDisplayMetrics().density; }

    private static void screenshot(DeviceSmoke s, String name) throws Exception {
        Bitmap bitmap = s.automation.takeScreenshot();
        if (bitmap == null) throw new AssertionError("合成界面截图不可用");
        try (FileOutputStream output = new FileOutputStream(new File(
                s.getTargetContext().getExternalFilesDir(null), "spelling-editor-" + name + ".png"))) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) throw new AssertionError("截图写入失败");
        } finally { bitmap.recycle(); }
    }
}

package app.msime.android.test;

import android.graphics.Rect;
import android.os.Build;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** 在真实编辑器中横划溢出的工具栏，确认没有误输入且两端仍可操作。 */
public final class ToolbarScrollDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return Build.VERSION.SDK_INT < 30
            ? "real toolbar swipe, fixed edges, calculator and dismiss; panel accessibility unavailable below API30"
            : "real toolbar swipe, fixed edges, calculator, last tool, more, return and dismiss";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir().getCanonicalFile();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.toPath())) throw new AssertionError("Preferences escaped sandbox");
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = bytes(preferences), originalLocal = bytes(local);
        long revision = original == null ? 0 : read(preferences).getLong("revision");
        JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
            .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
            .put("touch_toolbar", new JSONObject().put("emoji", true).put("clipboard", true).put("skin", true))
            .put("touch_keyboard_schemes", new JSONObject().put("enabled", new JSONArray().put("quanpin"))
                .put("selected", "quanpin"));
        JSONObject settings = new JSONObject().put("version", 1).put("settings", new JSONObject()
            .put("platform.android.toolbar_text_edit", true).put("platform.android.toolbar_floating", true)
            .put("platform.android.clipboard_suggestion", false));
        try {
            stage = "toolbar synthetic preferences";
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            await(key("n"));
            Bundle text = new Bundle();
            text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, "12*12");
            if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                throw new AssertionError("Synthetic text setup failed");
            automation.waitForIdle(500, 5000);
            SystemClock.sleep(1000);
            tap(key("123"));
            stage = "calculator and fixed edges before swipe";
            await(describedPrefix("计算结果 144"));
            Rect more = bounds(awaitStableBounds(described("更多")));
            Rect dismiss = bounds(awaitStableBounds(described("收起键盘")));
            Rect viewport = bounds(await(described("可滑动键盘工具")));
            Rect before = bounds(await(described("表情")));
            stage = "physical toolbar swipe";
            swipe(viewport, true);
            Rect after = bounds(await(described("表情")));
            if (before.equals(after)) throw new AssertionError("Toolbar swipe did not move tools");
            if (!more.equals(bounds(await(described("更多"))))
                    || !dismiss.equals(bounds(await(described("收起键盘")))))
                throw new AssertionError("Toolbar swipe moved an edge button");
            await(field("msime-test-plain").and(node -> equalsText("12*12", node.getText())));
            // API 28 的系统无障碍树不暴露工具面板，沿用核心套件的覆盖边界；仍验证横划和真实收起点击。
            if (Build.VERSION.SDK_INT >= 30) {
                stage = "last tool opens correct panel after swipe";
                physicalTap("文本编辑");
                await(described("文本编辑面板"));
                tap(described("返回键盘"));
                stage = "fixed brand opens menu at scroll end";
                physicalTap("更多");
                await(toolPanel());
                tap(described("返回键盘"));
            }
            stage = "physical toolbar reverse swipe";
            swipe(bounds(await(described("可滑动键盘工具"))), false);
            await(describedPrefix("计算结果 144"));
            stage = "fixed dismiss closes keyboard";
            physicalTap("收起键盘");
            SystemClock.sleep(700);
            dropStaleNodes();
            for (var window : automation.getWindows())
                if (find(window.getRoot(), described("键盘快捷栏")) != null)
                    throw new AssertionError("Dismiss left keyboard visible");
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restore(preferences, original);
            restore(local, originalLocal);
            restartIme();
        }
    }

    private void physicalTap(String description) throws Exception {
        Rect target = bounds(awaitStableBounds(described(description)));
        shellDiscard("input tap " + target.centerX() + " " + target.centerY());
    }

    private void swipe(Rect viewport, boolean forward) throws Exception {
        int inset = Math.round(12 * getTargetContext().getResources().getDisplayMetrics().density);
        int start = forward ? viewport.right - inset : viewport.left + inset;
        int end = forward ? viewport.left + inset : viewport.right - inset;
        shellDiscard("input swipe " + start + " " + viewport.centerY() + " " + end + " " + viewport.centerY() + " 400");
        SystemClock.sleep(700);
        dropStaleNodes();
    }
    @Override protected AccessibilityNodeInfo find(AccessibilityNodeInfo node, Predicate<AccessibilityNodeInfo> match) {
        if (node != null && Build.VERSION.SDK_INT < 34 && !node.refresh()) return null;
        return super.find(node, match);
    }
    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect(); node.getBoundsInScreen(result); return result;
    }
    private static JSONObject read(File file) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }
    private static byte[] bytes(File file) throws Exception {
        return file.isFile() ? Files.readAllBytes(file.toPath()) : null;
    }
    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else publish(file, original);
    }
}

package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 只使用合成拼写，在安装后的原生键盘中验收组字内点击、放大、插删和偏移拖动。 */
final class CompositionMagnifierChecks {
    private CompositionMagnifierChecks() { }

    static void run(DeviceSmoke s) throws Exception {
        File root = s.getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original,
            StandardCharsets.UTF_8)).getLong("revision");
        try {
            for (boolean nineKey : new boolean[] {false, true}) {
                String id = nineKey ? "nine_key" : "quanpin";
                JSONObject base = new JSONObject(options.getJSONObject("preferences").toString())
                    .put("scheme", "quanpin").put("traditional_chinese_output", false)
                    .put("touch_keyboard_layout", nineKey ? "nine_key" : "twenty_six_key")
                    .put("learning", false).put("cloud_candidates", false)
                    .put("mixed_input", new JSONObject().put("english", false).put("minimum_prefix", 2)
                        .put("emoji", false).put("kaomoji", false))
                    .put("touch_keyboard_schemes", new JSONObject()
                        .put("enabled", new JSONArray().put(id)).put("selected", id));
                s.publish(preferences, new JSONObject().put("format_version", 1)
                    .put("revision", ++revision).put("preferences", base).toString()
                    .getBytes(StandardCharsets.UTF_8));
                s.restartIme();
                s.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity");
                s.tap(s.field("msime-test-plain"));
                if (DeviceSmoke.equalsText("英", s.await(s.key("中").or(s.key("英"))).getText())) s.tap(s.key("英"));
                s.stage = "magnifier ordinary typing does not open editing";
                if (nineKey) {
                    for (String key : new String[] {"MNO", "GHI", "GHI", "ABC", "MNO"}) s.tap(s.key(key));
                } else {
                    for (String key : new String[] {"n", "i", "h", "a", "o"}) s.tap(s.key(key));
                }
                s.await(s.describedPrefix("候选 1："));
                SystemClock.sleep(300);
                if (panelVisible(s))
                    throw new AssertionError("Own selection echo opened the magnifier");
                s.stage = "magnifier opens from an in-composition editor selection";
                select(s.await(s.field("msime-test-plain")), 1);
                s.await(s.described("拼写编辑面板"));
                s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 1));
                if (!nineKey) {
                    s.stage = "magnifier inserts and deletes in the middle without losing the suffix";
                    s.tap(s.key("h"));
                    s.await(s.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("nhihao", node.getText())));
                    s.tap(s.key("⌫"));
                    s.await(s.field("msime-test-plain").and(node -> DeviceSmoke.equalsText("nihao", node.getText())));
                } else {
                    s.stage = "magnifier nine-key middle edit preserves remaining digits";
                    s.tap(s.key("DEF"));
                    s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 2));
                    s.tap(s.key("⌫"));
                    s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 1));
                }
                s.stage = "magnifier offset handle can reach both ends";
                select(s.await(s.described("放大拼写")), 0);
                s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 0));
                Rect text = bounds(s.await(s.described("放大拼写")));
                Rect handle = bounds(s.await(s.described("拼写光标拖动手柄")));
                float density = s.getTargetContext().getResources().getDisplayMetrics().density;
                if (handle.width() < 55 * density || handle.height() < 55 * density)
                    throw new AssertionError("Handle is too small for a thumb");
                if (handle.left <= text.left || handle.top - text.bottom < 28 * density)
                    throw new AssertionError("Handle is too close to the magnified spelling");
                drag(s, handle, text.right - 4, 1400);
                s.await(s.described("放大拼写").and(node -> node.getText() != null
                    && node.getTextSelectionStart() == node.getText().length()));
                drag(s, bounds(s.await(s.described("拼写光标拖动手柄"))), text.left + 4, 1400);
                s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 0));
                s.stage = "magnifier completion preserves candidates and the keyboard entry reopens it";
                s.tap(s.described("完成拼写编辑"));
                s.await(s.describedPrefix("候选 1："));
                if (panelVisible(s))
                    throw new AssertionError("Done did not close editing");
                s.tap(s.described("编辑拼写"));
                s.await(s.described("拼写编辑面板"));
                s.tap(s.describedPrefix("候选 1："));
                s.stage = "magnifier closes after candidate commit";
                SystemClock.sleep(300);
                if (panelVisible(s))
                    throw new AssertionError("Candidate commit left editing open");
                if (!nineKey) longSpelling(s);
                s.stage = "magnifier closes on another input field";
                s.tap(s.field("msime-test-number"));
                s.await(s.field("msime-test-number").and(AccessibilityNodeInfo::isFocused));
                if (panelVisible(s))
                    throw new AssertionError("New editor inherited spelling editor");
            }
        } finally {
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else s.publish(preferences, original);
        }
    }

    private static boolean panelVisible(DeviceSmoke s) {
        s.dropStaleNodes();
        for (var window : s.automation.getWindows()) {
            if (s.find(window.getRoot(), s.described("拼写编辑面板")) != null) return true;
        }
        return false;
    }

    private static void longSpelling(DeviceSmoke s) throws Exception {
        s.stage = "magnifier long spelling follows the caret to both screen edges";
        String prefix = s.await(s.field("msime-test-plain")).getText().toString();
        StringBuilder spelling = new StringBuilder();
        // 逐组等待真实触屏输入，避免 shell 连发注入未完成就开始验收拖动。
        for (int syllable = 0; syllable < 8; syllable++) {
            for (String key : new String[] {"n", "i", "h", "a", "o"}) s.tap(s.key(key));
            spelling.append("nihao");
            String expected = prefix + spelling;
            s.await(s.field("msime-test-plain").and(node -> node.refresh()
                && DeviceSmoke.equalsText(expected, node.getText())));
        }
        s.tap(s.described("编辑拼写"));
        AccessibilityNodeInfo text = s.await(s.described("放大拼写").and(node ->
            node.getText() != null && node.getText().length() >= 35));
        select(text, 0);
        s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 0));
        Rect area = bounds(text);
        drag(s, bounds(s.await(s.described("拼写光标拖动手柄"))), area.right - 4, 5000);
        s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == node.getText().length()));
        drag(s, bounds(s.await(s.described("拼写光标拖动手柄"))), area.left + 4, 5000);
        s.await(s.described("放大拼写").and(node -> node.getTextSelectionStart() == 0));
    }

    private static void select(AccessibilityNodeInfo node, int position) {
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, position);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, position);
        if (!node.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("Synthetic caret selection rejected");
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds;
    }

    private static void drag(DeviceSmoke s, Rect handle, int target, long hold) {
        long start = SystemClock.uptimeMillis();
        inject(s, start, MotionEvent.ACTION_DOWN, handle.centerX(), handle.centerY());
        for (int step = 1; step <= 12; step++) {
            SystemClock.sleep(16);
            inject(s, start, MotionEvent.ACTION_MOVE,
                handle.centerX() + (target - handle.centerX()) * step / 12f, handle.centerY());
        }
        SystemClock.sleep(hold);
        inject(s, start, MotionEvent.ACTION_UP, target, handle.centerY());
    }

    private static void inject(DeviceSmoke s, long start, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(start, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!s.automation.injectInputEvent(event, true)) throw new AssertionError("Synthetic drag rejected");
        } finally { event.recycle(); }
    }
}

package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONObject;

/** 真实键盘上跨行、反向、回缩、取消和边缘滚动拖选，只使用合成词片。 */
public final class ClipboardSegmentDragDeviceSmoke extends DeviceSmoke {
    private final Map<File, byte[]> backups = new LinkedHashMap<>();
    private long downTime;
    private static String word(int i) { return "Synthetic" + (char) ('A' + i); }

    @Override protected String successDescription() {
        return "segment taps, multiline range selection, reversal, cancellation and edge scrolling";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = read(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json");
        File history = new File(root, "MSIME/clipboard_history.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        for (File f : new File[] {preferences, history, local})
            backups.put(f, f.isFile() ? Files.readAllBytes(f.toPath()) : null);
        try {
            JSONObject prefs = read(preferences);
            prefs.put("revision", prefs.getLong("revision") + 1);
            prefs.getJSONObject("preferences").put("clipboard_history", true)
                .put("scheme", "quanpin").put("last_chinese_scheme", "quanpin")
                .put("touch_keyboard_layout", "twenty_six_key")
                .put("touch_toolbar", new JSONObject().put("clipboard", true));
            write(preferences, prefs.toString());
            write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)).toString());
            StringBuilder source = new StringBuilder();
            for (int i = 0; i < 26; i++) { if (i > 0) source.append(' '); source.append(word(i)); }
            write(history, new JSONArray().put(new JSONObject().put("text", source.toString())
                .put("timestampMs", 1700000000000L).put("pinned", true)).toString());
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
            tap(field("msime-test-plain"));
            await(key("n"));
            tap(described("剪贴板"));
            AccessibilityNodeInfo card = await(node -> node.isLongClickable() && node.getText() != null
                && node.getText().toString().startsWith(word(0)));
            if (!card.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK)) throw new AssertionError("Long press refused");
            tap(node -> equalsText("分词", node.getText()) && node.getParent() != null
            && equalsText("剪贴板记录操作", node.getParent().getContentDescription()));
            stage = "single tap keeps original behavior";
            tap(described("词片 " + word(0)));
            selected(0, true);
            tap(described("词片 " + word(0)));
            selected(0, false);
            stage = "drag spans multiple rows";
            Rect first = rect(0), fourth = rect(5);
            if (fourth.centerY() <= first.centerY()) throw new AssertionError("Fixture did not wrap");
            drag(first, fourth, false);
            for (int i = 0; i <= 5; i++) selected(i, true);
            selected(6, false);
            stage = "reverse drag deselects selected range";
            drag(rect(5), rect(0), false);
            for (int i = 0; i <= 5; i++) selected(i, false);
            stage = "cancel restores original selection";
            drag(rect(0), rect(5), true);
            for (int i = 0; i <= 5; i++) selected(i, false);
            stage = "range shrink restores words outside final range";
            Rect start = rect(0), far = rect(5), near = rect(1);
            downTime = SystemClock.uptimeMillis();
            event(MotionEvent.ACTION_DOWN, start.centerX(), start.centerY());
            move(start, far);
            move(far, near);
            event(MotionEvent.ACTION_UP, near.centerX(), near.centerY());
            selected(0, true); selected(1, true); selected(2, false); selected(3, false);
            stage = "edge hold scrolls to hidden final word";
            tap(key("全选")); tap(key("全不选"));
            start = rect(0);
            Rect bottom = new Rect();
            await(described("分词滚动区域")).getBoundsInScreen(bottom);
            int edgeY = bottom.bottom - 8;
            downTime = SystemClock.uptimeMillis();
            event(MotionEvent.ACTION_DOWN, start.centerX(), start.centerY());
            // 最后一行可能有多个词片，停在右下角才能覆盖末词。
            Rect edge = new Rect(bottom.right - 8, edgeY, bottom.right - 7, edgeY + 1);
            move(start, edge);
            SystemClock.sleep(8000);
            event(MotionEvent.ACTION_UP, edge.centerX(), edge.centerY());
            selected(25, true);
            tap(key("插入"));
            await(field("msime-test-plain").and(node -> equalsText(source.toString(), node.getText())));
        } finally {
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else publish(entry.getKey(), entry.getValue());
            }
            restartIme();
        }
    }

    private Rect rect(int i) {
        Rect r = new Rect();
        await(described("词片 " + word(i)).and(AccessibilityNodeInfo::isVisibleToUser)).getBoundsInScreen(r);
        return r;
    }
    private void selected(int i, boolean value) {
        await(described("词片 " + word(i)).and(node -> node.isSelected() == value));
    }
    private void drag(Rect from, Rect to, boolean cancel) {
        downTime = SystemClock.uptimeMillis();
        event(MotionEvent.ACTION_DOWN, from.centerX(), from.centerY());
        move(from, to);
        event(cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP, to.centerX(), to.centerY());
    }
    private void move(Rect from, Rect to) {
        for (int i = 1; i <= 12; i++) {
            event(MotionEvent.ACTION_MOVE, from.centerX() + (to.centerX() - from.centerX()) * i / 12f,
                from.centerY() + (to.centerY() - from.centerY()) * i / 12f);
            SystemClock.sleep(20);
        }
    }
    private void event(int action, float x, float y) {
        MotionEvent e = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0);
        e.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try { if (!automation.injectInputEvent(e, true)) throw new AssertionError("Touch injection refused"); }
        finally { e.recycle(); }
    }
    private static JSONObject read(File f) throws Exception {
        return new JSONObject(new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8));
    }
    private void write(File f, String text) throws Exception { publish(f, text.getBytes(StandardCharsets.UTF_8)); }
}

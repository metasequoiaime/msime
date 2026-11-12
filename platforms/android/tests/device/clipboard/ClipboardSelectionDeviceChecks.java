package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicBoolean;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Predicate;
import java.util.regex.Pattern;
import org.json.JSONArray;
import org.json.JSONObject;

/** 五条合成剪贴板记录的真实点按：选择、上限确认和关闭期间持续观察输入法窗口。 */
final class ClipboardSelectionDeviceChecks {
    private final DeviceSmoke s;
    private final android.app.UiAutomation automation;

    private ClipboardSelectionDeviceChecks(DeviceSmoke smoke) { s = smoke; automation = smoke.automation; }

    static void run(DeviceSmoke smoke) throws Exception { new ClipboardSelectionDeviceChecks(smoke).runChecks(); }
    private static final String LIMIT = "一次最多插入 4 项，导出和删除不受影响";
    private final Map<File, byte[]> backups = new LinkedHashMap<>();

    private void runChecks() throws Exception {
        File root = s.getTargetContext().getFilesDir();
        File preferences = s.sharedPreferencesFile();
        File history = new File(root, "MSIME/clipboard_history.json");
        File local = new File(root, "bootstrap/state/android-settings.json");
        for (File file : new File[] {preferences, history, local})
            backups.put(file, file.isFile() ? Files.readAllBytes(file.toPath()) : null);
        String size = override("size", "[0-9]+x[0-9]+");
        String density = override("density", "[0-9]+");
        String font = shell("settings get system font_scale").trim();
        try {
            // 报告中的手机尺寸与字号；使用合成内容，退出时恢复专用模拟器原配置。
            s.shellDiscard("wm size 1080x2376");
            s.shellDiscard("wm density 480");
            s.shellDiscard("settings put system font_scale 1.25002");
            long deadline = SystemClock.uptimeMillis() + 15000;
            while (true) {
                android.content.res.Configuration config = s.getTargetContext().getResources().getConfiguration();
                if (config.densityDpi == 480 && Math.abs(config.fontScale - 1.25002f) < .001f) break;
                if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Display fixture did not settle");
                SystemClock.sleep(100);
            }
            s.putSharedPreference("clipboard_history", true);
            s.putSharedPreference("scheme", "quanpin");
            s.putSharedPreference("touch_toolbar", new JSONObject().put("clipboard", true));
            write(local, new JSONObject().put("version", 1).put("settings", new JSONObject()
                .put("platform.android.toolbar_hidden", false)
                .put("platform.android.clipboard_columns", "two")).toString());
            JSONArray records = new JSONArray();
            for (int i = 1; i <= 5; i++) records.put(new JSONObject().put("text", text(i))
                .put("timestampMs", 1700000000000L + i).put("pinned", true));
            write(history, records.toString());
            s.restartIme();
            s.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity"
                + " --ez msime_test_editor_probe true");
            s.tap(s.field("msime-test-plain"));
            s.await(s.key("n"));
            s.tap(s.clipboardEntry());
            s.tap(s.key("多选"));
            for (int i = 1; i <= 4; i++) touch(card(i));
            s.await(s.key("4 项已选择"));
            screenshot("four");

            s.stage = "fifth selection keeps IME visible and does not show an insertion warning";
            stableTouch(card(5));
            s.await(s.key("5 项已选择"));
            for (int i = 1; i <= 5; i++) s.await(card(i).and(AccessibilityNodeInfo::isSelected));
            if (visible(node -> DeviceSmoke.equalsText(LIMIT, node.getText())) != null)
                throw new AssertionError("Selecting the fifth item displayed an insertion warning");
            s.await(s.key("导出").and(AccessibilityNodeInfo::isEnabled));
            s.await(s.key("删除").and(AccessibilityNodeInfo::isEnabled));
            assertEmptyFocusedEditor();
            screenshot("five");

            s.stage = "insertion warning opens without hiding or resizing the IME";
            stableTouch(s.key("插入"));
            AccessibilityNodeInfo message = s.awaitAny(s.described(LIMIT));
            AccessibilityNodeInfo done = s.awaitAny(s.described("知道了"));
            Rect label = bounds(message), button = bounds(done), panel = bounds(message.getParent());
            if (label.isEmpty() || button.isEmpty() || button.centerY() <= label.centerY()
                    || button.centerX() <= panel.centerX())
                throw new AssertionError("Acknowledgement button is not visible below the message on the right");
            if (Build.VERSION.SDK_INT >= 30) {
                Bitmap screen = automation.takeScreenshot();
                if (screen == null) throw new AssertionError("No screen for dialog positioning");
                try {
                    // 系统栏会从可用显示区域扣掉少量像素；不能把键盘内居中误当作屏幕居中。
                    if (Math.abs(panel.centerY() - screen.getHeight() / 2) > screen.getHeight() / 12)
                        throw new AssertionError("Acknowledgement is not centered on screen: " + panel);
                } finally { screen.recycle(); }
            }
            assertEmptyFocusedEditor();
            screenshot("dialog");

            s.stage = "acknowledgement dismiss keeps five selections and the editor focused";
            stableTouch(s.described("知道了"));
            if (visible(s.described(LIMIT)) != null) throw new AssertionError("Acknowledgement did not close");
            s.await(s.key("5 项已选择"));
            assertEmptyFocusedEditor();
            screenshot("dismissed");

            s.stage = "returning to four items inserts them in selection order";
            touch(card(5));
            s.await(s.key("4 项已选择"));
            touch(s.key("插入"));
            s.await(s.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(
                text(1) + text(2) + text(3) + text(4), node.getText())));
        } catch (Exception | AssertionError error) {
            screenshot("failure");
            throw error;
        } finally {
            for (Map.Entry<File, byte[]> entry : backups.entrySet()) {
                if (entry.getValue() == null) Files.deleteIfExists(entry.getKey().toPath());
                else s.publish(entry.getKey(), entry.getValue());
            }
            s.shellDiscard("wm size " + size);
            s.shellDiscard("wm density " + density);
            s.shellDiscard(font.matches("[0-9]+(?:\\.[0-9]+)?")
                ? "settings put system font_scale " + font : "settings delete system font_scale");
            s.restartIme();
        }
    }

    private static String text(int number) { return "Synthetic clip " + number; }
    private Predicate<AccessibilityNodeInfo> card(int number) {
        return node -> node.isClickable() && node.getText() != null
            && node.getText().toString().startsWith(text(number));
    }
    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect();
        if (node != null) node.getBoundsInScreen(result);
        return result;
    }
    private void assertEmptyFocusedEditor() {
        s.await(s.field("msime-test-plain").and(node -> node.isFocused()
            && (node.isShowingHintText() || node.getText() == null || node.getText().length() == 0)));
    }

    private AccessibilityNodeInfo visible(Predicate<AccessibilityNodeInfo> match) {
        s.dropStaleNodes();
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            AccessibilityNodeInfo node = s.find(window.getRoot(), match);
            if (node != null) return node;
        }
        return null;
    }

    private AccessibilityWindowInfo imeWindow() {
        for (AccessibilityWindowInfo window : automation.getWindows()) {
            if (window.getType() == AccessibilityWindowInfo.TYPE_INPUT_METHOD) return window;
        }
        throw new AssertionError("IME window disappeared during " + s.stage);
    }

    private void stableTouch(Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = s.awaitAny(match);
        automation.waitForIdle(100, 2000);
        AccessibilityWindowInfo before = imeWindow();
        int windowId = before.getId();
        Rect expected = new Rect();
        before.getBoundsInScreen(expected);
        if (expected.isEmpty()) throw new AssertionError("IME baseline is empty");
        AtomicBoolean running = new AtomicBoolean(true);
        AtomicReference<Throwable> failure = new AtomicReference<>();
        Thread monitor = new Thread(() -> {
            try {
                while (running.get()) {
                    AccessibilityWindowInfo current = imeWindow();
                    Rect actual = new Rect();
                    current.getBoundsInScreen(actual);
                    if (current.getId() != windowId || !actual.equals(expected))
                        throw new AssertionError("IME window changed: " + windowId + " " + expected
                            + " -> " + current.getId() + " " + actual);
                    SystemClock.sleep(20);
                }
            } catch (Throwable error) { failure.set(error); }
        }, "clipboard-window-observer");
        monitor.start();
        try {
            touch(target);
            SystemClock.sleep(1200);
        } finally {
            running.set(false);
            monitor.join(5000);
        }
        if (monitor.isAlive()) throw new AssertionError("IME observer did not stop");
        if (failure.get() != null) throw new AssertionError("Clipboard interaction disturbed IME", failure.get());
    }

    private void touch(Predicate<AccessibilityNodeInfo> match) {
        touch(s.awaitAny(match));
        SystemClock.sleep(200);
    }
    private void touch(AccessibilityNodeInfo node) {
        Rect point = bounds(node);
        if (!node.isVisibleToUser() || point.isEmpty()) throw new AssertionError("Touch target is not drawn");
        long time = SystemClock.uptimeMillis();
        for (int action : new int[] {MotionEvent.ACTION_DOWN, MotionEvent.ACTION_UP}) {
            MotionEvent event = MotionEvent.obtain(time, SystemClock.uptimeMillis(), action,
                point.centerX(), point.centerY(), 0);
            event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
            try {
                if (!automation.injectInputEvent(event, true)) throw new AssertionError("Touch injection refused");
            } finally { event.recycle(); }
        }
    }
    private void write(File file, String text) throws Exception {
        s.publish(file, text.getBytes(StandardCharsets.UTF_8));
    }
    private String shell(String command) throws Exception {
        try (ParcelFileDescriptor.AutoCloseInputStream input = new ParcelFileDescriptor.AutoCloseInputStream(
                automation.executeShellCommand(command))) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            byte[] buffer = new byte[1024];
            int count;
            while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            return new String(output.toByteArray(), StandardCharsets.UTF_8);
        }
    }
    private String override(String kind, String value) throws Exception {
        java.util.regex.Matcher match = Pattern.compile("Override " + kind + ": (" + value + ")")
            .matcher(shell("wm " + kind));
        return match.find() ? match.group(1) : "reset";
    }
    private void screenshot(String name) throws Exception {
        Bitmap bitmap = automation.takeScreenshot();
        if (bitmap == null) throw new AssertionError("No synthetic clipboard screenshot");
        try (FileOutputStream output = new FileOutputStream(new File(s.getTargetContext().getCacheDir(),
                "clipboard-selection-" + name + ".png"))) {
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, output);
        } finally { bitmap.recycle(); }
    }
}

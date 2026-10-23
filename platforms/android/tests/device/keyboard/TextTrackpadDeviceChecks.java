package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.function.Predicate;
import org.json.JSONObject;

/** 通过安装产品和独立编辑器验收触控板，不以局部手势计算替代系统按键通道。 */
final class TextTrackpadDeviceChecks {
    private final DeviceSmoke t;
    private final float density;
    private String kind;

    TextTrackpadDeviceChecks(DeviceSmoke test) {
        t = test;
        density = test.getTargetContext().getResources().getDisplayMetrics().density;
    }

    void run() throws Exception {
        File root = t.getTargetContext().getFilesDir();
        JSONObject options = json(new File(root, "runtime-options.json"));
        File preferences = new File(options.getString("preferences_directory"), "preferences.json").getCanonicalFile();
        if (!preferences.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Synthetic preferences escaped sandbox");
        File local = new File(root, "bootstrap/state/android-settings.json");
        byte[] original = bytes(preferences), originalLocal = bytes(local);
        JSONObject document = original == null ? new JSONObject().put("format_version", 1).put("revision", 0)
            .put("preferences", options.getJSONObject("preferences")) : json(preferences);
        JSONObject values = document.getJSONObject("preferences");
        values.put("clipboard_history", false).put("cloud_candidates", false).put("learning", false);
        values.put("android_text_trackpad", new JSONObject().put("enabled", true).put("hold_to_open", true)
            .put("horizontal_dp", 12).put("vertical_dp", 28).put("tap_selection", true)
            .put("haptics", false).put("trail", true).put("magnifier", true));
        JSONObject settings = originalLocal == null ? new JSONObject().put("version", 1)
            .put("settings", new JSONObject()) : json(local);
        settings.getJSONObject("settings").put("platform.android.toolbar_text_edit", true)
            .put("platform.android.toolbar_hidden", false).put("platform.android.hardware_caret_candidates", false);
        try {
            document.put("revision", document.getLong("revision") + 1);
            t.publish(preferences, document.toString().getBytes(StandardCharsets.UTF_8));
            t.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            t.restartIme();
            open("password");
            stage("password preview before engine startup");
            preview("trackpad-magnifier-password-first.png");
            open("edit");
            stage("trackpad horizontal motion");
            drag(-24.25f, 0);
            selection(6, 6);
            drag(12.25f, 0);
            selection(7, 7);
            stage("trackpad vertical motion");
            drag(0, 28.25f);
            selection(18, 18);
            drag(0, -28.25f);
            selection(7, 7);
            stage("trackpad Shift selection");
            t.tap(t.described("选择模式"));
            drag(12.25f, 0);
            selection(7, 8);
            t.tap(t.described("取消选择"));
            setSelection(8, 8);
            stage("trackpad temporary long press selection");
            setSelection(8, 8);
            Rect longPad = pad();
            android.graphics.Bitmap beforePreview = t.automation.takeScreenshot();
            long longDown = SystemClock.uptimeMillis();
            t.automation.executeAndWaitForEvent(
                () -> touch(longDown, MotionEvent.ACTION_DOWN, longPad.centerX(), longPad.centerY()),
                event -> event.getEventType() == android.view.accessibility.AccessibilityEvent.TYPE_ANNOUNCEMENT
                    && event.getText().toString().contains("拖动延伸选区"), 15000);
            try { screenshot("trackpad-magnifier-edit.png", beforePreview); }
            finally { if (beforePreview != null) beforePreview.recycle(); }
            dragMove(longDown, longPad.centerX(), longPad.centerY(), 12.25f * density, 0);
            touch(longDown, MotionEvent.ACTION_UP, longPad.centerX() + 12.25f * density, longPad.centerY());
            selection(8, 9);
            setSelection(8, 8);
            drag(-12.25f, 0);
            selection(7, 7);
            setSelection(8, 8);
            stage("trackpad second finger cancels movement and remainder");
            Rect cancelPad = pad();
            long cancelDown = SystemClock.uptimeMillis();
            touch(cancelDown, MotionEvent.ACTION_DOWN, cancelPad.centerX(), cancelPad.centerY());
            touch(cancelDown, MotionEvent.ACTION_MOVE, cancelPad.centerX() + 6 * density, cancelPad.centerY());
            secondPointer(cancelDown, MotionEvent.ACTION_POINTER_DOWN, cancelPad.centerX() + 6 * density, cancelPad.centerY());
            secondPointer(cancelDown, MotionEvent.ACTION_POINTER_UP, cancelPad.centerX() + 6 * density, cancelPad.centerY());
            touch(cancelDown, MotionEvent.ACTION_MOVE, cancelPad.centerX() + 24 * density, cancelPad.centerY());
            touch(cancelDown, MotionEvent.ACTION_UP, cancelPad.centerX() + 24 * density, cancelPad.centerY());
            selection(8, 8);
            drag(6, 0);
            selection(8, 8);
            stage("trackpad double tap word");
            taps(2);
            selection(6, 10);
            setSelection(8, 8);
            stage("trackpad triple tap line");
            taps(3);
            selection(0, 10);
            stage("trackpad last word and unterminated final line");
            setSelection(31, 31);
            taps(2);
            selection(31, 35);
            setSelection(29, 29);
            taps(3);
            selection(23, 35);
            stage("trackpad existing clipboard actions");
            t.tap(t.described("全选"));
            selection(0, TrackpadEditorActivity.TEXT.length());
            t.tap(t.described("复制"));
            t.tap(t.described("剪切"));
            t.await(t.field("trackpad-editor").and(node -> node.getText() == null || node.getText().length() == 0));
            t.tap(t.described("粘贴"));
            t.await(t.field("trackpad-editor").and(node -> DeviceSmoke.equalsText(TrackpadEditorActivity.TEXT, node.getText())));
            stage("trackpad preserves space, newline and number shortcuts");
            setSelection(TrackpadEditorActivity.TEXT.length(), TrackpadEditorActivity.TEXT.length());
            shortcut("插入空格");
            t.await(t.field("trackpad-editor").and(node -> DeviceSmoke.equalsText(TrackpadEditorActivity.TEXT + " ", node.getText())));
            shortcut("插入换行");
            t.await(t.field("trackpad-editor").and(node -> DeviceSmoke.equalsText(TrackpadEditorActivity.TEXT + " \n", node.getText())));
            shortcut("打开数字键盘");
            t.await(t.described("切换到字母键盘"));
            t.tap(t.described("切换到字母键盘"));
            open("edit");
            stage("trackpad mode persistence");
            t.tap(t.described("切换为方向键"));
            waitMode(preferences, false);
            t.restartIme();
            launch("edit");
            t.tap(t.described("文本编辑"));
            t.await(t.described("光标左移"));
            t.tap(t.described("切换为触控板"));
            waitMode(preferences, true);
            pad();
            stage("temporary trackpad restores prior panel");
            t.tap(t.described("返回键盘"));
            t.tap(t.key("更多"));
            t.await(t.toolPanel());
            Rect tool = bounds(t.await(t.described("文本编辑")));
            long down = SystemClock.uptimeMillis();
            touch(down, MotionEvent.ACTION_DOWN, tool.centerX(), tool.centerY());
            Rect pad = pad();
            touch(down, MotionEvent.ACTION_MOVE, pad.centerX(), pad.centerY());
            touch(down, MotionEvent.ACTION_MOVE, pad.centerX() - 12.25f * density, pad.centerY());
            touch(down, MotionEvent.ACTION_UP, pad.centerX() - 12.25f * density, pad.centerY());
            t.await(t.toolPanel());
            gone(t.described("文本编辑面板"));
            stage("WebView actual cursor movement");
            open("web");
            drag(-24.25f, 0);
            selection(6, 6);
            drag(0, 28.25f);
            selection(17, 17);
            stage("editor without cursor positions");
            open("no-anchor");
            preview("trackpad-magnifier-fallback.png");
            drag(-12.25f, 0);
            selection(7, 7);
            stage("long selection preview");
            open("long-selection");
            setSelection(8, 6500);
            preview("trackpad-magnifier-long-selection.png");
            stage("password preview does not read plain text");
            open("password");
            String reads = t.await(t.field("trackpad-status")).getText().toString();
            preview("trackpad-magnifier-password.png");
            pad = pad();
            down = SystemClock.uptimeMillis();
            touch(down, MotionEvent.ACTION_DOWN, pad.centerX(), pad.centerY());
            dragMove(down, pad.centerX(), pad.centerY(), -12.25f * density, 0);
            selection(7, 7);
            touch(down, MotionEvent.ACTION_UP, pad.centerX() - 12.25f * density, pad.centerY());
            if (!reads.contentEquals(t.await(t.field("trackpad-status")).getText()))
                throw new AssertionError("Password trackpad attempted a text read");
            stage("native trackpad settings and slider persistence");
            settingsControls(preferences);
            stage("independent key receiver");
            open("terminal");
            drag(-12.25f, 0);
            t.await(t.field("trackpad-status").and(node -> DeviceSmoke.equalsText("0:21:0,1:21:0", node.getText())));
            t.tap(t.described("选择模式"));
            drag(0, -28.25f);
            t.await(t.field("trackpad-status").and(node -> node.getText() != null
                && node.getText().toString().contains("0:19:65,1:19:65,1:59:0")));
            // API 31 起可通过真实 uinput 设备检验两方订阅；旧版的普通设备专项不假装接了实体键盘。
            if (android.os.Build.VERSION.SDK_INT >= 31) {
                stage("magnifier and physical candidate subscription coexistence");
                String originalVirtual = android.provider.Settings.Secure.getString(
                    t.getTargetContext().getContentResolver(), "show_ime_with_hard_keyboard");
                android.os.ParcelFileDescriptor[] keyboard = null;
                try {
                    t.shellDiscard("settings put secure show_ime_with_hard_keyboard 1");
                    keyboard = t.plugKeyboard();
                    DeviceSmoke.physicalKeyboard();
                    settings.getJSONObject("settings").put("platform.android.hardware_caret_candidates", true);
                    t.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
                    t.restartIme();
                    open("subscriptions");
                    String subscribed = t.await(t.field("trackpad-status").and(node -> node.getText() != null
                        && node.getText().toString().endsWith("3;"))).getText().toString();
                    preview("trackpad-magnifier-with-hardware.png");
                    if (!subscribed.contentEquals(t.await(t.field("trackpad-status")).getText()))
                        throw new AssertionError("Magnifier altered the hardware candidate cursor subscription");
                    t.tap(t.described("返回键盘"));
                    t.await(t.key("n"));
                    if (!subscribed.contentEquals(t.await(t.field("trackpad-status")).getText()))
                        throw new AssertionError("Closing trackpad cancelled the hardware cursor subscription");
                } finally {
                    if (keyboard != null) t.unplugKeyboard(keyboard);
                    t.shellDiscard(originalVirtual == null ? "settings delete secure show_ime_with_hard_keyboard"
                        : "settings put secure show_ime_with_hard_keyboard " + originalVirtual);
                }
            }
        } finally {
            t.shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            restore(preferences, original);
            restore(local, originalLocal);
            t.restartIme();
        }
    }

    private void stage(String value) {
        t.stage = value;
        Bundle progress = new Bundle();
        progress.putString("stream", "MSIME_TRACKPAD_STAGE: " + value + "\n");
        t.sendStatus(0, progress);
    }

    private void shortcut(String description) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            t.dropStaleNodes();
            for (AccessibilityWindowInfo window : t.automation.getWindows()) {
                if (t.find(window.getRoot(), t.described(description)) != null) {
                    t.tap(t.described(description).and(AccessibilityNodeInfo::isVisibleToUser));
                    return;
                }
            }
            t.await(t.described("文本编辑快捷操作"))
                .performAction(AccessibilityNodeInfo.ACTION_SCROLL_FORWARD);
            SystemClock.sleep(100);
        }
        throw new AssertionError("Trackpad edit shortcut was unavailable: " + description);
    }

    private void settingsControls(File preferences) throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android/app.msime.android.home.HomeActivity --es msime_open_page KEYBOARD_OPTIONS");
        t.tap(node -> DeviceSmoke.equalsText("手势", node.getText()) && node.isClickable());
        String[][] toggles = {{"优先使用触控板", "enabled"}, {"按住工具临时打开", "hold_to_open"},
            {"双击选词与三击选行", "tap_selection"}, {"逐步振动", "haptics"},
            {"显示移动轨迹", "trail"}, {"光标文字放大镜", "magnifier"}};
        for (String[] entry : toggles) {
            boolean original = json(preferences).getJSONObject("preferences").getJSONObject("android_text_trackpad")
                .getBoolean(entry[1]);
            for (boolean expected : new boolean[] {!original, original}) {
                AccessibilityNodeInfo row = settingsNode(entry[0], false);
                if (!row.performAction(AccessibilityNodeInfo.ACTION_CLICK))
                    throw new AssertionError("Native trackpad toggle rejected " + entry[1]);
                awaitOption(preferences, entry[1], expected);
            }
        }
        for (String[] entry : new String[][] {{"水平移动距离", "horizontal_dp", "4", "48", "12"},
                {"垂直移动距离", "vertical_dp", "8", "96", "28"}}) {
            int min = Integer.parseInt(entry[2]), max = Integer.parseInt(entry[3]);
            for (int desired : new int[] {min, max, Integer.parseInt(entry[4])}) {
                AccessibilityNodeInfo slider = settingsNode(entry[0], true);
                var range = slider.getRangeInfo();
                if (range == null || range.getMin() != 0 || range.getMax() != max - min)
                    throw new AssertionError("Native slider range differs from Rust options");
                Bundle args = new Bundle();
                args.putFloat(AccessibilityNodeInfo.ACTION_ARGUMENT_PROGRESS_VALUE, desired - min);
                if (!slider.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SET_PROGRESS.getId(), args))
                    throw new AssertionError("Native slider adjustment rejected");
                awaitOption(preferences, entry[1], desired);
            }
        }
        t.restartIme();
    }

    private AccessibilityNodeInfo settingsNode(String title, boolean slider) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (SystemClock.uptimeMillis() < deadline) {
            t.dropStaleNodes();
            for (AccessibilityWindowInfo window : t.automation.getWindows()) {
                AccessibilityNodeInfo value = t.findAny(window.getRoot(), node -> slider
                    ? DeviceSmoke.equalsText(title, node.getContentDescription())
                    : DeviceSmoke.equalsText(title, node.getText()));
                if (value == null) continue;
                if (!slider) {
                    while (value != null && !value.isCheckable()) value = value.getParent();
                }
                if (value == null) continue;
                value.performAction(AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.getId());
                Rect visible = bounds(value);
                if (value.isVisibleToUser() && !visible.isEmpty()) return value;
            }
            SystemClock.sleep(100);
        }
        throw new AssertionError("Native trackpad setting was unavailable: " + title);
    }

    private void awaitOption(File file, String key, Object expected) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (!expected.equals(json(file).getJSONObject("preferences").getJSONObject("android_text_trackpad").get(key))) {
            if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Native trackpad setting was not persisted: " + key);
            SystemClock.sleep(100);
        }
    }

    private void preview(String name) throws Exception {
        t.automation.waitForIdle(200, 5000);
        android.graphics.Bitmap before = t.automation.takeScreenshot();
        Rect rect = pad();
        long down = SystemClock.uptimeMillis();
        try {
            t.automation.executeAndWaitForEvent(
                () -> touch(down, MotionEvent.ACTION_DOWN, rect.centerX(), rect.centerY()),
                event -> event.getEventType() == android.view.accessibility.AccessibilityEvent.TYPE_ANNOUNCEMENT
                    && event.getText().toString().contains("拖动延伸选区"), 15000);
            screenshot(name, before);
        } finally {
            touch(down, MotionEvent.ACTION_UP, rect.centerX(), rect.centerY());
            if (before != null) before.recycle();
        }
    }

    private void screenshot(String name, android.graphics.Bitmap before) throws Exception {
        int keyboardTop = bounds(t.await(t.described("键盘快捷栏"))).top;
        int resource = android.content.res.Resources.getSystem().getIdentifier("status_bar_height", "dimen", "android");
        int statusBottom = resource == 0 ? 0 : android.content.res.Resources.getSystem().getDimensionPixelSize(resource);
        android.graphics.Bitmap bitmap;
        int changed, systemBarChanged;
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (true) {
            bitmap = t.automation.takeScreenshot();
            if (bitmap == null) throw new AssertionError("Synthetic trackpad screenshot unavailable");
            if (before == null || before.getWidth() != bitmap.getWidth() || before.getHeight() != bitmap.getHeight()) {
                bitmap.recycle();
                throw new AssertionError("Synthetic preview baseline unavailable");
            }
            changed = systemBarChanged = 0;
            for (int y = 0; y < keyboardTop; y++)
                for (int x = 0; x < bitmap.getWidth(); x++)
                    if (before.getPixel(x, y) != bitmap.getPixel(x, y)) {
                        changed++;
                        if (y < statusBottom) systemBarChanged++;
                    }
            if (changed >= bitmap.getWidth() * 8 || SystemClock.uptimeMillis() >= deadline) break;
            bitmap.recycle();
            // 回读和浮窗排版异步完成，等实际像素出现，不把长按公告当成绘制结束。
            SystemClock.sleep(100);
        }
        try (var output = new java.io.FileOutputStream(new File(t.getTargetContext().getExternalFilesDir(null), name))) {
            if (!bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, output))
                throw new AssertionError("Synthetic trackpad screenshot failed");
            if (systemBarChanged > bitmap.getWidth() * 2)
                throw new AssertionError("Magnifier overlaps status bar: " + systemBarChanged + " changed pixels");
            // 光标闪烁只改几十个像素；放大镜必须在键盘上方绘制出可见面积。
            if (changed < bitmap.getWidth() * 8)
                throw new AssertionError("Magnifier did not draw above the keyboard: " + changed + " changed pixels");
        } finally { bitmap.recycle(); }
    }

    private void open(String value) throws Exception {
        launch(value);
        t.tap(t.described("文本编辑"));
        pad();
    }

    private void launch(String value) throws Exception {
        kind = value;
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.TrackpadEditorActivity --es kind " + value);
        t.await(t.described("文本编辑"));
        if (!"password".equals(value))
            t.await(t.describedPrefix("输入方案：").and(AccessibilityNodeInfo::isEnabled));
        if ("web".equals(value)) t.await(t.field("trackpad-status").and(node -> DeviceSmoke.equalsText("start=8;end=8", node.getText())));
        else {
            selection(8, 8);
            t.await(t.field("trackpad-editor").and(AccessibilityNodeInfo::isFocused));
        }
        t.automation.waitForIdle(500, 5000);
    }

    private Rect pad() { return bounds(t.awaitStableBounds(t.describedPrefix("文本编辑触控板"))); }

    private void drag(float dxDp, float dyDp) {
        Rect rect = pad();
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, rect.centerX(), rect.centerY());
        dragMove(down, rect.centerX(), rect.centerY(), dxDp * density, dyDp * density);
        touch(down, MotionEvent.ACTION_UP, rect.centerX() + dxDp * density, rect.centerY() + dyDp * density);
    }

    private void dragMove(long down, float x, float y, float dx, float dy) {
        for (int at = 1; at <= 6; at++) touch(down, MotionEvent.ACTION_MOVE, x + dx * at / 6, y + dy * at / 6);
    }

    private void taps(int count) {
        Rect rect = pad();
        // Binder 注入自身可耗时数百毫秒；用事件时间表达快速连点，不能把 IPC 等待算入双击间隔。
        long reset = SystemClock.uptimeMillis() - (count + 1) * 40;
        touch(reset, reset, MotionEvent.ACTION_DOWN, rect.centerX(), rect.centerY());
        touch(reset, reset + 1, MotionEvent.ACTION_CANCEL, rect.centerX(), rect.centerY());
        for (int at = 0; at < count; at++) {
            long down = reset + (at + 1) * 40;
            touch(down, down, MotionEvent.ACTION_DOWN, rect.centerX(), rect.centerY());
            touch(down, down + 10, MotionEvent.ACTION_UP, rect.centerX(), rect.centerY());
        }
    }

    private void secondPointer(long down, int action, float x, float y) {
        MotionEvent.PointerProperties[] properties = new MotionEvent.PointerProperties[2];
        MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[2];
        for (int at = 0; at < 2; at++) {
            properties[at] = new MotionEvent.PointerProperties();
            properties[at].id = at;
            properties[at].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coords[at] = new MotionEvent.PointerCoords();
            coords[at].x = x + at * 20 * density;
            coords[at].y = y;
            coords[at].pressure = coords[at].size = 1;
        }
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(),
            action | 1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT, 2, properties, coords,
            0, 0, 1, 1, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
        try {
            if (!t.automation.injectInputEvent(event, true)) throw new AssertionError("Trackpad second pointer injection rejected");
        } finally { event.recycle(); }
    }

    private void touch(long down, int action, float x, float y) {
        touch(down, SystemClock.uptimeMillis(), action, x, y);
    }

    private void touch(long down, long eventAt, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, eventAt, action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            // 同步等待每个事件会把普通拖动拉长成真正的长按；连续投递完整手势后由编辑器状态断言收尾。
            if (!t.automation.injectInputEvent(event, false)) throw new AssertionError("Trackpad touch injection rejected");
        } finally { event.recycle(); }
    }

    private void selection(int start, int end) {
        if ("web".equals(kind)) {
            t.await(t.field("trackpad-status").and(node -> DeviceSmoke.equalsText("start=" + start + ";end=" + end, node.getText())));
        } else t.await(t.field("trackpad-editor").and(node -> node.getTextSelectionStart() == start && node.getTextSelectionEnd() == end));
    }

    private void setSelection(int start, int end) {
        AccessibilityNodeInfo current = t.await(t.field("trackpad-editor"));
        if (current.getTextSelectionStart() == start && current.getTextSelectionEnd() == end) return;
        Bundle arguments = new Bundle();
        arguments.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start);
        arguments.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, end);
        if (!t.await(t.field("trackpad-editor")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, arguments))
            throw new AssertionError("Synthetic editor selection rejected");
        selection(start, end);
    }

    private void waitMode(File file, boolean enabled) throws Exception {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (json(file).getJSONObject("preferences").getJSONObject("android_text_trackpad").getBoolean("enabled") != enabled) {
            if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Trackpad mode was not persisted");
            SystemClock.sleep(100);
        }
    }

    private void gone(Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        while (true) {
            t.dropStaleNodes();
            boolean found = false;
            for (AccessibilityWindowInfo window : t.automation.getWindows())
                if (t.find(window.getRoot(), match) != null) { found = true; break; }
            if (!found) return;
            if (SystemClock.uptimeMillis() >= deadline) throw new AssertionError("Temporary panel was not restored");
            SystemClock.sleep(100);
        }
    }

    private static Rect bounds(AccessibilityNodeInfo node) { Rect rect = new Rect(); node.getBoundsInScreen(rect); return rect; }
    private static JSONObject json(File file) throws Exception { return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8)); }
    private static byte[] bytes(File file) throws Exception { return file.isFile() ? Files.readAllBytes(file.toPath()) : null; }
    private void restore(File file, byte[] original) throws Exception {
        if (original == null) Files.deleteIfExists(file.toPath()); else t.publish(file, original);
    }
}

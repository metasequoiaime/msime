package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 用真实触摸和键区像素验证静止长按提示、连删、上滑清空及取消。 */
public final class QuickDeletePreviewDeviceSmoke extends DeviceSmoke {
    @Override protected String successDescription() {
        return "stationary hold reveals quick-delete preview without clearing, in 26-key and 9-key layouts";
    }

    @Override protected void runChecks() throws Exception {
        File root = getTargetContext().getFilesDir();
        JSONObject options = new JSONObject(new String(Files.readAllBytes(
            new File(root, "runtime-options.json").toPath()), StandardCharsets.UTF_8));
        File directory = new File(options.getString("preferences_directory")).getCanonicalFile();
        if (!directory.toPath().startsWith(root.getCanonicalFile().toPath()))
            throw new AssertionError("Preferences escaped the test sandbox");
        File preferences = new File(directory, "preferences.json");
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        long revision = original == null ? 0 : new JSONObject(new String(original,
            StandardCharsets.UTF_8)).getLong("revision");
        try {
            JSONObject values = new JSONObject(options.getJSONObject("preferences").toString())
                .put("scheme", "quanpin").put("touch_keyboard_layout", "twenty_six_key")
                .put("ime_mode_scope", "app").put("default_ime_mode", "chinese")
                .put("touch_keyboard_schemes", new JSONObject().put("enabled",
                    new JSONArray().put("quanpin").put("nine_key")).put("selected", "quanpin"));
            publish(preferences, new JSONObject().put("format_version", 1).put("revision", revision + 1)
                .put("preferences", values).toString().getBytes(StandardCharsets.UTF_8));
            restartIme();
            shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.QuickDeleteEditorActivity");
            tap(field("msime-test-plain"));
            await(key("⌫"));
            for (String layout : new String[] {"全拼 26 键", "全拼 9 键"}) {
                stage = "select layout " + layout;
                tap(describedPrefix("输入方案："));
                tap(described("输入方案卡片 " + layout));
                await(described("输入方案：" + layout));
                for (var window : automation.getWindows()) {
                    if (find(window.getRoot(), key("英")) != null) {
                        tap(key("英"));
                        break;
                    }
                }
                stationaryHold(layout, false);
                stationaryHold(layout, true);
                swipeAfterHold(layout);
            }
        } finally {
            shellDiscard("am start -W -n app.msime.android/app.msime.android.home.HomeActivity");
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else publish(preferences, original);
            restartIme();
        }
    }

    private void setup() {
        Bundle text = new Bundle();
        text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,
            "合成文本".repeat(50) + "尾");
        if (!await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
            throw new AssertionError("Synthetic editor setup failed");
        Bundle selection = new Bundle();
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, 200);
        selection.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, 200);
        await(field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, selection);
        await(field("msime-test-plain").and(node -> node.getTextSelectionStart() == 200));
        SystemClock.sleep(250);
        // 设置合成文本会重启输入会话；方案按钮启用后引擎才已就绪，不能按住即将替换的占位键。
        awaitStableBounds(describedPrefix("输入方案：").and(AccessibilityNodeInfo::isEnabled));
    }

    private void stationaryHold(String layout, boolean cancel) throws Exception {
        stage = "stationary hold preview " + layout + (cancel ? " cancel" : " release");
        setup();
        Rect key = deleteBounds();
        Rect region = previewRegion(key);
        Bitmap before = capture(region);
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        try {
            SystemClock.sleep(600);
            Bitmap held = null;
            int changed = 0;
            long previewDeadline = SystemClock.uptimeMillis() + 1200;
            do {
                if (held != null) held.recycle();
                held = capture(region);
                changed = changedPixels(before, held);
                if (changed >= 60 * getContext().getResources().getDisplayMetrics().density
                        * getContext().getResources().getDisplayMetrics().density) break;
                SystemClock.sleep(100);
            } while (SystemClock.uptimeMillis() < previewDeadline);
            try {
                float density = getContext().getResources().getDisplayMetrics().density;
                try (FileOutputStream output = new FileOutputStream(new File(
                        getTargetContext().getFilesDir(), layout.contains("9")
                            ? "quick-delete-held-nine.png" : "quick-delete-held-letters.png"))) {
                    held.compress(Bitmap.CompressFormat.PNG, 100, output);
                }
                if (changed < 60 * density * density)
                    throw new AssertionError("No visible preview above a stationary held delete key; band="
                        + region.height() / density + "dp; key=" + key.toShortString() + ";length=" + length());
            } finally { held.recycle(); }
            int length = length();
            if (length <= 1 || length >= 200)
                throw new AssertionError("Preview stopped ordinary repeat deletion or cleared all text");
        } finally {
            before.recycle();
            touch(down, cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP,
                key.centerX(), key.centerY());
        }
        SystemClock.sleep(200);
        int released = length();
        SystemClock.sleep(250);
        if (released <= 1 || length() != released)
            throw new AssertionError("Release or cancel cleared text or left repeat deletion running");
    }

    private int changedPixels(Bitmap before, Bitmap after) {
        int changed = 0;
        for (int y = 0; y < after.getHeight(); y++) {
            for (int x = 0; x < after.getWidth(); x++) {
                if (after.getPixel(x, y) != before.getPixel(x, y)) changed++;
            }
        }
        return changed;
    }

    private void swipeAfterHold(String layout) {
        stage = "held preview then swipe clear " + layout;
        setup();
        Rect key = deleteBounds();
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        SystemClock.sleep(550);
        float y = key.top - 42 * getContext().getResources().getDisplayMetrics().density;
        touch(down, MotionEvent.ACTION_MOVE, key.centerX(), y);
        touch(down, MotionEvent.ACTION_UP, key.centerX(), y);
        long deadline = SystemClock.uptimeMillis() + 5000;
        do {
            if (length() == 1) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Swiping into the preview did not clear the prefix");
    }

    private int length() {
        CharSequence text = await(field("msime-test-plain")).getText();
        return text == null ? 0 : text.length();
    }

    private Rect deleteBounds() {
        Rect bounds = new Rect();
        awaitStableBounds(key("⌫")).getBoundsInScreen(bounds);
        return bounds;
    }

    private Rect previewRegion(Rect key) {
        float density = getContext().getResources().getDisplayMetrics().density;
        for (var window : automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect bounds = new Rect();
            window.getBoundsInScreen(bounds);
            bounds.top = Math.max(bounds.top, key.top - Math.round(100 * density));
            bounds.bottom = key.top - Math.round(8 * density);
            if (!bounds.isEmpty()) return bounds;
        }
        throw new AssertionError("No keyboard-only preview region");
    }

    private Bitmap capture(Rect region) {
        Bitmap screen = automation.takeScreenshot();
        if (screen == null) throw new AssertionError("Keyboard capture unavailable");
        try {
            // 只保留键区上方的提示带，编辑器内容不写入截图。
            return Bitmap.createBitmap(screen, region.left, region.top, region.width(), region.height());
        } finally { screen.recycle(); }
    }

    private void touch(long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(event, true)) throw new AssertionError("Touch injection rejected");
        } finally { event.recycle(); }
    }
}

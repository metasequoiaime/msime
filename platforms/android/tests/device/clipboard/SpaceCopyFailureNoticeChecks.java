package app.msime.android.test;

import android.graphics.Bitmap;
import android.graphics.Rect;
import android.os.Build;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.io.FileOutputStream;

/** 在合成复制编辑器里验证屏幕顶部提示及键盘窗口稳定性，不读取其他通知内容。 */
final class SpaceCopyFailureNoticeChecks implements AutoCloseable {
    private static final String MESSAGE = "当前输入框不支持复制全文";
    private final DeviceSmoke test;
    private volatile boolean legacyMessage;
    private Rect keyboard;
    private Rect letter;

    SpaceCopyFailureNoticeChecks(DeviceSmoke test) {
        this.test = test;
        test.automation.setOnAccessibilityEventListener(event -> {
            if (event.getEventType() == AccessibilityEvent.TYPE_NOTIFICATION_STATE_CHANGED
                    && DeviceSmoke.equalsText("app.msime.android", event.getPackageName())
                    && event.getText().contains(MESSAGE)) legacyMessage = true;
        });
    }

    void before() {
        legacyMessage = false;
        letter = bounds(test.awaitStableBounds(test.key("a")));
        keyboard = keyboardBounds();
    }

    void verify() throws Exception {
        test.stage = "copy failure notice at phone top without moving or hiding the keyboard";
        long deadline = SystemClock.uptimeMillis() + 10000;
        boolean captured = false;
        do {
            stable();
            Bitmap screenshot = test.automation.takeScreenshot();
            if (screenshot == null) throw new AssertionError("复制提示截图不可用");
            try {
                Rect notice = noticeBounds(screenshot);
                if (notice != null) {
                    if (notice.isEmpty() || notice.top < 0 || notice.bottom > screenshot.getHeight() / 4
                            || notice.bottom >= keyboard.top
                            || Math.abs(notice.centerX() - screenshot.getWidth() / 2) > 4 * density())
                        throw new AssertionError("复制失败提示不在屏幕顶部中央：" + notice);
                    File file = new File(test.getTargetContext().getExternalFilesDir(null), "space-copy-failure-notice.png");
                    try (FileOutputStream output = new FileOutputStream(file)) {
                        if (!screenshot.compress(Bitmap.CompressFormat.PNG, 100, output))
                            throw new AssertionError("复制提示截图写入失败");
                    }
                    captured = true;
                    break;
                }
            } finally { screenshot.recycle(); }
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        if (!captured) throw new AssertionError("复制失败提示没有在屏幕顶部出现");

        // 继续采样到提示消失，再观察半秒；显示、自动关闭两条路径都不能重排或关闭键盘。
        deadline = SystemClock.uptimeMillis() + 8000;
        long goneAt = 0;
        do {
            stable();
            Bitmap screenshot = test.automation.takeScreenshot();
            if (screenshot == null) throw new AssertionError("复制提示截图不可用");
            boolean visible;
            try { visible = noticeBounds(screenshot) != null; }
            finally { screenshot.recycle(); }
            if (visible) goneAt = 0;
            else if (goneAt == 0) goneAt = SystemClock.uptimeMillis();
            else if (SystemClock.uptimeMillis() - goneAt >= 500) return;
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("复制失败提示没有自动消失");
    }

    private Rect noticeBounds(Bitmap screenshot) {
        if (Build.VERSION.SDK_INT < 30)
            return legacyMessage ? ClipboardPhraseNoticeDeviceCheck.legacyNoticeBounds(screenshot) : null;
        for (var window : test.automation.getWindows()) {
            if (!DeviceSmoke.equalsText("MSIME space copy notice", window.getTitle())) continue;
            AccessibilityNodeInfo node = test.find(window.getRoot(), test.described("空格复制提示：" + MESSAGE));
            if (node != null) return bounds(node);
        }
        return null;
    }

    private void stable() {
        if (!keyboard.equals(keyboardBounds())) throw new AssertionError("复制提示改变了键盘窗口范围");
        AccessibilityNodeInfo key = null, editor = null;
        for (var window : test.automation.getWindows()) {
            if (key == null) key = test.find(window.getRoot(), test.key("a"));
            if (editor == null) editor = test.find(window.getRoot(), test.field("msime-test-copy-source"));
        }
        if (key == null || !letter.equals(bounds(key))) throw new AssertionError("复制提示移动了按键区");
        if (editor == null || !editor.isFocused()) throw new AssertionError("复制提示抢走了编辑器焦点");
    }

    private Rect keyboardBounds() {
        for (var window : test.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect result = new Rect();
            window.getBoundsInScreen(result);
            if (!result.isEmpty()) return result;
        }
        throw new AssertionError("复制提示期间键盘窗口消失");
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect();
        node.getBoundsInScreen(result);
        return result;
    }

    private float density() { return test.getTargetContext().getResources().getDisplayMetrics().density; }

    @Override public void close() { test.automation.setOnAccessibilityEventListener(null); }
}

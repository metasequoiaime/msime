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
import java.nio.file.Files;
import org.json.JSONArray;
import org.json.JSONObject;

/** 合成识别服务经真实输入法返回空结果，验证顶部提示、工具栏与窗口稳定性。 */
final class VoiceEmptyNoticeDeviceChecks {
    private final DeviceSmoke t;
    private String scenario;
    private String messagePrefix;
    private volatile boolean legacyMessage;
    private Rect keyboard;
    private Rect letter;
    private Rect toolbar;

    VoiceEmptyNoticeDeviceChecks(DeviceSmoke test, String selected) {
        t = test;
        scenario = selected;
        messagePrefix = switch (scenario) {
            case "timeout" -> "没有听到声音";
            case "no-match" -> "没有识别出文字";
            case "empty-result" -> "语音识别服务没有返回文字";
            default -> throw new IllegalArgumentException("未知合成空结果场景");
        };
    }

    void run() throws Exception {
        File preferences = t.sharedPreferencesFile();
        byte[] original = preferences.exists() ? Files.readAllBytes(preferences.toPath()) : null;
        t.automation.setOnAccessibilityEventListener(event -> {
            if (event.getEventType() == AccessibilityEvent.TYPE_NOTIFICATION_STATE_CHANGED
                    && DeviceSmoke.equalsText("app.msime.android", event.getPackageName())) {
                for (CharSequence text : event.getText())
                    if (text != null && text.toString().startsWith(messagePrefix)) legacyMessage = true;
            }
        });
        try {
            t.putSharedPreference("scheme", "quanpin");
            t.putSharedPreference("touch_keyboard_layout", "twenty_six_key");
            t.putSharedPreference("touch_keyboard_schemes", new JSONObject()
                .put("enabled", new JSONArray().put("quanpin")).put("selected", "quanpin"));
            t.putSharedPreference("touch_voice_shortcut", true);
            JSONObject voice = new JSONObject(new String(Files.readAllBytes(preferences.toPath()),
                java.nio.charset.StandardCharsets.UTF_8)).getJSONObject("preferences").getJSONObject("voice_input");
            voice.put("asr_provider", "system").put("asr_model_path", "").put("polish_enabled", false);
            t.putSharedPreference("voice_input", voice);
            t.restartIme();
            t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
            t.tap(t.field("msime-test-plain"));
            letter = bounds(t.awaitStableBounds(t.key("a")));
            toolbar = bounds(t.awaitStableBounds(t.described("可滑动键盘工具")));
            keyboard = keyboardBounds();
            t.stage = "real IME empty voice callback: " + scenario;
            // 通过真实工具栏的无障碍点击启动；不要求语音图标恰好处于横向滚动区当前可见的一页。
            if (!t.await(t.described("语音输入")).performAction(AccessibilityNodeInfo.ACTION_CLICK))
                throw new AssertionError("工具栏语音入口拒绝点击");
            long deadline = SystemClock.uptimeMillis() + 15000;
            boolean captured = false;
            do {
                assertKeyboardBounds();
                Bitmap screenshot = t.automation.takeScreenshot();
                if (screenshot == null) throw new AssertionError("空语音提示截图不可用");
                try {
                    Rect notice = noticeBounds(screenshot);
                    if (notice != null) {
                        stable();
                        float density = t.getTargetContext().getResources().getDisplayMetrics().density;
                        if (notice.isEmpty() || notice.top < 0 || notice.bottom > screenshot.getHeight() / 4
                                || notice.bottom >= keyboard.top
                                || Math.abs(notice.centerX() - screenshot.getWidth() / 2) > 4 * density)
                            throw new AssertionError("空语音提示不在手机屏幕顶部中央：" + notice);
                        File file = new File(t.getTargetContext().getExternalFilesDir(null), "voice-empty-" + scenario + ".png");
                        try (FileOutputStream output = new FileOutputStream(file)) {
                            if (!screenshot.compress(Bitmap.CompressFormat.PNG, 100, output))
                                throw new AssertionError("空语音提示截图写入失败");
                        }
                        captured = true;
                        break;
                    }
                } finally { screenshot.recycle(); }
                SystemClock.sleep(50);
            } while (SystemClock.uptimeMillis() < deadline);
            if (!captured) throw new AssertionError("空语音提示没有在屏幕顶部出现");
            deadline = SystemClock.uptimeMillis() + 8000;
            long goneAt = 0;
            do {
                stable();
                Bitmap screenshot = t.automation.takeScreenshot();
                if (screenshot == null) throw new AssertionError("空语音提示截图不可用");
                boolean visible;
                try { visible = noticeBounds(screenshot) != null; }
                finally { screenshot.recycle(); }
                if (visible) goneAt = 0;
                else if (goneAt == 0) goneAt = SystemClock.uptimeMillis();
                else if (SystemClock.uptimeMillis() - goneAt >= 500) return;
                SystemClock.sleep(50);
            } while (SystemClock.uptimeMillis() < deadline);
            throw new AssertionError("空语音提示没有自动消失");
        } catch (Exception | AssertionError failure) {
            // 在恢复偏好和重启输入法之前留下合成窗口现场，不能只保留清理后的首页。
            Bitmap screenshot = t.automation.takeScreenshot();
            if (screenshot != null) {
                try (FileOutputStream output = new FileOutputStream(new File(
                        t.getTargetContext().getExternalFilesDir(null), "voice-empty-" + scenario + "-failure.png"))) {
                    if (!screenshot.compress(Bitmap.CompressFormat.PNG, 100, output))
                        throw new AssertionError("空语音失败截图写入失败");
                } catch (Exception | AssertionError captureFailure) { failure.addSuppressed(captureFailure); }
                finally { screenshot.recycle(); }
            }
            throw failure;
        } finally {
            t.automation.setOnAccessibilityEventListener(null);
            if (original == null) Files.deleteIfExists(preferences.toPath());
            else Files.write(preferences.toPath(), original);
            t.restartIme();
        }
    }

    private Rect noticeBounds(Bitmap screenshot) {
        if (Build.VERSION.SDK_INT < 30)
            return legacyMessage ? ClipboardPhraseNoticeDeviceCheck.legacyNoticeBounds(screenshot) : null;
        for (var window : t.automation.getWindows()) {
            if (!DeviceSmoke.equalsText("MSIME empty voice notice", window.getTitle())) continue;
            AccessibilityNodeInfo node = t.find(window.getRoot(), t.describedPrefix("空语音提示：" + messagePrefix));
            if (node != null) return bounds(node);
        }
        return null;
    }

    private void stable() {
        t.dropStaleNodes();
        assertKeyboardBounds();
        AccessibilityNodeInfo key = null, tools = null, editor = null;
        for (var window : t.automation.getWindows()) {
            if (key == null) key = t.find(window.getRoot(), t.key("a"));
            if (tools == null) tools = t.find(window.getRoot(), t.described("可滑动键盘工具"));
            if (editor == null) editor = t.find(window.getRoot(), t.field("msime-test-plain"));
        }
        if (key == null || !letter.equals(bounds(key))) throw new AssertionError("空语音提示移动了按键区");
        if (tools == null || !toolbar.equals(bounds(tools))) throw new AssertionError("空语音提示改变了工具栏范围");
        if (editor == null || !editor.isFocused()) throw new AssertionError("空语音提示抢走了编辑器焦点");
    }

    private void assertKeyboardBounds() {
        Rect actual = keyboardBounds();
        if (!keyboard.equals(actual)) throw new AssertionError("空语音改变了输入法窗口范围：expected="
            + keyboard + ", actual=" + actual);
    }

    private Rect keyboardBounds() {
        for (var window : t.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect result = new Rect();
            window.getBoundsInScreen(result);
            if (!result.isEmpty()) return result;
        }
        throw new AssertionError("空语音提示期间键盘窗口消失");
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect result = new Rect();
        node.getBoundsInScreen(result);
        return result;
    }
}

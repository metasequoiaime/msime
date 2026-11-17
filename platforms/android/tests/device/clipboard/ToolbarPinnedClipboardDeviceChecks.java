package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.io.File;
import java.nio.charset.StandardCharsets;
import org.json.JSONObject;

/** #11427：从真实系统复制预览点按常驻工具，保持原套件默认配置的回归路径。 */
final class ToolbarPinnedClipboardDeviceChecks {
    private static final String TEXT = "synthetic-pinned-copy";
    private static final String PREVIEW = "粘贴最近复制的内容：" + TEXT;
    private final DeviceSmoke test;

    ToolbarPinnedClipboardDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run(File local, JSONObject settings) throws Exception {
        for (int size : new int[] {90, 150}) {
            test.stage = "pinned clipboard prepare " + size;
            settings.getJSONObject("settings").put("platform.android.toolbar_pinned", "text_edit")
                .put("platform.android.toolbar_size_percent", size);
            test.publish(local, settings.toString().getBytes(StandardCharsets.UTF_8));
            test.restartIme();
            test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity --ez msime_test_multiline true");
            test.tap(test.field("msime-test-plain"));
            test.await(test.key("n"));
            Bundle text = new Bundle();
            text.putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE, TEXT);
            if (!test.await(test.field("msime-test-plain")).performAction(AccessibilityNodeInfo.ACTION_SET_TEXT, text))
                throw new AssertionError("Synthetic editor text setup failed");
            test.automation.waitForIdle(500, 5000);
            SystemClock.sleep(1500);
            if (visible("最近复制")) click("不再显示这条复制的内容");
            click("文本编辑");
            test.await(test.described("文本编辑面板"));
            click("全选");
            click("复制");
            test.await(test.described(PREVIEW));
            click("返回键盘");
            test.await(test.key("n"));
            test.await(test.described(PREVIEW));

            test.stage = "pinned clipboard click " + size;
            Rect before = keyboardBounds();
            Rect preview = bounds(test.awaitStableBounds(test.described(PREVIEW)));
            Rect pin = bounds(test.awaitStableBounds(test.described("文本编辑")));
            if (pin.left < preview.right || pin.centerY() < preview.top || pin.centerY() > preview.bottom)
                throw new AssertionError("Pinned tool is not on the clipboard row's right edge");
            test.tap(test.described("文本编辑").and(AccessibilityNodeInfo::isClickable));
            test.await(test.described("文本编辑面板"));
            test.await(test.described(PREVIEW));
            if (!before.equals(keyboardBounds()))
                throw new AssertionError("Opening the pinned text editor moved the keyboard window");
            expectText();
            click("返回键盘");
            test.await(test.key("n"));
            click("不再显示这条复制的内容");
            if (visible("最近复制")) throw new AssertionError("Clipboard dismissal left its preview visible");
            click("文本编辑");
            test.await(test.described("文本编辑面板"));
            expectText();
        }
    }

    private void expectText() {
        test.await(test.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(TEXT, node.getText())));
    }

    private void click(String description) throws Exception {
        test.dropStaleNodes();
        test.tap(test.described(description).and(AccessibilityNodeInfo::isClickable).and(AccessibilityNodeInfo::isEnabled));
    }

    private boolean visible(String description) {
        test.dropStaleNodes();
        for (AccessibilityWindowInfo window : test.automation.getWindows())
            if (test.find(window.getRoot(), test.described(description)) != null) return true;
        return false;
    }

    private Rect keyboardBounds() {
        for (AccessibilityWindowInfo window : test.automation.getWindows()) {
            if (window.getType() != AccessibilityWindowInfo.TYPE_INPUT_METHOD) continue;
            Rect bounds = new Rect();
            window.getBoundsInScreen(bounds);
            if (!bounds.isEmpty()) return bounds;
        }
        throw new AssertionError("Input method window is missing");
    }

    private static Rect bounds(AccessibilityNodeInfo node) {
        Rect bounds = new Rect();
        node.getBoundsInScreen(bounds);
        return bounds;
    }
}

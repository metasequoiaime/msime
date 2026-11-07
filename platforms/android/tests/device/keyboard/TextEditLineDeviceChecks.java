package app.msime.android.test;

import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;

/** 真实编辑器里的文档首尾、行级上滑、Shift 选择与换行，不依赖宿主内部实现。 */
final class TextEditLineDeviceChecks {
    private final DeviceSmoke t;

    TextEditLineDeviceChecks(DeviceSmoke test) { t = test; }

    void run() throws Exception {
        open("edit");
        t.stage = "document boundaries";
        setSelection(16);
        tap("移到开头");
        selection(0, 0);
        tap("移到结尾");
        selection(TrackpadEditorActivity.TEXT.length(), TrackpadEditorActivity.TEXT.length());
        t.stage = "upward swipes preserve line actions";
        setSelection(16);
        swipeUp("移到开头");
        selection(11, 11);
        setSelection(16);
        swipeUp("移到结尾");
        selection(22, 22);
        setSelection(16);
        swipeUp("全选");
        selection(11, 22);
        t.stage = "document boundaries extend selection";
        setSelection(16);
        tap("选择模式");
        tap("移到开头");
        selection(0, 16);
        tap("取消选择");
        setSelection(16);
        tap("选择模式");
        tap("移到结尾");
        selection(16, TrackpadEditorActivity.TEXT.length());
        tap("取消选择");
        t.stage = "document boundaries stay usable in trackpad mode";
        tap("触控板");
        t.await(t.described("触控板").and(AccessibilityNodeInfo::isChecked));
        setSelection(16);
        tap("移到开头");
        selection(0, 0);
        tap("移到结尾");
        selection(TrackpadEditorActivity.TEXT.length(), TrackpadEditorActivity.TEXT.length());
        tap("触控板");
        t.await(t.described("触控板").and(node -> !node.isChecked()));

        open("terminal");
        t.stage = "document boundaries reach a terminal without surrounding text";
        // 文档首尾带 Ctrl（META_CTRL_ON | META_CTRL_LEFT_ON = 12288）；选择模式再加 Shift（65）。
        tap("移到开头");
        t.await(t.field("trackpad-status").and(node -> DeviceSmoke.equalsText("0:122:12288,1:122:12288", node.getText())));
        tap("选择模式");
        tap("移到结尾");
        t.await(t.field("trackpad-status").and(node -> node.getText() != null
            && node.getText().toString().contains("0:123:12353,1:123:12353")));

        open("edit");
        t.stage = "return short tap inserts exactly one newline";
        setSelection(TrackpadEditorActivity.TEXT.length());
        Rect key = returnBounds();
        gesture(key, false);
        text(TrackpadEditorActivity.TEXT + "\n");
        t.stage = "return slide out cancels the newline";
        key = returnBounds();
        gesture(key, true);
        text(TrackpadEditorActivity.TEXT + "\n");
        if (visible("符号面板")) throw new AssertionError("Sliding out opened symbols");
        // 换行键只管换行：长按不打开符号面板，松手照常换一行，面板留着。
        t.stage = "return long press only inserts one newline";
        key = returnBounds();
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        SystemClock.sleep(android.view.ViewConfiguration.getLongPressTimeout() * 2L);
        touch(down, MotionEvent.ACTION_UP, key.centerX(), key.centerY());
        t.automation.waitForIdle(300, 5000);
        text(TrackpadEditorActivity.TEXT + "\n\n");
        if (visible("符号面板")) throw new AssertionError("Return long press opened symbols");
        if (!visible("文本编辑面板")) throw new AssertionError("Return long press closed the text edit panel");
    }

    private void open(String kind) throws Exception {
        t.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/"
            + "app.msime.android.test.TrackpadEditorActivity --es kind " + kind);
        t.dropStaleNodes();
        t.await(t.field("trackpad-editor"));
        t.await(t.key("n"));
        tap("文本编辑");
        t.await(t.described("文本编辑面板"));
    }

    private void tap(String description) throws Exception {
        t.dropStaleNodes();
        t.tap(t.described(description).and(AccessibilityNodeInfo::isClickable));
    }

    private void setSelection(int at) {
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, at);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, at);
        if (!t.await(t.field("trackpad-editor")).performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args))
            throw new AssertionError("Synthetic cursor setup failed");
        selection(at, at);
    }

    private void selection(int start, int end) {
        t.await(t.field("trackpad-editor").and(node ->
            Math.min(node.getTextSelectionStart(), node.getTextSelectionEnd()) == start
                && Math.max(node.getTextSelectionStart(), node.getTextSelectionEnd()) == end));
    }

    private void text(String expected) {
        t.await(t.field("trackpad-editor").and(node -> DeviceSmoke.equalsText(expected, node.getText())));
    }

    private Rect returnBounds() {
        Rect bounds = new Rect();
        t.awaitStableBounds(t.describedPrefix("插入换行")).getBoundsInScreen(bounds);
        return bounds;
    }

    private void swipeUp(String description) throws Exception {
        Rect key = new Rect();
        t.awaitStableBounds(t.described(description)).getBoundsInScreen(key);
        float distance = 40 * t.getTargetContext().getResources().getDisplayMetrics().density;
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        touch(down, MotionEvent.ACTION_MOVE, key.centerX(), key.centerY() - distance);
        touch(down, MotionEvent.ACTION_UP, key.centerX(), key.centerY() - distance);
        t.automation.waitForIdle(300, 5000);
    }

    private void gesture(Rect key, boolean cancel) throws Exception {
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, key.centerX(), key.centerY());
        float x = cancel ? key.left - key.width() : key.centerX();
        if (cancel) touch(down, MotionEvent.ACTION_MOVE, x, key.centerY());
        touch(down, MotionEvent.ACTION_UP, x, key.centerY());
        t.automation.waitForIdle(300, 5000);
    }

    private void touch(long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.automation.injectInputEvent(event, false)) throw new AssertionError("Synthetic touch rejected");
        } finally { event.recycle(); }
    }

    private boolean visible(String description) {
        t.dropStaleNodes();
        for (AccessibilityWindowInfo window : t.automation.getWindows())
            if (t.find(window.getRoot(), t.described(description)) != null) return true;
        return false;
    }
}

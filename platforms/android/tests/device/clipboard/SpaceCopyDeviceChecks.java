package app.msime.android.test;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.graphics.Rect;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;

/** 真实系统触摸和 InputConnection 验证上滑复制，合成剪贴板内容不会进入诊断输出。 */
final class SpaceCopyDeviceChecks {
    private static final String TEXT = "合成复制😀甲乙\n第二行";
    private static final String SENTINEL = "synthetic-space-copy-sentinel";
    private static final String SOURCE = "msime-test-copy-source";
    private final DeviceSmoke test;
    private ClipboardManager clipboard;
    private Rect space;
    private long down;
    private float x;
    private float y;
    private boolean pressed;

    SpaceCopyDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run(boolean preview) throws Exception {
        clipboard = test.getTargetContext().getSystemService(ClipboardManager.class);
        try { checks(preview); }
        finally { if (pressed) release(true); }
    }

    private void checks(boolean preview) throws Exception {
        test.stage = "space copy open synthetic editor";
        test.restartIme();
        open("");
        seed();
        test.stage = "space copy upward waits for release";
        press();
        move(-36);
        expectClipboard(SENTINEL);
        test.await(test.described("松开复制全文"));
        SystemClock.sleep(500);
        release(false);
        expectClipboard(TEXT);
        expectSelection(SOURCE, 3, 3);
        test.stage = "space copy preview follows existing preference";
        if (preview) test.await(test.described("粘贴最近复制的内容：合成复制😀甲乙 第二行"));
        else absentPreview();

        test.stage = "space copy editor rejection cannot fall back to direct clipboard writes";
        open("--ez reject-copy true");
        seed();
        swipe();
        expectClipboard(SENTINEL);
        if (copyAttempts() != 1) throw new AssertionError("Editor did not receive the copy request");
        expectSelection(SOURCE, 3, 3);

        test.stage = "space short tap still inserts a space";
        expectSelection(SOURCE, 3, 3);
        press();
        release(false);
        test.await(test.field(SOURCE).and(node -> DeviceSmoke.equalsText(
            TEXT.substring(0, 3) + " " + TEXT.substring(3), node.getText())));
        open("");
        seed();
        test.stage = "space horizontal swipe still moves the caret";
        press();
        x += 36 * test.getTargetContext().getResources().getDisplayMetrics().density;
        touch(MotionEvent.ACTION_MOVE);
        release(false);
        test.await(test.field(SOURCE).and(node -> node.getTextSelectionStart() > 3
            && node.getTextSelectionStart() == node.getTextSelectionEnd()
            && DeviceSmoke.equalsText(TEXT, node.getText())));
        expectClipboard(SENTINEL);
        test.stage = "space copy preserves a selected range";
        open("--ei selection-start 1 --ei selection-end 4");
        seed();
        swipe();
        expectClipboard(TEXT);
        expectSelection(SOURCE, 1, 4);
        test.stage = "space copy slid back cancels without typing space";
        open("");
        seed();
        press();
        move(-36);
        move(0);
        release(false);
        expectClipboard(SENTINEL);
        test.await(test.field(SOURCE).and(node -> DeviceSmoke.equalsText(TEXT, node.getText())));
        test.stage = "space copy system cancellation does not copy";
        press();
        move(-36);
        release(true);
        expectClipboard(SENTINEL);
        test.stage = "space copy follows the current input field";
        test.tap(test.field("msime-test-copy-other"));
        SystemClock.sleep(400);
        swipe();
        expectClipboard("另一个合成输入框");

        test.stage = "space copy rejected select-all cannot copy a partial range";
        open("--ez reject-all true");
        seed();
        swipe();
        expectClipboard(SENTINEL);
        if (copyAttempts() != 0) throw new AssertionError("Copy ran despite rejected select-all");
        test.stage = "space copy unknown location does not modify editor selection";
        open("--ez unknown-position true");
        seed();
        swipe();
        expectClipboard(SENTINEL);
        expectSelection(SOURCE, 3, 3);
        test.stage = "space copy password field never receives copy";
        open("--ez password true");
        seed();
        swipe();
        expectClipboard(SENTINEL);
        if (copyAttempts() != 0) throw new AssertionError("Password field received copy request");
        test.stage = "space copy WebView textarea keeps text and caret";
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.SpaceCopyEditorActivity --ez webview true");
        test.tap(editor("msime-test-copy-web"));
        test.await(test.key("空格"));
        SystemClock.sleep(600);
        select("msime-test-copy-web", 3, 3);
        seed();
        swipe();
        expectClipboard(TEXT);
        expectSelection("msime-test-copy-web", 3, 3);
    }

    private java.util.function.Predicate<AccessibilityNodeInfo> editor(String field) {
        if (!field.equals("msime-test-copy-web")) return test.field(field);
        // 旧 WebView 不把 textarea 的 aria-label 暴露为 contentDescription；夹具只有一个可编辑节点。
        return node -> DeviceSmoke.equalsText("app.msime.android.test", node.getPackageName()) && node.isEditable();
    }

    private void open(String extras) throws Exception {
        test.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.SpaceCopyEditorActivity " + extras);
        test.await(test.field(SOURCE).and(AccessibilityNodeInfo::isFocused));
        test.await(test.key("空格"));
        SystemClock.sleep(500);
    }

    private void select(String field, int start, int end) {
        test.dropStaleNodes();
        AccessibilityNodeInfo editor = test.await(editor(field));
        Bundle args = new Bundle();
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_START_INT, start);
        args.putInt(AccessibilityNodeInfo.ACTION_ARGUMENT_SELECTION_END_INT, end);
        // TextView 对已经处于目标位置的请求也会返回 false，按读回的位置判断结果。
        editor.performAction(AccessibilityNodeInfo.ACTION_SET_SELECTION, args);
        SystemClock.sleep(200);
        expectSelection(field, start, end);
    }

    private void expectSelection(String field, int start, int end) {
        long until = SystemClock.uptimeMillis() + 5000;
        int actualStart = -1, actualEnd = -1;
        do {
            AccessibilityNodeInfo editor = test.await(editor(field));
            editor.refresh();
            actualStart = editor.getTextSelectionStart();
            actualEnd = editor.getTextSelectionEnd();
            if (actualStart == start && actualEnd == end) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < until);
        throw new AssertionError("Synthetic selection mismatch: " + actualStart + ":" + actualEnd
            + " expected " + start + ":" + end);
    }

    private int copyAttempts() {
        AccessibilityNodeInfo node = test.await(test.field(SOURCE));
        node.refresh();
        return node.getExtras().getInt("synthetic-copy-attempts", -1);
    }

    private void seed() {
        clipboard.setPrimaryClip(ClipData.newPlainText("synthetic", SENTINEL));
        SystemClock.sleep(200);
    }

    private void expectClipboard(String expected) {
        long until = SystemClock.uptimeMillis() + 4000;
        do {
            ClipData clip = clipboard.getPrimaryClip();
            if (clip != null && clip.getItemCount() == 1 && expected.contentEquals(clip.getItemAt(0).getText())) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < until);
        throw new AssertionError("Synthetic clipboard did not match expected result");
    }

    private void absentPreview() {
        SystemClock.sleep(400);
        for (var window : test.automation.getWindows())
            if (test.find(window.getRoot(), test.describedPrefix("粘贴最近复制的内容：")) != null)
                throw new AssertionError("Clipboard preview ignored disabled preference");
    }

    private void press() {
        space = new Rect();
        test.awaitStableBounds(test.key("空格")).getBoundsInScreen(space);
        down = SystemClock.uptimeMillis();
        x = space.centerX();
        y = space.centerY();
        touch(MotionEvent.ACTION_DOWN);
        pressed = true;
    }

    private void move(float dp) {
        y = space.centerY() + dp * test.getTargetContext().getResources().getDisplayMetrics().density;
        touch(MotionEvent.ACTION_MOVE);
    }

    private void release(boolean cancel) {
        try { touch(cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP); }
        finally { pressed = false; }
        SystemClock.sleep(300);
    }

    private void swipe() { press(); move(-36); release(false); }

    private void touch(int action) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!test.automation.injectInputEvent(event, true)) throw new AssertionError("Touch injection rejected");
        } finally { event.recycle(); }
    }
}

package app.msime.android.test;

import android.graphics.Rect;
import android.content.Intent;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;

/** 在合成编辑器中通过系统触摸执行清除、下滑撤回、滑回取消和输入框切换。 */
public final class QuickDeleteUndoDeviceSmoke extends DeviceSmoke {
    private boolean scrubOnly;
    private QuickDeleteEditorActivity fixture;
    private static final String EDITOR = "msime-test-plain";

    @Override public void onCreate(android.os.Bundle arguments) {
        scrubOnly = arguments != null && "true".equals(arguments.getString("scrub-only"));
        super.onCreate(arguments);
    }

    @Override protected String successDescription() {
        return scrubOnly ? "horizontal backspace delete/restore within one held gesture and editor boundaries"
            : "quick delete, held downward undo, horizontal scrub, cancel, short delete and editor boundary";
    }

    @Override protected void runChecks() throws Exception {
        if (scrubOnly) {
            new BackspaceScrubDeviceChecks(this).run();
            return;
        }
        stage = "select installed input method";
        restartIme();
        stage = "open synthetic quick-delete editor";
        Intent intent = new Intent(getTargetContext(), QuickDeleteEditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        fixture = (QuickDeleteEditorActivity) startActivitySync(intent);
        stage = "focus synthetic quick-delete editor";
        tap(field(EDITOR));
        stage = "wait for delete key";
        await(key("⌫"));
        stage = "quick delete leaves following text";
        setup("合成😀内容X尾", 7);
        quickDelete();
        expect("尾");
        stage = "held undo sliding back does nothing";
        undo(true, false);
        expect("尾");
        stage = "cancelled undo does nothing";
        undo(false, true);
        expect("尾");
        stage = "editor rejects undo without consuming backup";
        runOnMainSync(() -> fixture.rejectWrites = true);
        undo(false, false);
        expect("尾");
        runOnMainSync(() -> fixture.rejectWrites = false);
        stage = "held downward undo restores deleted prefix";
        undo(false, false);
        // 按下即删的 X 属于既有短按行为，撤回恢复的是之后快速清除的部分。
        expect("合成😀内容尾");
        stage = "multi-chunk deletion restores Unicode text in order";
        String large = "多段😀甲".repeat(1800);
        setup(large + "X尾", large.length() + 1);
        quickDelete();
        expect("尾");
        undo(false, false);
        expect(large + "尾");
        stage = "quick-delete backup survives short press until release";
        setup("合成文本X", 5);
        quickDelete();
        expect("");
        stage = "short press invalidates quick-delete backup";
        Rect delete = deleteBounds();
        long pressed = SystemClock.uptimeMillis();
        touch(pressed, MotionEvent.ACTION_DOWN, delete.centerX(), delete.centerY());
        SystemClock.sleep(40);
        touch(pressed, MotionEvent.ACTION_UP, delete.centerX(), delete.centerY());
        SystemClock.sleep(250);
        undo(false, false);
        stage = "short press cannot restore the old backup";
        expect("");
        stage = "editor switch invalidates backup";
        setup("合成文本X", 5);
        quickDelete();
        expect("");
        tap(field("msime-test-other"));
        tap(field(EDITOR));
        undo(false, false);
        expect("");
        stage = "unavailable position disables undo but keeps quick deletion";
        runOnMainSync(() -> fixture.unknownPosition = true);
        setup("合成文本X", 5);
        quickDelete();
        expect("");
        undo(false, false);
        expect("");
        runOnMainSync(() -> fixture.unknownPosition = false);
        stage = "new text at the same cursor invalidates backup";
        setup("合成文本X", 5);
        quickDelete();
        expect("");
        setup("尾", 0);
        undo(false, false);
        expect("尾");
        stage = "no-learning editor never retains deleted text";
        runOnMainSync(() -> {
            fixture.editor.setImeOptions(android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING);
            android.view.inputmethod.InputMethodManager manager = (android.view.inputmethod.InputMethodManager)
                fixture.getSystemService(android.content.Context.INPUT_METHOD_SERVICE);
            manager.restartInput(fixture.editor);
        });
        SystemClock.sleep(400);
        setup("合成文本X", 5);
        quickDelete();
        expect("");
        int accepted = fixture.acceptedWrites;
        undo(false, false);
        expect("");
        if (fixture.acceptedWrites != accepted) throw new AssertionError("Private editor unexpectedly restored text");
        new BackspaceScrubDeviceChecks(this).run();
        stage = "normalize Chinese mode before ordinary input regression";
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        tap(field(EDITOR));
        await(key("⌫"));
        for (var window : automation.getWindows()) {
            if (find(window.getRoot(), key("英")) != null) {
                tap(key("英"));
                break;
            }
        }
        super.runChecks();
    }

    private void setup(String text, int cursor) {
        // 合成文本只通过测试拥有的控件设置；产品操作仍由系统触摸和 InputConnection 完成。
        runOnMainSync(() -> {
            fixture.editor.setText(text);
            fixture.editor.setSelection(cursor);
        });
        SystemClock.sleep(250);
    }

    private void expect(String text) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        boolean[] matches = new boolean[1];
        int[] state = new int[3];
        do {
            runOnMainSync(() -> {
                matches[0] = text.contentEquals(fixture.editor.getText());
                state[0] = fixture.editor.length();
                state[1] = fixture.editor.getSelectionStart();
                state[2] = fixture.acceptedWrites;
            });
            if (matches[0]) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic editor length=" + state[0] + ";cursor=" + state[1]
            + ";accepted=" + state[2] + ";expected-length=" + text.length());
    }

    private Rect deleteBounds() {
        Rect bounds = new Rect();
        awaitStableBounds(key("⌫")).getBoundsInScreen(bounds);
        return bounds;
    }

    private void quickDelete() {
        Rect bounds = deleteBounds();
        float density = getContext().getResources().getDisplayMetrics().density;
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        touch(down, MotionEvent.ACTION_MOVE, bounds.centerX(), bounds.top - 42 * density);
        touch(down, MotionEvent.ACTION_UP, bounds.centerX(), bounds.top - 42 * density);
        SystemClock.sleep(300);
    }

    private void undo(boolean disarm, boolean cancel) {
        Rect bounds = deleteBounds();
        float density = getContext().getResources().getDisplayMetrics().density;
        long down = SystemClock.uptimeMillis();
        touch(down, MotionEvent.ACTION_DOWN, bounds.centerX(), bounds.centerY());
        SystemClock.sleep(480);
        float y = bounds.bottom + 18 * density;
        touch(down, MotionEvent.ACTION_MOVE, bounds.centerX(), y);
        if (disarm) {
            y = bounds.centerY();
            touch(down, MotionEvent.ACTION_MOVE, bounds.centerX(), y);
        }
        touch(down, cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP, bounds.centerX(), y);
        SystemClock.sleep(300);
    }

    private void touch(long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!automation.injectInputEvent(event, true)) throw new AssertionError("Touch injection rejected");
        } finally { event.recycle(); }
    }
}

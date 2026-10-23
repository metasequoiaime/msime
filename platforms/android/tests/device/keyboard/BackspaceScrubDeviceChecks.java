package app.msime.android.test;

import android.content.Intent;
import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;

/** 复用撤回套件的系统触摸入口，验收同一未松手手势中的删除、恢复与失效。 */
final class BackspaceScrubDeviceChecks {
    private final DeviceSmoke test;
    private QuickDeleteEditorActivity fixture;
    private Rect bounds;
    private long down;
    private float x;
    private static final String PREFIX = "合成😀甲乙丙丁戊己庚辛壬癸";
    private static final String ORIGINAL = PREFIX + "尾";

    BackspaceScrubDeviceChecks(DeviceSmoke test) { this.test = test; }

    void run() throws Exception {
        test.stage = "scrub open synthetic editor";
        test.restartIme();
        Intent intent = new Intent(test.getTargetContext(), QuickDeleteEditorActivity.class);
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_CLEAR_TASK);
        fixture = (QuickDeleteEditorActivity) test.startActivitySync(intent);
        test.tap(test.field("msime-test-plain"));
        test.await(test.key("⌫"));
        test.stage = "scrub down still deletes immediately";
        setup(ORIGINAL, PREFIX.length());
        press();
        expect(PREFIX.substring(0, PREFIX.length() - 1) + "尾");
        test.stage = "scrub left removes multiple codepoints and keeps following text";
        move(.72f);
        int left = length();
        if (left >= PREFIX.length() - 2 || !content().endsWith("尾"))
            throw new AssertionError("Horizontal gesture did not delete multiple synthetic codepoints");
        test.stage = "scrub right progressively restores";
        move(.35f);
        if (length() <= left || length() >= ORIGINAL.length())
            throw new AssertionError("Rightward gesture did not progressively restore");
        test.stage = "scrub origin restores full Unicode prefix";
        move(0);
        expect(ORIGINAL);
        release(false);
        test.stage = "scrub release drops restoration state";
        press();
        move(.72f);
        release(false);
        String deleted = content();
        press();
        touch(MotionEvent.ACTION_MOVE, bounds.centerX() + 20);
        release(false);
        expect(deleted.substring(0, deleted.length() - 2) + "尾");

        test.stage = "scrub held repeat joins the same reversible gesture";
        setup(ORIGINAL, PREFIX.length());
        press();
        SystemClock.sleep(650);
        move(.65f);
        move(0);
        expect(ORIGINAL);
        release(false);
        test.stage = "scrub cancel keeps completed deletion";
        press();
        move(.7f);
        deleted = content();
        release(true);
        expect(deleted);

        test.stage = "scrub external same-position text change disables restore";
        setup(ORIGINAL, PREFIX.length());
        press();
        move(.65f);
        int position = cursor();
        String changed = "改".repeat(position) + "尾";
        setup(changed, position);
        move(0);
        release(false);
        expect(changed);
        test.stage = "scrub external selection change disables restore";
        setup(ORIGINAL, PREFIX.length());
        press();
        move(.65f);
        deleted = content();
        test.runOnMainSync(() -> fixture.editor.setSelection(0));
        SystemClock.sleep(150);
        move(0);
        release(false);
        expect(deleted);

        test.stage = "scrub rejected restore cannot be retried in the same gesture";
        setup(ORIGINAL, PREFIX.length());
        press();
        move(.65f);
        deleted = content();
        fixture.rejectWrites = true;
        move(0);
        expect(deleted);
        fixture.rejectWrites = false;
        move(.2f);
        move(0);
        release(false);
        expect(deleted);

        test.stage = "scrub selected text keeps ordinary selection deletion";
        setup(ORIGINAL, PREFIX.length());
        test.runOnMainSync(() -> fixture.editor.setSelection(4, PREFIX.length()));
        SystemClock.sleep(150);
        press();
        move(.7f);
        move(0);
        release(false);
        expect("合成😀尾");

        test.stage = "scrub unknown position falls back to ordinary backspace";
        fixture.unknownPosition = true;
        setup(ORIGINAL, PREFIX.length());
        press();
        move(.7f);
        move(0);
        release(false);
        expect(PREFIX.substring(0, PREFIX.length() - 1) + "尾");
        fixture.unknownPosition = false;

        test.stage = "scrub password editor never restores";
        test.runOnMainSync(() -> {
            fixture.editor.setInputType(android.text.InputType.TYPE_CLASS_TEXT
                | android.text.InputType.TYPE_TEXT_VARIATION_PASSWORD);
            fixture.getSystemService(android.view.inputmethod.InputMethodManager.class).restartInput(fixture.editor);
        });
        SystemClock.sleep(500);
        setup(ORIGINAL, PREFIX.length());
        press();
        move(.7f);
        move(0);
        release(false);
        expect(PREFIX.substring(0, PREFIX.length() - 1) + "尾");
        test.stage = "scrub no-learning editor never restores";
        test.runOnMainSync(() -> {
            fixture.editor.setInputType(android.text.InputType.TYPE_CLASS_TEXT
                | android.text.InputType.TYPE_TEXT_FLAG_MULTI_LINE);
            fixture.editor.setImeOptions(android.view.inputmethod.EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING);
            fixture.getSystemService(android.view.inputmethod.InputMethodManager.class).restartInput(fixture.editor);
        });
        SystemClock.sleep(500);
        setup(ORIGINAL, PREFIX.length());
        press();
        move(.7f);
        move(0);
        release(false);
        expect(PREFIX.substring(0, PREFIX.length() - 1) + "尾");
    }

    private void setup(String text, int cursor) {
        test.runOnMainSync(() -> {
            fixture.editor.setText(text);
            fixture.editor.setSelection(cursor);
        });
        SystemClock.sleep(300);
    }

    private String content() {
        String[] value = new String[1];
        test.runOnMainSync(() -> value[0] = fixture.editor.getText().toString());
        return value[0];
    }

    private int length() { return content().length(); }

    private int cursor() {
        int[] value = new int[1];
        test.runOnMainSync(() -> value[0] = fixture.editor.getSelectionStart());
        return value[0];
    }

    private void expect(String expected) {
        long deadline = SystemClock.uptimeMillis() + 4000;
        do {
            if (expected.equals(content())) return;
            SystemClock.sleep(50);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("Synthetic scrub length=" + length() + "; expected=" + expected.length());
    }

    private void press() {
        bounds = new Rect();
        test.awaitStableBounds(test.key("⌫")).getBoundsInScreen(bounds);
        down = SystemClock.uptimeMillis();
        x = bounds.centerX();
        touch(MotionEvent.ACTION_DOWN, x);
        SystemClock.sleep(50);
    }

    private void move(float fraction) {
        // 真实键到左缘的距离决定比例；右滑回到原按下点。
        x = Math.max(4, bounds.centerX() * (1 - fraction));
        touch(MotionEvent.ACTION_MOVE, x);
        SystemClock.sleep(100);
    }

    private void release(boolean cancel) {
        touch(cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP, x);
        SystemClock.sleep(200);
    }

    private void touch(int action, float at) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, at, bounds.centerY(), 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!test.automation.injectInputEvent(event, true)) throw new AssertionError("Touch injection rejected");
        } finally { event.recycle(); }
    }
}

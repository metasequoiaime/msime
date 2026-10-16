package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;

/** 在真实 Android 键盘上验收 123 页叹号键的上滑数字与长按角标。 */
public final class SuperscriptShortcutDeviceSmoke extends DeviceSmoke {
    private static final String PLAIN = "msime-test-plain";
    private static final float SWIPE_DP = 28f;

    @Override protected String successDescription() {
        return "123-page exclamation shortcut types zero and superscript/subscript zero";
    }

    @Override protected void runChecks() throws Exception {
        shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/app.msime.android.test.EditorActivity");
        stage = "focus editor";
        tap(field(PLAIN));
        await(field(PLAIN).and(AccessibilityNodeInfo::isFocused));
        stage = "open number layer";
        tap(described("切换到数字和符号"));
        AccessibilityNodeInfo exclamation = await(describedPrefix("！；上滑输入 0；长按选择上标"));

        stage = "long press superscript zero";
        if (!exclamation.isLongClickable()
                || !exclamation.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Exclamation key did not expose its long-press menu");
        tap(described("输入 ⁰"));
        await(field(PLAIN).and(node -> equalsText("⁰", node.getText())));

        stage = "swipe up ordinary zero";
        swipeUp(describedPrefix("！；上滑输入 0；长按选择上标"));
        await(field(PLAIN).and(node -> equalsText("⁰0", node.getText())));

        stage = "long press subscript zero";
        exclamation = await(describedPrefix("！；上滑输入 0；长按选择上标"));
        if (!exclamation.performAction(AccessibilityNodeInfo.ACTION_LONG_CLICK))
            throw new AssertionError("Exclamation key long press did not reopen");
        tap(described("输入 ₀"));
        await(field(PLAIN).and(node -> equalsText("⁰0₀", node.getText())));
    }

    private void swipeUp(java.util.function.Predicate<AccessibilityNodeInfo> match) throws Exception {
        AccessibilityNodeInfo target = await(match);
        Rect bounds = new Rect();
        target.getBoundsInScreen(bounds);
        float x = bounds.centerX();
        float startY = bounds.centerY();
        float endY = startY - SWIPE_DP * getTargetContext().getResources().getDisplayMetrics().density;
        long downTime = SystemClock.uptimeMillis();
        MotionEvent down = MotionEvent.obtain(downTime, downTime, MotionEvent.ACTION_DOWN, x, startY, 0);
        MotionEvent move = MotionEvent.obtain(downTime, downTime + 40, MotionEvent.ACTION_MOVE, x, endY, 0);
        MotionEvent up = MotionEvent.obtain(downTime, downTime + 80, MotionEvent.ACTION_UP, x, endY, 0);
        try {
            for (MotionEvent event : new MotionEvent[] {down, move, up}) {
                event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
                if (!automation.injectInputEvent(event, true))
                    throw new AssertionError("Superscript shortcut swipe injection failed");
            }
        } finally {
            down.recycle();
            move.recycle();
            up.recycle();
        }
        SystemClock.sleep(150);
    }
}

package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;
import android.view.accessibility.AccessibilityWindowInfo;
import java.util.function.Predicate;

/** 14/18 键字母键的横滑：从键的中心注入真实坐标事件，不用无障碍点击代替（#8930、#11040）。 */
final class GridSwipeTouch {
    private GridSwipeTouch() {}

    /**
     * 按下读屏描述为 `description` 的键，依次横移到 `offsetsDp`（相对键中心，dp），再松手；`cancel` 时以系统取消结束。不给位移就是一次点按。
     */
    static void swipe(DeviceSmoke t, String description, boolean cancel, float... offsetsDp) throws Exception {
        t.automation.waitForIdle(500, 5000);
        AccessibilityNodeInfo key = t.awaitStableBounds(t.described(description));
        Rect bounds = new Rect();
        key.getBoundsInScreen(bounds);
        float density = t.getTargetContext().getResources().getDisplayMetrics().density;
        float x = bounds.centerX();
        float y = bounds.centerY();
        long down = SystemClock.uptimeMillis();
        inject(t, down, MotionEvent.ACTION_DOWN, x, y);
        try {
            for (float offset : offsetsDp) {
                SystemClock.sleep(16);
                x = bounds.centerX() + offset * density;
                inject(t, down, MotionEvent.ACTION_MOVE, x, y);
            }
            inject(t, down, cancel ? MotionEvent.ACTION_CANCEL : MotionEvent.ACTION_UP, x, y);
        } catch (Exception | AssertionError error) {
            inject(t, down, MotionEvent.ACTION_CANCEL, x, y);
            throw error;
        }
        t.dropStaleNodes();
    }

    /** 等到没有窗口显示 `match`：横滑限定字母之后，被排除的拼音要从拼音选择里消失。 */
    static void awaitGone(DeviceSmoke t, Predicate<AccessibilityNodeInfo> match) {
        long deadline = SystemClock.uptimeMillis() + 15000;
        do {
            boolean present = false;
            for (AccessibilityWindowInfo window : t.automation.getWindows()) {
                if (t.find(window.getRoot(), match) != null) {
                    present = true;
                    break;
                }
            }
            if (!present) return;
            SystemClock.sleep(100);
        } while (SystemClock.uptimeMillis() < deadline);
        throw new AssertionError("横滑限定字母后仍显示被排除的项（" + t.stage + "）");
    }

    private static void inject(DeviceSmoke t, long down, int action, float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, SystemClock.uptimeMillis(), action, x, y, 0);
        event.setSource(InputDevice.SOURCE_TOUCHSCREEN);
        try {
            if (!t.automation.injectInputEvent(event, true)) throw new AssertionError("横滑触摸注入被拒绝");
        } finally {
            event.recycle();
        }
    }
}

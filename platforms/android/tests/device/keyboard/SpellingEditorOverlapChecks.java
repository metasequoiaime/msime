package app.msime.android.test;

import android.graphics.Rect;
import android.os.SystemClock;
import android.view.InputDevice;
import android.view.MotionEvent;

/** 在默认开启拼写编辑时重叠按下两个真实字母键，核对输入与面板状态。 */
final class SpellingEditorOverlapChecks {
    static void run(DeviceSmoke s) throws Exception {
        for (int delay : new int[] {1, 20}) {
            for (boolean reverseRelease : new boolean[] {false, true}) {
                s.stage = "overlapping letter touches delay=" + delay + " reverse=" + reverseRelease;
                s.shellDiscard("am start -W -f 0x10008000 -n app.msime.android.test/.EditorActivity");
                s.tap(s.field("msime-test-plain"));
                s.selectChineseKeyboard();
                Rect first = new Rect(), second = new Rect();
                s.awaitStableBounds(s.key("n")).getBoundsInScreen(first);
                s.awaitStableBounds(s.key("a")).getBoundsInScreen(second);
                long down = SystemClock.uptimeMillis();
                Rect[] keys = {first, second};
                inject(s, down, down, MotionEvent.ACTION_DOWN, new int[] {0}, keys);
                inject(s, down, down + delay, MotionEvent.ACTION_POINTER_DOWN
                    | (1 << MotionEvent.ACTION_POINTER_INDEX_SHIFT), new int[] {0, 1}, keys);
                int lifted = reverseRelease ? 1 : 0;
                inject(s, down, down + delay + 30, MotionEvent.ACTION_POINTER_UP
                    | (lifted << MotionEvent.ACTION_POINTER_INDEX_SHIFT), new int[] {0, 1}, keys);
                inject(s, down, down + delay + 40, MotionEvent.ACTION_UP, new int[] {1 - lifted}, keys);
                String expected = reverseRelease ? "an" : "na";
                s.await(s.field("msime-test-plain").and(node -> DeviceSmoke.equalsText(expected, node.getText())));
                long deadline = SystemClock.uptimeMillis() + 750;
                do {
                    for (var window : s.automation.getWindows()) {
                        if (s.find(window.getRoot(), s.described("拼写编辑面板")) != null)
                            throw new AssertionError("重叠字母输入意外打开拼写编辑面板：" + s.stage);
                    }
                    SystemClock.sleep(25);
                } while (SystemClock.uptimeMillis() < deadline);
            }
        }
    }

    private static void inject(DeviceSmoke s, long down, long at, int action, int[] ids, Rect[] keys) {
        MotionEvent.PointerProperties[] properties = new MotionEvent.PointerProperties[ids.length];
        MotionEvent.PointerCoords[] coords = new MotionEvent.PointerCoords[ids.length];
        for (int index = 0; index < ids.length; index++) {
            properties[index] = new MotionEvent.PointerProperties();
            properties[index].id = ids[index];
            properties[index].toolType = MotionEvent.TOOL_TYPE_FINGER;
            coords[index] = new MotionEvent.PointerCoords();
            coords[index].x = keys[ids[index]].centerX();
            coords[index].y = keys[ids[index]].centerY();
            coords[index].pressure = coords[index].size = 1;
        }
        MotionEvent event = MotionEvent.obtain(down, at, action, ids.length, properties, coords,
            0, 0, 1, 1, 0, 0, InputDevice.SOURCE_TOUCHSCREEN, 0);
        try {
            // 整组异步投递，避免 Binder 同步等待把双指连按变成长按。
            if (!s.automation.injectInputEvent(event, false)) throw new AssertionError("双指字母事件注入失败");
        } finally { event.recycle(); }
    }
}

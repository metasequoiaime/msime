import app.msime.android.TextTrackpadGesture;

/** 二维触控板的输入连接、余量、选区和多指生命周期回归。 */
public final class TextTrackpadGestureSmoke {
    private static final Object EDITOR = new Object();

    public static void main(String[] args) {
        relativeMotionAndReversal();
        cancellationAndConnectionChanges();
        longPressAndTapSelection();
        malformedEventsDoNotMove();
        System.out.println("Android text trackpad gesture: relative motion, selection and cancellation passed");
    }

    private static TextTrackpadGesture gesture() {
        return new TextTrackpadGesture(12, 28, 500, 300, 6);
    }

    private static void relativeMotionAndReversal() {
        TextTrackpadGesture gesture = gesture();
        gesture.down(EDITOR, 100, 100, 0);
        steps(gesture.move(EDITOR, 1, 107, 115), 0, 0, false);
        steps(gesture.move(EDITOR, 1, 113, 130), 1, 1, false);
        steps(gesture.move(EDITOR, 1, 125, 158), 1, 1, false);
        steps(gesture.move(EDITOR, 1, 114, 139), 0, 0, false);
        steps(gesture.move(EDITOR, 1, 111, 127), -1, -1, false);
        check(gesture.up(EDITOR, 111, 127, 100, true) == TextTrackpadGesture.Tap.NONE,
            "拖动结束不触发选词");
        gesture.down(EDITOR, 0, 0, 200);
        steps(gesture.move(EDITOR, 1, 11, 27), 0, 0, false);
        gesture.up(EDITOR, 11, 27, 210, false);
        gesture.down(EDITOR, 0, 0, 300);
        steps(gesture.move(EDITOR, 1, 1, 1), 0, 0, false);
    }

    private static void cancellationAndConnectionChanges() {
        TextTrackpadGesture gesture = gesture();
        gesture.down(EDITOR, 0, 0, 0);
        steps(gesture.move(new Object(), 1, 24, 56), 0, 0, false);
        check(!gesture.active(), "连接切换取消旧手势");
        gesture.down(EDITOR, 0, 0, 10);
        gesture.move(EDITOR, 2, 24, 56);
        steps(gesture.move(EDITOR, 1, 48, 112), 0, 0, false);
        check(!gesture.active(), "第二指抬起不恢复旧手势");
        gesture.down(EDITOR, 0, 0, 20);
        steps(gesture.move(EDITOR, 1, 12, 28), 1, 1, false);
        gesture.cancel();
        steps(gesture.move(EDITOR, 1, 24, 56), 0, 0, false);
        gesture.down(EDITOR, 0, 0, 30);
        steps(gesture.move(EDITOR, 1, 12, 28), 1, 1, false);
    }

    private static void longPressAndTapSelection() {
        TextTrackpadGesture gesture = gesture();
        gesture.down(EDITOR, 0, 0, 10);
        check(!gesture.longPress(EDITOR, 509), "长按阈值之前不选择");
        check(gesture.longPress(EDITOR, 510), "长按开启临时选择");
        steps(gesture.move(EDITOR, 1, 12, 28), 1, 1, true);
        check(gesture.up(EDITOR, 12, 28, 600, true) == TextTrackpadGesture.Tap.NONE,
            "长按松手不触发点按选择");
        gesture.down(EDITOR, 0, 0, 700);
        steps(gesture.move(EDITOR, 1, 12, 0), 1, 0, false);
        check(!gesture.longPress(EDITOR, 1300), "拖动后不再开启长按");
        gesture.cancel();
        tap(gesture, EDITOR, 1400, TextTrackpadGesture.Tap.NONE);
        tap(gesture, EDITOR, 1500, TextTrackpadGesture.Tap.WORD);
        tap(gesture, EDITOR, 1600, TextTrackpadGesture.Tap.LINE);
        tap(gesture, EDITOR, 1700, TextTrackpadGesture.Tap.NONE);
        tap(gesture, new Object(), 1800, TextTrackpadGesture.Tap.NONE);
        gesture.cancel();
        tap(gesture, EDITOR, 2000, TextTrackpadGesture.Tap.NONE);
        tap(gesture, EDITOR, 2401, TextTrackpadGesture.Tap.NONE);
        gesture.down(EDITOR, 0, 0, 2450);
        check(gesture.up(EDITOR, 0, 0, 2460, false) == TextTrackpadGesture.Tap.NONE,
            "关闭点按选择");
        tap(gesture, EDITOR, 2500, TextTrackpadGesture.Tap.NONE);
        gesture.down(EDITOR, 80, 0, 2600);
        check(gesture.up(EDITOR, 80, 0, 2610, true) == TextTrackpadGesture.Tap.NONE,
            "不同位置不拼成双击");
    }

    private static void malformedEventsDoNotMove() {
        TextTrackpadGesture gesture = gesture();
        gesture.down(null, 0, 0, 0);
        check(!gesture.active(), "空连接不开始");
        for (float coordinate : new float[] {Float.NaN, Float.POSITIVE_INFINITY, Float.NEGATIVE_INFINITY}) {
            gesture.down(EDITOR, 0, 0, 0);
            steps(gesture.move(EDITOR, 1, coordinate, 0), 0, 0, false);
            check(!gesture.active(), "非法坐标取消");
        }
        gesture.down(EDITOR, 0, 0, 0);
        TextTrackpadGesture.Steps bounded = gesture.move(EDITOR, 1, Float.MAX_VALUE, Float.MAX_VALUE);
        check(Math.abs(bounded.horizontal()) <= 64 && Math.abs(bounded.vertical()) <= 64,
            "单次事件有界");
        gesture.cancel();
        gesture.down(EDITOR, 0, 0, 100);
        check(gesture.up(EDITOR, 0, 0, 90, true) == TextTrackpadGesture.Tap.NONE,
            "倒退时间不计入点按");
        for (float invalid : new float[] {0, -1, Float.NaN, Float.POSITIVE_INFINITY}) {
            try {
                new TextTrackpadGesture(invalid, 28, 500, 300, 6);
                throw new AssertionError("非法步长被接受");
            } catch (IllegalArgumentException expected) { }
        }
    }

    private static void tap(TextTrackpadGesture gesture, Object editor, long at, TextTrackpadGesture.Tap expected) {
        gesture.down(editor, 0, 0, at);
        check(gesture.up(editor, 0, 0, at + 10, true) == expected, "点按结果：" + expected);
    }

    private static void steps(TextTrackpadGesture.Steps value, int x, int y, boolean selecting) {
        check(value.horizontal() == x && value.vertical() == y && value.selecting() == selecting,
            "二维位移应为 " + x + ", " + y + ", " + selecting + "，实际 " + value);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

import app.msime.android.NineKeySwipePolicy;
import app.msime.android.NineKeySwipePolicy.Gesture;
import app.msime.android.SwipeHintPolicy;

/** 拼音九键网格键的滑动（#5580）：沿「滑动输入符号」的方向出数字，反方向弹字母，与 26 键共用开关与方向。 */
public final class NineKeySwipePolicySmoke {
    public static void main(String[] args) {
        String up = SwipeHintPolicy.UP;
        String down = SwipeHintPolicy.DOWN;
        check(NineKeySwipePolicy.gesture(true, up, 40, 25.5f, true) == Gesture.DIGIT, "with the up setting a swipe up types the digit");
        check(NineKeySwipePolicy.gesture(true, up, 10, 24.5f, true) == Gesture.LETTERS, "with the up setting a swipe down opens the letters");
        check(NineKeySwipePolicy.gesture(true, down, 10, 24.5f, true) == Gesture.DIGIT, "with the default down setting a swipe down types the digit");
        check(NineKeySwipePolicy.gesture(true, down, 40, 25.5f, true) == Gesture.LETTERS, "with the default down setting a swipe up opens the letters");
        check(NineKeySwipePolicy.gesture(true, up, 40, 26, true) == Gesture.NONE, "exactly 14 dp is still a tap");
        check(NineKeySwipePolicy.gesture(true, up, 40, 40, true) == Gesture.NONE, "no movement is a tap");
        check(NineKeySwipePolicy.gesture(false, up, 40, 0, true) == Gesture.NONE, "with swipes off nothing triggers");
        check(NineKeySwipePolicy.gesture(false, up, 0, 40, true) == Gesture.NONE, "with swipes off the letters do not open either");
        check(NineKeySwipePolicy.gesture(true, up, 40, 0, false) == Gesture.DIGIT, "the @# key still swipes to its digit 1");
        check(NineKeySwipePolicy.gesture(true, up, 0, 40, false) == Gesture.NONE, "the @# key has no letters to offer");
        check(NineKeySwipePolicy.gesture(true, "bogus", 0, 40, true) == Gesture.DIGIT, "an unknown direction reads as down, as on 26 keys");
        System.out.println("Android nine-key swipe: digit along the swipe direction, letters against it, shared switch passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

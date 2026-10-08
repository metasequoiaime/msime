import app.msime.android.AndroidLocalSettings;
import app.msime.android.NineKeySwipePolicy;
import app.msime.android.NineKeySwipePolicy.Gesture;
import app.msime.android.SwipeHintPolicy;

/** 拼音九键网格键的滑动（#5580）：独立的本地设置，默认关闭；选了方向后沿它出数字，反方向弹字母。 */
public final class NineKeySwipePolicySmoke {
    public static void main(String[] args) {
        String up = SwipeHintPolicy.UP;
        String down = SwipeHintPolicy.DOWN;
        String off = NineKeySwipePolicy.OFF;
        check(NineKeySwipePolicy.gesture(up, 40, 25.5f, true) == Gesture.DIGIT, "with the up setting a swipe up types the digit");
        check(NineKeySwipePolicy.gesture(up, 10, 24.5f, true) == Gesture.LETTERS, "with the up setting a swipe down opens the letters");
        check(NineKeySwipePolicy.gesture(down, 10, 24.5f, true) == Gesture.DIGIT, "with the down setting a swipe down types the digit");
        check(NineKeySwipePolicy.gesture(down, 40, 25.5f, true) == Gesture.LETTERS, "with the down setting a swipe up opens the letters");
        check(NineKeySwipePolicy.gesture(up, 40, 26, true) == Gesture.NONE, "exactly 14 dp is still a tap");
        check(NineKeySwipePolicy.gesture(up, 40, 40, true) == Gesture.NONE, "no movement is a tap");
        check(NineKeySwipePolicy.gesture(off, 40, 0, true) == Gesture.NONE, "with swipes off nothing triggers");
        check(NineKeySwipePolicy.gesture(off, 0, 40, true) == Gesture.NONE, "with swipes off the letters do not open either");
        check(NineKeySwipePolicy.gesture(up, 40, 0, false) == Gesture.DIGIT, "the @# key still swipes to its digit 1");
        check(NineKeySwipePolicy.gesture(up, 0, 40, false) == Gesture.NONE, "the @# key has no letters to offer");
        check(NineKeySwipePolicy.gesture("bogus", 0, 40, true) == Gesture.NONE, "an unknown value reads as off");
        check(NineKeySwipePolicy.gesture(null, 0, 40, true) == Gesture.NONE, "a missing value reads as off");

        // 默认关闭、只在本机：升级后九键的点按不变，也不跟 26 键「滑动输入符号」的开关和方向走。
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_SWIPE);
        check(off.equals(spec.defaultValue), "nine-key swipe is off by default");
        check(!spec.synced, "nine-key swipe is a local setting");
        check(up.equals(spec.accept(up)) && down.equals(spec.accept(down)) && off.equals(spec.accept(off)),
            "off, up and down are the accepted values");
        check(spec.accept("left") == null, "other directions are rejected");
        System.out.println("Android nine-key swipe: off by default, digit along the chosen direction, letters against it passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

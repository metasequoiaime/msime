import app.msime.android.AndroidLocalSettings;
import app.msime.android.NineKeySwipePolicy;
import app.msime.android.NineKeySwipePolicy.Gesture;
import app.msime.android.SwipeHintPolicy;

/** 拼音九键网格键的滑动（#5580）：独立的本地设置，默认关闭；选了方向后沿它出数字，反方向弹字母。判定距离是另一项本地设置（#6208），默认 24 dp，和 26 键的 14 dp 分开。 */
public final class NineKeySwipePolicySmoke {
    public static void main(String[] args) {
        String up = SwipeHintPolicy.UP;
        String down = SwipeHintPolicy.DOWN;
        String off = NineKeySwipePolicy.OFF;
        float legacy = SwipeHintPolicy.THRESHOLD_DP;
        check(NineKeySwipePolicy.gesture(up, 40, 25.5f, true, legacy) == Gesture.DIGIT, "with the up setting a swipe up types the digit");
        check(NineKeySwipePolicy.gesture(up, 10, 24.5f, true, legacy) == Gesture.LETTERS, "with the up setting a swipe down opens the letters");
        check(NineKeySwipePolicy.gesture(down, 10, 24.5f, true, legacy) == Gesture.DIGIT, "with the down setting a swipe down types the digit");
        check(NineKeySwipePolicy.gesture(down, 40, 25.5f, true, legacy) == Gesture.LETTERS, "with the down setting a swipe up opens the letters");
        check(NineKeySwipePolicy.gesture(up, 40, 26, true, legacy) == Gesture.NONE, "exactly 14 dp is still a tap");
        check(NineKeySwipePolicy.gesture(up, 40, 40, true, legacy) == Gesture.NONE, "no movement is a tap");
        check(NineKeySwipePolicy.gesture(off, 40, 0, true, legacy) == Gesture.NONE, "with swipes off nothing triggers");
        check(NineKeySwipePolicy.gesture(off, 0, 40, true, legacy) == Gesture.NONE, "with swipes off the letters do not open either");
        check(NineKeySwipePolicy.gesture(up, 40, 0, false, legacy) == Gesture.DIGIT, "the @# key still swipes to its digit 1");
        check(NineKeySwipePolicy.gesture(up, 0, 40, false, legacy) == Gesture.NONE, "the @# key has no letters to offer");
        check(NineKeySwipePolicy.gesture("bogus", 0, 40, true, legacy) == Gesture.NONE, "an unknown value reads as off");
        check(NineKeySwipePolicy.gesture(null, 0, 40, true, legacy) == Gesture.NONE, "a missing value reads as off");

        // #6208：九键用自己的「九键滑动距离」。默认 24 dp 时，快速连打时手指往上带的 20 dp 仍是点按，两个方向都一样。
        float standard = NineKeySwipePolicy.DEFAULT_THRESHOLD_DP;
        check(NineKeySwipePolicy.gesture(up, 40, 20, true, standard) == Gesture.NONE, "a 20 dp drift up is a tap at the default distance");
        check(NineKeySwipePolicy.gesture(up, 20, 40, true, standard) == Gesture.NONE, "a 20 dp drift down is a tap at the default distance");
        check(NineKeySwipePolicy.gesture(up, 40, 16, true, standard) == Gesture.NONE, "exactly the default distance is still a tap");
        check(NineKeySwipePolicy.gesture(up, 40, 15.5f, true, standard) == Gesture.DIGIT, "past the default distance a swipe up types the digit");
        check(NineKeySwipePolicy.gesture(up, 16, 40.5f, true, standard) == Gesture.LETTERS, "past the default distance a swipe down opens the letters");
        check(NineKeySwipePolicy.gesture(up, 40, 20, true, NineKeySwipePolicy.MIN_THRESHOLD_DP) == Gesture.DIGIT,
            "at the smallest distance the old 14 dp feel comes back");
        check(NineKeySwipePolicy.gesture(up, 60, 15, true, NineKeySwipePolicy.MAX_THRESHOLD_DP) == Gesture.NONE,
            "at the largest distance a 45 dp swipe is still a tap");
        check(NineKeySwipePolicy.MIN_THRESHOLD_DP == (int) SwipeHintPolicy.THRESHOLD_DP,
            "the smallest nine-key distance equals the 26-key threshold");
        check(SwipeHintPolicy.swiped(up, 40, 20), "the 26-key threshold stays at 14 dp");

        // 默认关闭、只在本机：升级后九键的点按不变，也不跟 26 键「滑动输入符号」的开关和方向走。
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_SWIPE);
        check(off.equals(spec.defaultValue), "nine-key swipe is off by default");
        check(!spec.synced, "nine-key swipe is a local setting");
        check(up.equals(spec.accept(up)) && down.equals(spec.accept(down)) && off.equals(spec.accept(off)),
            "off, up and down are the accepted values");
        check(spec.accept("left") == null, "other directions are rejected");

        AndroidLocalSettings.Spec distance = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE);
        check(distance.kind == AndroidLocalSettings.Spec.Kind.INTEGER, "nine-key swipe distance is an integer");
        check(Integer.valueOf(24).equals(distance.defaultValue), "nine-key swipe distance defaults to 24 dp");
        check(distance.min == 14 && distance.max == 48 && distance.step == 2, "nine-key swipe distance is 14..48 dp in steps of 2");
        check(!distance.synced, "nine-key swipe distance is a local setting");
        check(Integer.valueOf(14).equals(distance.accept(14)) && Integer.valueOf(48).equals(distance.accept(48)),
            "both ends of the range are accepted");
        check(distance.accept(25) == null && distance.accept(12) == null && distance.accept(50) == null,
            "off-step and out-of-range distances are rejected");
        check(AndroidLocalSettings.defaults().integer(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE)
            == NineKeySwipePolicy.DEFAULT_THRESHOLD_DP, "an untouched install reads the default distance");
        System.out.println("Android nine-key swipe: off by default, digit along the chosen direction, letters against it, own distance passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

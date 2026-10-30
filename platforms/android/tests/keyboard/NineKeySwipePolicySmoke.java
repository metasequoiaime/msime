import app.msime.android.AndroidLocalSettings;
import app.msime.android.NineKeySwipePolicy;
import app.msime.android.NineKeySwipePolicy.Gesture;
import app.msime.android.SwipeHintPolicy;

/** 拼音九键网格键的滑动（#5580）：独立的共享偏好 `touch_nine_key_swipe_digits`（#10229，随账号同步），默认关闭；选了方向后沿它出数字，反方向弹字母。判定距离仍是 Android 本地设置（#6208），默认 24 dp，和 26 键的 14 dp 分开。 */
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

        // #10229：设置在共享偏好里，字段名与 client-core 相同，写法与旧本机值逐字相同；缺键、不是字符串和不认识的写法都读成关闭，升级后九键的点按不变，也不跟 26 键「滑动输入符号」的开关和方向走。
        check("touch_nine_key_swipe_digits".equals(NineKeySwipePolicy.PREFERENCE), "the shared field name matches client-core");
        check(off.equals("off") && up.equals("up") && down.equals("down"), "the wire values are off, up and down");
        check(off.equals(NineKeySwipePolicy.mode(null)), "a missing or non-string value reads as off");
        check(up.equals(NineKeySwipePolicy.mode(up)) && down.equals(NineKeySwipePolicy.mode(down))
            && off.equals(NineKeySwipePolicy.mode(off)), "off, up and down read back as written");
        check(off.equals(NineKeySwipePolicy.mode("left")) && off.equals(NineKeySwipePolicy.mode("UP"))
            && off.equals(NineKeySwipePolicy.mode("")), "unknown spellings read as off");
        check(NineKeySwipePolicy.gesture(NineKeySwipePolicy.mode("sideways"), 40, 0, true, standard) == Gesture.NONE,
            "an unknown shared value never swipes");

        // 旧的本机键只留给迁移读：规格声明还在（否则读文件时会被丢掉），仍不经 android_local 同步，写法与共享偏好相同，迁移是原样拷贝。
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.NINE_KEY_SWIPE);
        check(off.equals(spec.defaultValue), "the legacy key defaulted to off");
        check(!spec.synced, "the legacy key never travels through android_local");
        check(up.equals(spec.accept(up)) && down.equals(spec.accept(down)) && off.equals(spec.accept(off)),
            "the legacy key still accepts off, up and down so migration can read it");
        check(spec.accept("left") == null, "other legacy directions are rejected");

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
        System.out.println("Android nine-key swipe: shared preference off by default, digit along the chosen direction, letters against it, own local distance passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

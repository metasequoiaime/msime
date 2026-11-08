import app.msime.android.EighteenKeySwipePolicy;

/** 十八键横滑只限定双字母组；轻点、纵移和不完整触摸不选择字母。 */
public final class EighteenKeySwipePolicySmoke {
    public static void main(String[] args) {
        check(letter(-25, 0) == 'W', "左滑取左字母");
        check(letter(25, 0) == 'E', "右滑取右字母");
        check(letter(24, 0) == 0 && letter(-24, 0) == 0, "阈值内仍是点按");
        check(letter(25, 20) == 0 && letter(-25, -20) == 0, "纵向位移不能被识别为横滑");
        check(letter(40, 20) == 'E', "明显横滑允许少量纵向移动");
        check(letter(-30, 0) == 'W' && letter(30, 0) == 'E', "反向移动可以重选");
        check(letter(0, 0) == 0, "回到起点不选字母");
        check(letter(Float.NaN, 0) == 0 && letter(Float.POSITIVE_INFINITY, 0) == 0,
            "无效坐标不选择字母");
        for (String unsupported : new String[]{"q", "l;", "", "abc", "WE"}) {
            check(EighteenKeySwipePolicy.letter(unsupported, 50, 0) == 0, "只接受两个小写字母");
        }
        check(EighteenKeySwipePolicy.letter(null, 50, 0) == 0, "缺少键面不选择字母");
        // 一次按压的状态（接管、重选、多指、取消、长按优先）由 GridKeyGesturePolicySmoke 覆盖。
        System.out.println("Android eighteen-key exact swipe: passed");
    }

    private static char letter(float dx, float dy) {
        return EighteenKeySwipePolicy.letter("we", dx, dy);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

import app.msime.android.EighteenKeySwipePolicy;
import app.msime.android.InputSchemeTraits;

/** 14/18 键横滑只限定双字母组；轻点、纵移和不完整触摸不选择字母，五笔和单字母键不横滑。 */
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
        // 横滑给 14/18 键全拼和双拼的两字母键（#11040）；单字母键、五笔 14 键和其他方案只点按。
        for (int scheme : new int[]{InputSchemeTraits.QUANPIN, InputSchemeTraits.SHUANGPIN}) {
            check("bn".equals(EighteenKeySwipePolicy.exactLetters("bn", scheme)), "全拼和双拼的两字母键能横滑");
            check(EighteenKeySwipePolicy.exactLetters("m", scheme) == null, "单字母键没有横滑");
            check(EighteenKeySwipePolicy.exactLetters("l;", scheme) == null, "L 分号键没有横滑");
        }
        for (int scheme : new int[]{InputSchemeTraits.WUBI, InputSchemeTraits.JAPANESE, -1}) {
            check(EighteenKeySwipePolicy.exactLetters("bn", scheme) == null, "五笔和其他方案不横滑");
        }
        check(EighteenKeySwipePolicy.exactLetters(null, InputSchemeTraits.QUANPIN) == null, "缺少键面不横滑");
        // 一次按压的状态（接管、重选、多指、取消、长按优先）由 GridKeyGesturePolicySmoke 覆盖。
        System.out.println("Android grid exact swipe: passed");
    }

    private static char letter(float dx, float dy) {
        return EighteenKeySwipePolicy.letter("we", dx, dy);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

import app.msime.android.NineKeyFlickPolicy;

/** 九键四向 flick 的方向、阈值和字母不足规则。 */
public final class NineKeyFlickPolicySmoke {
    public static void main(String[] args) {
        check(NineKeyFlickPolicy.letter(0, 0, 0, -25, "abc", 24) == NineKeyFlickPolicy.Letter.FIRST, "上滑输入首字母");
        check(NineKeyFlickPolicy.letter(0, 0, -25, 0, "abc", 24) == NineKeyFlickPolicy.Letter.SECOND, "左滑输入第二字母");
        check(NineKeyFlickPolicy.letter(0, 0, 25, 0, "abc", 24) == NineKeyFlickPolicy.Letter.THIRD, "右滑输入第三字母");
        check(NineKeyFlickPolicy.letter(0, 0, 0, 25, "abc", 24) == NineKeyFlickPolicy.Letter.NONE, "下滑无动作");
        check(NineKeyFlickPolicy.letter(0, 0, 0, -24, "abc", 24) == NineKeyFlickPolicy.Letter.NONE, "阈值严格大于");
        check(NineKeyFlickPolicy.letter(0, 0, 0, -25, "a", 24) == NineKeyFlickPolicy.Letter.FIRST, "单字母键仍支持上滑");
        check(NineKeyFlickPolicy.letter(0, 0, -25, 0, "a", 24) == NineKeyFlickPolicy.Letter.NONE, "缺少第二字母无动作");
        check("c".equals(NineKeyFlickPolicy.character("abc", NineKeyFlickPolicy.Letter.THIRD)), "字符映射");
        System.out.println("NineKeyFlickPolicySmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

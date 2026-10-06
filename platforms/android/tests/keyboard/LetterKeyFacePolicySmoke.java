import app.msime.android.LetterKeyFacePolicy;

public final class LetterKeyFacePolicySmoke {
    public static void main(String[] args) {
        check(!LetterKeyFacePolicy.displaysUppercase(true, false, false),
            "新设计中文模式字母键画小写");
        check(LetterKeyFacePolicy.face("a", true, false, false).equals("a"),
            "Chinese face is lowercase");
        // type() 在 Shift 锁定时送大写，中文模式也不例外：键面和读屏都要说大写。
        check(LetterKeyFacePolicy.face("a", true, false, true).equals("A"),
            "Chinese shift draws the uppercase it sends");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, false, true)
            .equals("大写 A"), "Chinese shift is announced");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, false, false)
            .equals("字母 A"), "Chinese face is not announced as shift");
        check(!LetterKeyFacePolicy.displaysUppercase(false, false, false),
            "English starts lowercase");
        check(LetterKeyFacePolicy.face("a", false, false, true).equals("A"),
            "English shift changes the face");
        check(LetterKeyFacePolicy.accessibilityLabel("a", false, false, true)
            .equals("大写 A"), "English shift is announced");
        check(!LetterKeyFacePolicy.displaysUppercase(true, true, false),
            "Local input keeps literal lowercase face");
        check(LetterKeyFacePolicy.face("a", true, true, false).equals("a"),
            "Local input face is lowercase");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, true, false)
            .equals("字母 A"), "Local input announces the letter");
        check(LetterKeyFacePolicy.face("a", true, true, true).equals("a"),
            "Chinese local input keeps the lowercase face under shift");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, true, true)
            .equals("字母 A"), "Chinese local input is not announced as shift");
        check(LetterKeyFacePolicy.face("a", false, true, true).equals("A"),
            "English shift still draws uppercase");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

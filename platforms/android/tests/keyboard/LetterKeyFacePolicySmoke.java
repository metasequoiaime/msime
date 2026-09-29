import app.msime.android.LetterKeyFacePolicy;

public final class LetterKeyFacePolicySmoke {
    public static void main(String[] args) {
        check(LetterKeyFacePolicy.displaysUppercase(true, false, false),
            "Chinese composition keys stay uppercase");
        check(LetterKeyFacePolicy.face("a", true, false, false).equals("A"),
            "Chinese face is uppercase");
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
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

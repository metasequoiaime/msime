import app.msime.android.LetterKeyFacePolicy;

public final class LetterKeyFacePolicySmoke {
    public static void main(String[] args) {
        // 中文模式的中文方案键面画大写（与搜狗、微信、百度、豆包一致），读屏仍读「字母」，因为输入的是小写拼写。
        check(LetterKeyFacePolicy.displaysUppercase(true, false, false, false),
            "Chinese mode draws uppercase faces");
        check(LetterKeyFacePolicy.face("a", true, false, false, false).equals("A"),
            "Chinese face is uppercase");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, false, false)
            .equals("字母 A"), "Chinese uppercase face is not announced as shift");
        // type() 在 Shift 锁定时送大写，中文模式也不例外：键面和读屏都要说大写。
        check(LetterKeyFacePolicy.face("a", true, false, false, true).equals("A"),
            "Chinese shift draws the uppercase it sends");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, false, true)
            .equals("大写 A"), "Chinese shift is announced");
        // 英文模式平时画小写，Shift 才画大写。
        check(!LetterKeyFacePolicy.displaysUppercase(false, false, false, false),
            "English starts lowercase");
        check(LetterKeyFacePolicy.face("a", false, false, false, false).equals("a"),
            "English face is lowercase");
        check(LetterKeyFacePolicy.face("a", false, false, false, true).equals("A"),
            "English shift changes the face");
        check(LetterKeyFacePolicy.accessibilityLabel("a", false, false, true)
            .equals("大写 A"), "English shift is announced");
        check(LetterKeyFacePolicy.face("a", false, false, true, true).equals("A"),
            "English shift still draws uppercase");
        // 越南语、藏文的字母区分大小写，调用方传 chineseMode=false：键面跟 Shift 走，不画中文的大写键面。
        check(LetterKeyFacePolicy.face("a", false, false, false, false).equals("a"),
            "Vietnamese face is lowercase without shift");
        check(LetterKeyFacePolicy.face("a", false, false, false, true).equals("A"),
            "Vietnamese face follows shift");
        // 日语罗马字照日文键盘的惯例画小写，Shift 才画大写。
        check(!LetterKeyFacePolicy.displaysUppercase(true, true, false, false),
            "Japanese romaji keeps lowercase faces");
        check(LetterKeyFacePolicy.face("a", true, true, false, false).equals("a"),
            "Japanese face is lowercase");
        check(LetterKeyFacePolicy.face("a", true, true, false, true).equals("A"),
            "Japanese shift draws the uppercase it sends");
        // 中文的本地输入模式（英文补全、Unicode、快捷短语等）按字面输入：键面回到小写，读屏不当作大写输入。
        check(LetterKeyFacePolicy.face("a", true, false, true, false).equals("a"),
            "Chinese local input draws the lowercase it types");
        check(LetterKeyFacePolicy.face("a", true, false, true, true).equals("a"),
            "Chinese local input stays lowercase under shift");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, true, false)
            .equals("字母 A"), "Local input announces the letter");
        check(LetterKeyFacePolicy.accessibilityLabel("a", true, true, true)
            .equals("字母 A"), "Chinese local input is not announced as shift");
        check(LetterKeyFacePolicy.face("a", true, true, true, true).equals("a"),
            "Japanese local input keeps the lowercase face under shift");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

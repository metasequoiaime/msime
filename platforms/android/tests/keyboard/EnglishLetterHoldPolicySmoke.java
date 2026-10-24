import app.msime.android.EnglishLetterHoldPolicy;
import java.util.HashSet;
import java.util.List;

public final class EnglishLetterHoldPolicySmoke {
    public static void main(String[] args) {
        String row = "qwertyuiop";
        String superDigits = "¹²³⁴⁵⁶⁷⁸⁹⁰";
        String subDigits = "₁₂₃₄₅₆₇₈₉₀";
        String financial = "壹贰叁肆伍陆柒捌玖零";
        for (int index = 0; index < row.length(); index++) {
            List<String> choices = EnglishLetterHoldPolicy.choices(row.substring(index, index + 1));
            check(choices.contains(String.valueOf(superDigits.charAt(index))), "首排数字上标");
            check(choices.contains(String.valueOf(subDigits.charAt(index))), "首排数字下标");
            check(choices.contains(String.valueOf(financial.charAt(index))), "首排财务数字");
        }
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String lower = String.valueOf(letter);
            String upper = String.valueOf(Character.toUpperCase(letter));
            List<String> choices = EnglishLetterHoldPolicy.choices(lower);
            check(choices.get(0).equals(upper) && !choices.contains(lower), "大写置首且不重复短按小写");
            check(choices.equals(EnglishLetterHoldPolicy.choices(upper)), "Shift 不改变长按字符集合");
            check(new HashSet<>(choices).size() == choices.size(), "菜单不重复字符");
        }
        check(EnglishLetterHoldPolicy.choices("a").containsAll(List.of("ᵃ", "ₐ", "ā", "á", "ǎ", "à")),
            "a 的上下标与四声");
        check(EnglishLetterHoldPolicy.choices("v").containsAll(List.of("ᵛ", "ᵥ", "ü", "ǖ", "ǘ", "ǚ", "ǜ")),
            "v 提供 ü 与四声");
        check(EnglishLetterHoldPolicy.choices("e").containsAll(List.of("ê", "ē", "é", "ě", "è")), "e 的声调");
        check(EnglishLetterHoldPolicy.choices("n").containsAll(List.of("ń", "ň", "ǹ")), "n 的声调");
        for (String[] entry : new String[][] {{"w", "\u209d"}, {"y", "\u209e"}, {"z", "\u209f"}})
            check(EnglishLetterHoldPolicy.choices(entry[0]).contains(entry[1]),
                "Unicode 18 的 " + entry[0] + " 下标不能遗漏");
        check(EnglishLetterHoldPolicy.choices("b").equals(List.of("B", "!", "ᵇ")), "不伪造 b 的下标");
        check(EnglishLetterHoldPolicy.choices("q").equals(List.of("Q", "1", "¹", "₁", "壹")),
            "缺字的 q 不以普通字母或其他字形替代");
        check(EnglishLetterHoldPolicy.choices("o").contains(EnglishLetterHoldPolicy.PINYIN_KEYBOARD),
            "O 的既有拼音键盘入口保留");
        check(EnglishLetterHoldPolicy.choices(null).isEmpty()
            && EnglishLetterHoldPolicy.choices("ab").isEmpty()
            && EnglishLetterHoldPolicy.choices("1").isEmpty(), "忽略无效输入");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

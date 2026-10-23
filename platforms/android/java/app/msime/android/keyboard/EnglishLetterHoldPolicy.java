package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** 英文 26 键的长按字符；不重复短按的小写，也不伪造 Unicode 缺少的上下标。 */
public final class EnglishLetterHoldPolicy {
    public static final String PINYIN_KEYBOARD = "⮕";
    private static final String[] SUPER = {
        "ᵃ", "ᵇ", "ᶜ", "ᵈ", "ᵉ", "ᶠ", "ᵍ", "ʰ", "ⁱ", "ʲ", "ᵏ", "ˡ", "ᵐ",
        "ⁿ", "ᵒ", "ᵖ", "", "ʳ", "ˢ", "ᵗ", "ᵘ", "ᵛ", "ʷ", "ˣ", "ʸ", "ᶻ"};
    private static final String[] SUB = {
        "ₐ", "", "", "", "ₑ", "", "", "ₕ", "ᵢ", "ⱼ", "ₖ", "ₗ", "ₘ",
        "ₙ", "ₒ", "ₚ", "", "ᵣ", "ₛ", "ₜ", "ᵤ", "ᵥ", "₝", "ₓ", "₞", "₟"};
    private static final String SUPER_DIGITS = "⁰¹²³⁴⁵⁶⁷⁸⁹";
    private static final String SUB_DIGITS = "₀₁₂₃₄₅₆₇₈₉";
    private static final String FINANCIAL_DIGITS = "零壹贰叁肆伍陆柒捌玖";

    private EnglishLetterHoldPolicy() { }

    public static List<String> choices(String input) {
        if (input == null || input.length() != 1) return List.of();
        char letter = input.toLowerCase(Locale.ROOT).charAt(0);
        if (letter < 'a' || letter > 'z') return List.of();
        List<String> choices = new ArrayList<>();
        choices.add(String.valueOf(Character.toUpperCase(letter)));
        String hint = LetterHintTable.hint(String.valueOf(letter));
        if (hint != null) choices.add(hint);
        if (hint != null && hint.length() == 1 && hint.charAt(0) >= '0' && hint.charAt(0) <= '9') {
            int digit = hint.charAt(0) - '0';
            choices.add(String.valueOf(SUPER_DIGITS.charAt(digit)));
            choices.add(String.valueOf(SUB_DIGITS.charAt(digit)));
            choices.add(String.valueOf(FINANCIAL_DIGITS.charAt(digit)));
        }
        if (!SUPER[letter - 'a'].isEmpty()) choices.add(SUPER[letter - 'a']);
        if (!SUB[letter - 'a'].isEmpty()) choices.add(SUB[letter - 'a']);
        String tones = switch (letter) {
            case 'a' -> "āáǎà";
            case 'e' -> "êēéěè";
            case 'i' -> "īíǐì";
            case 'o' -> "ōóǒò";
            case 'u' -> "ūúǔù";
            case 'v' -> "üǖǘǚǜ";
            case 'n' -> "ńňǹ";
            case 'm' -> "ḿ";
            default -> "";
        };
        for (int index = 0; index < tones.length(); index++)
            choices.add(String.valueOf(tones.charAt(index)));
        if (letter == 'o') choices.add(PINYIN_KEYBOARD);
        return List.copyOf(choices);
    }
}

package app.msime.android;

/** 主标签与角标的实际宽度不同，26 个字母仍须使用同一个排版边界。 */
public final class KeyHintWidthPolicySmoke {
    private KeyHintWidthPolicySmoke() { }

    public static void main(String[] args) {
        KeyHintWidthPolicy.Widths expected = new KeyHintWidthPolicy.Widths(24f, 12f);
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String label = String.valueOf(letter);
            for (boolean chinese : new boolean[] {false, true}) {
                String corner = LetterHintTable.face(label, chinese);
                var actual = KeyHintWidthPolicy.measure(label, corner,
                    KeyHintWidthPolicySmoke::mainWidth, KeyHintWidthPolicySmoke::cornerWidth);
                if (!expected.equals(actual))
                    throw new AssertionError("字母键留白随角标改变：" + label + " " + actual);
            }
        }
        var digit = KeyHintWidthPolicy.measure("9", "!", text -> 18, text -> 3);
        if (!digit.equals(new KeyHintWidthPolicy.Widths(18f, 3f)))
            throw new AssertionError("九键数字不应按字母组扩展留白");
        System.out.println("KeyHintWidthPolicySmoke passed");
    }

    private static double mainWidth(String text) {
        return switch (text) { case "W" -> 22; case "M" -> 24; default -> 10; };
    }

    private static double cornerWidth(String text) {
        return switch (text) { case "!", "(" -> 3; case "、" -> 12; default -> 8; };
    }
}

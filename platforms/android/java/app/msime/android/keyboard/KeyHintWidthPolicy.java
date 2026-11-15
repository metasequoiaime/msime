package app.msime.android;

import java.util.function.ToDoubleFunction;

/** 同组字母用相同的字形宽度上界决定角标留白，避免窄角标让个别主字母单独放大。 */
public final class KeyHintWidthPolicy {
    public record Widths(float main, float corner) { }

    private KeyHintWidthPolicy() { }

    public static Widths measure(String label, String corner,
            ToDoubleFunction<String> mainMeasure, ToDoubleFunction<String> cornerMeasure) {
        float mainWidth = (float) mainMeasure.applyAsDouble(label);
        float cornerWidth = (float) cornerMeasure.applyAsDouble(corner);
        if (label.length() != 1 || !isAsciiLetter(label.charAt(0)))
            return new Widths(mainWidth, cornerWidth);
        // 26 键的字母与角标来自同一组；大小写和中英文标点切换也不能改变某一个键的留白。
        for (char letter = 'a'; letter <= 'z'; letter++) {
            String lower = String.valueOf(letter);
            mainWidth = Math.max(mainWidth, (float) mainMeasure.applyAsDouble(lower));
            mainWidth = Math.max(mainWidth, (float) mainMeasure.applyAsDouble(lower.toUpperCase(java.util.Locale.ROOT)));
            for (String hint : new String[] {LetterHintTable.hint(lower), LetterHintTable.hint(lower, true),
                    LetterHintTable.face(lower, true),
                    LetterHintTable.face(lower, true, ChineseSymbolFaces.Marks.of("corner", "hollow"))})
                cornerWidth = Math.max(cornerWidth, (float) cornerMeasure.applyAsDouble(hint));
        }
        return new Widths(mainWidth, cornerWidth);
    }

    private static boolean isAsciiLetter(char value) {
        return value >= 'a' && value <= 'z' || value >= 'A' && value <= 'Z';
    }
}

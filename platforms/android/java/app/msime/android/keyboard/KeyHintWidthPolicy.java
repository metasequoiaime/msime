package app.msime.android;

import java.util.function.Supplier;
import java.util.function.ToDoubleFunction;

/** 同组字母用相同的字形宽度上界决定角标留白，避免窄角标让个别主字母单独放大。 */
public final class KeyHintWidthPolicy {
    public record Widths(float main, float corner) { }

    private KeyHintWidthPolicy() { }

    public static Widths measure(String label, String corner,
            ToDoubleFunction<String> mainMeasure, ToDoubleFunction<String> cornerMeasure) {
        return measure(label, corner, mainMeasure, cornerMeasure, () -> letterBounds(mainMeasure, cornerMeasure));
    }

    /** 同上，字母组的上界由调用方给（键盘按画笔缓存，见 KeyHintButton），只在单个字母的键面上取用。 */
    public static Widths measure(String label, String corner, ToDoubleFunction<String> mainMeasure,
            ToDoubleFunction<String> cornerMeasure, Supplier<Widths> letterBounds) {
        float mainWidth = (float) mainMeasure.applyAsDouble(label);
        float cornerWidth = (float) cornerMeasure.applyAsDouble(corner);
        if (label.length() != 1 || !isAsciiLetter(label.charAt(0)))
            return new Widths(mainWidth, cornerWidth);
        Widths bounds = letterBounds.get();
        return new Widths(Math.max(mainWidth, bounds.main()), Math.max(cornerWidth, bounds.corner()));
    }

    /** 26 个字母（大小写）和它们各种角标的宽度上界，与具体哪个键无关。 */
    public static Widths letterBounds(ToDoubleFunction<String> mainMeasure, ToDoubleFunction<String> cornerMeasure) {
        float mainWidth = 0;
        float cornerWidth = 0;
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

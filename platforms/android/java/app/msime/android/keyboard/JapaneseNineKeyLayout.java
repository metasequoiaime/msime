package app.msime.android;

import java.util.List;

/** Apple-compatible kana labels and Engine romanization strokes for the Android host. */
public final class JapaneseNineKeyLayout {
    public record Key(List<String> kana, List<String> strokes) {
        public Key {
            if (kana.size() != 5 || strokes.size() != 5) {
                throw new IllegalArgumentException("Japanese nine-key entries require five directions");
            }
            kana = List.copyOf(kana);
            strokes = List.copyOf(strokes);
        }
    }

    private static final List<Key> KEYS = List.of(
        key("あ", "い", "う", "え", "お", "a", "i", "u", "e", "o"),
        key("か", "き", "く", "け", "こ", "ka", "ki", "ku", "ke", "ko"),
        key("さ", "し", "す", "せ", "そ", "sa", "shi", "su", "se", "so"),
        key("た", "ち", "つ", "て", "と", "ta", "chi", "tsu", "te", "to"),
        key("な", "に", "ぬ", "ね", "の", "na", "ni", "nu", "ne", "no"),
        key("は", "ひ", "ふ", "へ", "ほ", "ha", "hi", "fu", "he", "ho"),
        key("ま", "み", "む", "め", "も", "ma", "mi", "mu", "me", "mo"),
        key("や", "「", "ゆ", "」", "よ", "ya", "", "yu", "", "yo"),
        key("ら", "り", "る", "れ", "ろ", "ra", "ri", "ru", "re", "ro"),
        key("わ", "を", "ん", "ー", "〜", "wa", "wo", "n'", "-", ""),
        key("、", "。", "？", "！", "…", "", "", "", "", ""));

    /** Symbols printed on the Japanese nine-key digit layer. All choices commit directly. */
    private static final List<Key> DIGIT_KEYS = List.of(
        key("1", "☆", "♪", "→", "", "", "", "", "", ""),
        key("2", "¥", "$", "€", "", "", "", "", "", ""),
        key("3", "%", "°", "#", "", "", "", "", "", ""),
        key("4", "○", "*", "・", "", "", "", "", "", ""),
        key("5", "+", "-", "=", "", "", "", "", "", ""),
        key("6", "<", "^", ">", "", "", "", "", "", ""),
        key("7", "「", "」", "：", "", "", "", "", "", ""),
        key("8", "〒", "※", "♂", "", "", "", "", "", ""),
        key("9", "（", "）", "／", "", "", "", "", "", ""),
        key("0", "〜", "…", "ー", "", "", "", "", "", ""),
        key("、", "。", "？", "！", "…", "", "", "", "", ""));

    private static final List<String> DIGIT_BRACKETS = List.of(
        "（", "）", "「", "」", "『", "』", "【", "】");

    private JapaneseNineKeyLayout() {}

    private static Key key(String center, String left, String up, String right, String down,
                           String centerStroke, String leftStroke, String upStroke,
                           String rightStroke, String downStroke) {
        return new Key(List.of(center, left, up, right, down),
            List.of(centerStroke, leftStroke, upStroke, rightStroke, downStroke));
    }

    public static List<Key> keys() { return KEYS; }
    public static List<Key> digitKeys() { return DIGIT_KEYS; }
    public static List<String> digitBrackets() { return DIGIT_BRACKETS; }
    /** Center, left, up, right and down use the same direction indices as the Apple host. */
    public static int direction(float offsetX, float offsetY, float threshold) {
        if (threshold < 0) throw new IllegalArgumentException("Flick threshold cannot be negative");
        if (Math.max(Math.abs(offsetX), Math.abs(offsetY)) < threshold) return 0;
        if (Math.abs(offsetX) > Math.abs(offsetY)) return offsetX < 0 ? 1 : 3;
        return offsetY < 0 ? 2 : 4;
    }
}

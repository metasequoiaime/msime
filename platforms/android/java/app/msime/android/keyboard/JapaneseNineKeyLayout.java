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
    /** How long after a tap the same key keeps cycling its kana (toggle input) instead of starting a new one. */
    public static final long TOGGLE_WINDOW_MS = 1000;

    /**
     * The directions a repeated tap cycles through: the entries of the same kind as the centre (all romaji strokes, or all literals for the punctuation key), skipping empty ones. や cycles や ゆ よ and leaves 「」 to the flick; わ cycles わ を ん ー and leaves 〜.
     */
    public static List<Integer> toggleCycle(Key key) {
        boolean literal = key.strokes().get(0).isEmpty();
        java.util.ArrayList<Integer> cycle = new java.util.ArrayList<>();
        for (int index = 0; index < key.kana().size(); index++) {
            if (key.kana().get(index).isEmpty()) continue;
            if (key.strokes().get(index).isEmpty() == literal) cycle.add(index);
        }
        return List.copyOf(cycle);
    }

    /** The direction `step` places after `current` in `cycle`, wrapping at either end; a direction outside the cycle restarts at its first entry. */
    public static int toggleStep(List<Integer> cycle, int current, int step) {
        int position = cycle.indexOf(current);
        if (position < 0) return cycle.get(0);
        return cycle.get(Math.floorMod(position + step, cycle.size()));
    }

    /** The longest stroke a key sends (`shi`, `chi`, `tsu`): one kana never takes more deletes than this. */
    public static final int LONGEST_STROKE = 3;

    /** Whether the composed reading ends in romaji the Engine has not turned into kana yet (`こんch`, `こn'`): a nine-key delete keeps going until it does not, so one press removes one whole kana. */
    public static boolean endsWithPendingRomaji(String reading) {
        if (reading == null || reading.isEmpty()) return false;
        char last = reading.charAt(reading.length() - 1);
        return (last >= 'a' && last <= 'z') || (last >= 'A' && last <= 'Z') || last == '\'';
    }

    /** Center, left, up, right and down use the same direction indices as the Apple host. */
    public static int direction(float offsetX, float offsetY, float threshold) {
        if (threshold < 0) throw new IllegalArgumentException("Flick threshold cannot be negative");
        if (Math.max(Math.abs(offsetX), Math.abs(offsetY)) < threshold) return 0;
        if (Math.abs(offsetX) > Math.abs(offsetY)) return offsetX < 0 ? 1 : 3;
        return offsetY < 0 ? 2 : 4;
    }
}

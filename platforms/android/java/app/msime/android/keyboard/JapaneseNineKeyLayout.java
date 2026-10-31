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

    /** 从假名网格按 123 借来的数字九宫格上，返回键印的假名键面名字（{@link KeyboardActionRow#layerTitle(int, boolean)}）。 */
    public static final String LAYER_TITLE = "あいう";

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
        java.util.ArrayList<Integer> cycle = new java.util.ArrayList<>(key.kana().size());
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

    /** 上一次轻点过去 `elapsedMillis` 毫秒后再点同一个键，是否还算连点切换。轻点时的切换判断和按下时提示中间格的判断都走这里，两边不会一边算到、一边没算到。 */
    public static boolean withinToggleWindow(long elapsedMillis) {
        return elapsedMillis <= TOGGLE_WINDOW_MS;
    }

    /**
     * 此刻轻点 `key` 会打出的方向。`toggleKey` 和 `toggleDirection` 是上一次连点留下的键和方向（没有时 `toggleKey` 为 null），`elapsedMillis` 是距那一下的时间；组字是否还原样在那里由调用方先判断，判过不成立就传 null。数字符号层、换了键、过了时间窗都从键面上的那个（0）开始，否则是循环里的下一个（あ 之后是 い，や 之后跳过「」是 ゆ）。
     */
    public static int tapDirection(Key key, boolean symbolsLayer, Key toggleKey, int toggleDirection,
                                   long elapsedMillis) {
        if (symbolsLayer || toggleKey == null || key != toggleKey || !withinToggleWindow(elapsedMillis)) return 0;
        return toggleStep(toggleCycle(key), toggleDirection, 1);
    }

    /** 参数同 {@link #tapDirection}：连点时间窗还剩多少毫秒，不在连点 `key` 时为 -1。按住不动超过它再松手，轻点就从键面上的假名重新开始，提示中间格要在那一刻换回来。 */
    public static long toggleRemainingMillis(Key key, boolean symbolsLayer, Key toggleKey, long elapsedMillis) {
        if (symbolsLayer || toggleKey == null || key != toggleKey || !withinToggleWindow(elapsedMillis)) return -1;
        return TOGGLE_WINDOW_MS - elapsedMillis;
    }

    /** The longest stroke a key sends (`shi`, `chi`, `tsu`): one kana never takes more deletes than this. */
    public static final int LONGEST_STROKE = 3;

    /** 画出来的读音（`display_reading`）末尾是否还挂着引擎没转成假名的罗马字（`こんch`、`こn`）：九键的删除一直删到不再挂着为止，按一次删掉一整个假名。 */
    public static boolean endsWithPendingRomaji(String reading) {
        if (reading == null || reading.isEmpty()) return false;
        char last = reading.charAt(reading.length() - 1);
        return (last >= 'a' && last <= 'z') || (last >= 'A' && last <= 'Z') || last == '\'';
    }

    /** Center, left, up, right and down use the same direction indices as the Apple host. */
    public static int direction(float offsetX, float offsetY, float threshold) {
        if (threshold < 0) throw new IllegalArgumentException("Flick threshold cannot be negative");
        if (BoundsPolicy.atLeast(Math.abs(offsetX), Math.abs(offsetY)) < threshold) return 0;
        if (Math.abs(offsetX) > Math.abs(offsetY)) return offsetX < 0 ? 1 : 3;
        return offsetY < 0 ? 2 : 4;
    }
}

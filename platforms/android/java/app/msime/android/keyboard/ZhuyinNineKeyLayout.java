package app.msime.android;

import java.util.List;

/**
 * 注音 9 键：左侧五个声调键，中间 1-9 和 0 十个音键，每个音键印着它代表的一组注音符号。
 *
 * <p>The grouping is FIG. 1 of US patent 6,009,444: every one of the 37 bopomofo sits on exactly one key. Like the Dachen keycaps these faces are labels only. A sound key sends its ASCII digit and a tone key the letter the Engine's Zhuyin nine-key editor reads as that tone (z ˉ, x ˊ, c ˇ, v ˋ, b ˙; the bar's Space is the first tone as well), so the Engine decides which legal syllables a digit run spells and converts them; this host keeps no bopomofo table and never opens `msime-zhuyin.db` itself.
 */
public final class ZhuyinNineKeyLayout {
    /** One sound key: the digit it sends and the bopomofo printed on it. */
    public record Key(int digit, String symbols) {
        /** The ASCII digit this key sends to the Engine. */
        public char input() { return (char) ('0' + digit); }
    }

    /** One tone key: the letter it sends, the mark it prints, its spoken name and its heatmap id. */
    public record Tone(char input, String face, String label, String keyId) {}

    private static final List<List<Key>> ROWS = List.of(
        List.of(new Key(1, "ㄅㄆㄇㄈ"), new Key(2, "ㄉㄊㄋㄌ"), new Key(3, "ㄍㄎㄏ")),
        List.of(new Key(4, "ㄐㄑㄒ"), new Key(5, "ㄓㄔㄕㄖ"), new Key(6, "ㄗㄘㄙ")),
        List.of(new Key(7, "ㄚㄛㄜㄝ"), new Key(8, "ㄧㄨㄩㄦ"), new Key(9, "ㄞㄟㄠㄡ")));
    private static final Key ZERO = new Key(0, "ㄢㄣㄤㄥ");
    // 声调键的统计 id 沿用大千键盘上同一声调键的 id（一声是空格），热力图上同一个声调在两种键盘里计入同一个键。
    private static final List<Tone> TONES = List.of(
        new Tone('z', "ˉ", "一声", "Space"),
        new Tone('x', "ˊ", "二声", "Digit6"),
        new Tone('c', "ˇ", "三声", "Digit3"),
        new Tone('v', "ˋ", "四声", "Digit4"),
        new Tone('b', "˙", "轻声", "Digit7"));

    /** Face of the layer key that returns from the symbol page to the grid. */
    public static final String LAYER_TITLE = "注";

    private ZhuyinNineKeyLayout() {}

    /** The three rows of three sound keys, 1 to 9. */
    public static List<List<Key>> rows() {
        return ROWS;
    }

    /** The 0 key under 8. */
    public static Key zero() {
        return ZERO;
    }

    /** The five tone keys from the first tone to the neutral tone. */
    public static List<Tone> tones() {
        return TONES;
    }

    /** What a sound key prints: its bopomofo over its digit, e.g. 「ㄅㄆㄇㄈ\n1」. */
    public static String face(Key key) {
        if (key == null) throw new IllegalArgumentException("Missing zhuyin nine-key key");
        return key.symbols() + "\n" + key.digit();
    }

    /** Spoken label, e.g. 「注音 1 ㄅㄆㄇㄈ」. */
    public static String accessibilityLabel(Key key) {
        if (key == null) throw new IllegalArgumentException("Missing zhuyin nine-key key");
        return "注音 " + key.digit() + " " + key.symbols();
    }

    /** Spoken label of a tone key, e.g. 「注音 一声」. */
    public static String accessibilityLabel(Tone tone) {
        if (tone == null) throw new IllegalArgumentException("Missing zhuyin tone key");
        return "注音 " + tone.label();
    }

    /** The heatmap id of a sound key: the nine-key cell with its digit. */
    public static String keyId(Key key) {
        if (key == null) throw new IllegalArgumentException("Missing zhuyin nine-key key");
        return KeyPressIds.forNineKeyDigit(key.digit());
    }
}

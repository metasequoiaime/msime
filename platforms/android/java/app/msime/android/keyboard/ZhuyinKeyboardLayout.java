package app.msime.android;

import java.util.List;
import java.util.Map;

/**
 * The Dachen bopomofo keyboard: four rows that put every bopomofo symbol and tone on a key of its own, in the positions a physical Dachen keyboard prints them.
 *
 * <p>Like the Korean keycaps these are labels only. Each key sends the ASCII key the Engine's Dachen table (crates/engine/src/zhuyin/layout.rs, which follows libchewing) reads, so the Engine decides what a key spells in every state; this host keeps no bopomofo composition of its own. Space is the first tone, and the bar's Space key sends it.
 */
public final class ZhuyinKeyboardLayout {
    private static final List<List<String>> ROWS = List.of(
        List.of("1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "-"),
        List.of("q", "w", "e", "r", "t", "y", "u", "i", "o", "p"),
        List.of("a", "s", "d", "f", "g", "h", "j", "k", "l", ";"),
        List.of("z", "x", "c", "v", "b", "n", "m", ",", ".", "/"));
    private static final Map<String, String> SYMBOLS = Map.ofEntries(
        Map.entry("1", "ㄅ"), Map.entry("2", "ㄉ"), Map.entry("3", "ˇ"), Map.entry("4", "ˋ"),
        Map.entry("5", "ㄓ"), Map.entry("6", "ˊ"), Map.entry("7", "˙"), Map.entry("8", "ㄚ"),
        Map.entry("9", "ㄞ"), Map.entry("0", "ㄢ"), Map.entry("-", "ㄦ"),
        Map.entry("q", "ㄆ"), Map.entry("w", "ㄊ"), Map.entry("e", "ㄍ"), Map.entry("r", "ㄐ"),
        Map.entry("t", "ㄔ"), Map.entry("y", "ㄗ"), Map.entry("u", "ㄧ"), Map.entry("i", "ㄛ"),
        Map.entry("o", "ㄟ"), Map.entry("p", "ㄣ"),
        Map.entry("a", "ㄇ"), Map.entry("s", "ㄋ"), Map.entry("d", "ㄎ"), Map.entry("f", "ㄑ"),
        Map.entry("g", "ㄕ"), Map.entry("h", "ㄘ"), Map.entry("j", "ㄨ"), Map.entry("k", "ㄜ"),
        Map.entry("l", "ㄠ"), Map.entry(";", "ㄤ"),
        Map.entry("z", "ㄈ"), Map.entry("x", "ㄌ"), Map.entry("c", "ㄏ"), Map.entry("v", "ㄒ"),
        Map.entry("b", "ㄖ"), Map.entry("n", "ㄙ"), Map.entry("m", "ㄩ"), Map.entry(",", "ㄝ"),
        Map.entry(".", "ㄡ"), Map.entry("/", "ㄥ"));
    private static final Map<String, String> TONES = Map.of(
        "6", "二声", "3", "三声", "4", "四声", "7", "轻声");

    /** The keys the symbol layer must not hand to the Engine while this keyboard is on screen: Dachen claims them as bopomofo or tone keys (`DACHEN_SYMBOLS` without Space), so a digit or mark picked from that page is written as itself instead. */
    public static final String CLAIMED_SYMBOLS = "1234567890,./;-";

    /** Face of the layer key that returns from the symbol page to these rows. */
    public static final String LAYER_TITLE = "注";

    private ZhuyinKeyboardLayout() {}

    /** The four Dachen rows, each key in the ASCII form it sends. */
    public static List<List<String>> rows() {
        return ROWS;
    }

    /** The bopomofo symbol or tone mark drawn on a key; a key outside the layout keeps its own face. */
    public static String face(String key) {
        if (key == null) return "";
        return SYMBOLS.getOrDefault(key, key);
    }

    /** Spoken label: the tone a tone key types, otherwise the bopomofo symbol. */
    public static String accessibilityLabel(String key) {
        String tone = key == null ? null : TONES.get(key);
        if (tone != null) return "注音 " + tone;
        String face = face(key);
        return face.isEmpty() ? "注音符号" : "注音 " + face;
    }

    /** Whether a symbol-layer key is one Dachen would read as a bopomofo or tone key. */
    public static boolean claimsSymbol(String key) {
        return key != null && key.length() == 1 && CLAIMED_SYMBOLS.contains(key);
    }
}

package app.msime.android;

import java.util.List;

/**
 * 笔画键盘：九键外框中间的 2×3 网格，五种笔画加一个通配键。
 *
 * <p>Like the Dachen keycaps these are labels only. Each key sends the stroke letter the Engine's Stroke scheme reads (h 横, s 竖, p 撇, n 点, z 折, x for any one stroke), so the Engine decides what the strokes spell and which characters they match; this host keeps no stroke-code table and never opens `stroke.db` itself. The Engine draws the typed strokes as glyphs in the view's `reading`, which is what the host marks inline.
 */
public final class StrokeKeyboardLayout {
    /** One grid key: the glyph it prints, the stroke name under it, and the ASCII letter it sends. */
    public record Key(String glyph, String name, char input) {}

    /** The wildcard letter: with nothing composing the Engine does not start a composition from it, so the keypad does not send it then. */
    public static final char WILDCARD = 'x';

    private static final List<List<Key>> ROWS = List.of(
        List.of(new Key("一", "横", 'h'), new Key("丨", "竖", 's'), new Key("丿", "撇", 'p')),
        List.of(new Key("丶", "点", 'n'), new Key("乛", "折", 'z'), new Key("＊", "通配", WILDCARD)));

    /** Face of the layer key that returns from the symbol page to the stroke grid. */
    public static final String LAYER_TITLE = "笔";

    private StrokeKeyboardLayout() {}

    /** The two grid rows, each key with the letter it sends. */
    public static List<List<Key>> rows() {
        return ROWS;
    }

    /** What a grid key prints: the stroke glyph over its name, e.g. 「一\n横」. */
    public static String face(Key key) {
        if (key == null) throw new IllegalArgumentException("Missing stroke key");
        return key.glyph() + "\n" + key.name();
    }

    /** Spoken label, e.g. 「笔画 横」 or 「笔画 通配」. */
    public static String accessibilityLabel(Key key) {
        if (key == null) throw new IllegalArgumentException("Missing stroke key");
        return "笔画 " + key.name();
    }

    /** Whether the keypad sends `input` now: the wildcard only extends a composition, and with nothing composing the Engine leaves it to the host, which would type a stray x. */
    public static boolean sends(char input, boolean composing) {
        return input != WILDCARD || composing;
    }
}

package app.msime.android;

/**
 * Whether the candidate strip shows what is being spelled.
 *
 * <p>The shared setting is 「候选栏预编辑」 on a touch host, with two values: `pinyin` draws the
 * segmented reading above the candidates, `empty` leaves that line without it. On a phone the strip
 * is the only place a composition is visible at all, so this is a real choice rather than a
 * cosmetic one — some users want the row back for candidates.
 *
 * <p>Two things that share that line are deliberately **not** governed by it.
 *
 * <p>The already-chosen part of a phrase stays visible. The runtime keeps it out of the document at
 * this host's request (`phrase_preedit`) precisely so the user can keep spelling the rest; hiding it
 * here would leave characters the user has already picked neither in the document nor on screen,
 * which is the state `scripts/test-phrase-preedit-hosts.py` exists to prevent.
 *
 * <p>A local input mode's name — 表情, 颜文字, 日期时间 and the rest — stays too. It is a label
 * saying which mode is running, not the text being composed, and removing it would leave the user
 * in a mode with nothing on screen to say so. What the mode is composing beyond its trigger is
 * preedit like any other, and does follow the setting.
 */
public final class CandidatePreeditStylePolicy {
    public static final String PINYIN = "pinyin";
    public static final String EMPTY = "empty";

    private CandidatePreeditStylePolicy() {}

    /** The stored value, with anything unrecognised reading as the shared default. */
    public static String style(String value) {
        return EMPTY.equals(value) ? EMPTY : PINYIN;
    }

    /** Whether the strip draws the composition at all. */
    public static boolean showsComposition(String style) {
        return !EMPTY.equals(style);
    }

    /**
     * The composed text to put on the strip: what was being spelled, or nothing.
     *
     * @param modeName true when the text is a local mode's own name rather than composed input
     */
    public static String composedText(String style, String text, boolean modeName) {
        if (text == null) return "";
        if (modeName || showsComposition(style)) return text;
        return "";
    }
}

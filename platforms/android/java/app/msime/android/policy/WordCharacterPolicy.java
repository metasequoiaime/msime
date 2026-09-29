package app.msime.android;

import android.view.KeyEvent;

/**
 * Which hardware key, if any, asks for the first or last character of the highlighted candidate.
 *
 * <p>以词定字 is a hardware-keyboard gesture: with a candidate highlighted, one key commits its
 * first Han character and its partner commits the last. Which pair carries it is the user's
 * choice, and the pair not in use keeps typing its own symbol.
 *
 * <p>The two pairs are also candidate-paging keys, which is why the shared `Preferences::validate`
 * refuses to let this feature and paging claim the same pair. This host reads the resolved binding
 * rather than re-deriving that rule: `disabled` when the feature is off, otherwise the pair name.
 */
public final class WordCharacterPolicy {
    /** Which end of the candidate a key asks for, or nothing. */
    public enum Edge {
        NONE(-1),
        /** `MSIME_FIRST_HAN` in the shared header. */
        FIRST(0),
        /** `MSIME_LAST_HAN` in the shared header. */
        LAST(1);

        private final int code;

        Edge(int code) {
            this.code = code;
        }

        /** The value `msime_client_select_edge` takes; never send `NONE`. */
        public int code() {
            return code;
        }
    }

    public static final String DISABLED = "disabled";
    public static final String BRACKETS = "brackets";
    public static final String MINUS_EQUAL = "minus_equal";

    private WordCharacterPolicy() {}

    /** The binding to hold for a preferences document, already folded with the enabled switch. */
    public static String binding(boolean enabled, String keys) {
        if (!enabled) return DISABLED;
        return MINUS_EQUAL.equals(keys) ? MINUS_EQUAL : BRACKETS;
    }

    /**
     * The edge this key press asks for.
     *
     * <p>Shift is excluded because the shifted faces of both pairs are ordinary punctuation the
     * user is entitled to type while composing; a candidate has to be highlighted because there is
     * otherwise nothing to take a character from.
     */
    public static Edge edgeFor(int keyCode, boolean shift, String binding,
                               boolean hasHighlightedCandidate) {
        if (shift || !hasHighlightedCandidate || binding == null) return Edge.NONE;
        if (BRACKETS.equals(binding)) {
            if (keyCode == KeyEvent.KEYCODE_LEFT_BRACKET) return Edge.FIRST;
            if (keyCode == KeyEvent.KEYCODE_RIGHT_BRACKET) return Edge.LAST;
            return Edge.NONE;
        }
        if (MINUS_EQUAL.equals(binding)) {
            if (keyCode == KeyEvent.KEYCODE_MINUS) return Edge.FIRST;
            if (keyCode == KeyEvent.KEYCODE_EQUALS) return Edge.LAST;
        }
        return Edge.NONE;
    }
}

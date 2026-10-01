package app.msime.android;

import android.view.KeyEvent;

/**
 * How this host drives the Zhuyin scheme, whose Engine is a Dachen bopomofo editor that converts what it is given into Traditional characters.
 *
 * <p>The editor belongs to the Engine: this host sends the Dachen ASCII keys, marks the view's `reading` (the conversion followed by the pending bopomofo) inline, and inserts whatever the Engine commits. Candidates appear only in a list the user opens with {@link #OPEN_CANDIDATE_LIST_COMMAND}, so the strip is empty until then (the Zhuyin contract in msime_client.h).
 */
public final class ZhuyinInputPolicy {
    /** `SchemeType::Zhuyin`: the value `View.scheme` and `commit_context.scheme` carry for this scheme. */
    public static final int ZHUYIN_SCHEME = InputSchemeTraits.ZHUYIN;
    /** Shared host command 16, `MSIME_OPEN_CANDIDATE_LIST`: the Korean Hanja command under the name that says what it does here, open the list of the composition. */
    public static final int OPEN_CANDIDATE_LIST_COMMAND = KoreanInputPolicy.CONVERT_HANJA_COMMAND;
    /** The Engine's Zhuyin Shift overlay (`SHIFT_PUNCTUATION` in crates/engine/src/zhuyin/layout.rs): each commits the conversion with its full-width mark, so the key is the Engine's in any state rather than a paging or word-character key. */
    private static final String SHIFT_PUNCTUATION = "<>?:[]{}";

    private ZhuyinInputPolicy() {}

    /** Whether the Engine's Dachen editor takes the keys: the Zhuyin scheme outside dedicated English. Zhuyin has no local modes. */
    public static boolean active(int scheme, boolean dedicatedEnglish) {
        return scheme == ZHUYIN_SCHEME && !dedicatedEnglish;
    }

    /** Whether the candidate list of the composition is open: the Zhuyin rules with candidates on the view, since the Engine lists none until the list opens. */
    public static boolean listOpen(boolean zhuyin, String localMode, int candidateCount) {
        return zhuyin && "none".equals(localMode) && candidateCount > 0;
    }

    /** Whether the open-list command applies, which is also when the candidate bar shows its 選 button: while anything composes. With the list open the button stays, and the command closes the list again. */
    public static boolean opensList(boolean zhuyin, String localMode, String editingText) {
        return zhuyin && "none".equals(localMode) && editingText != null && !editingText.isEmpty();
    }

    /** Down with no modifier while the conversion composes and its list is closed: libchewing's key for opening the list. It is the input method's whatever the arrow binding says, because with the list closed there is no highlight to move. Once the list is open Down moves the highlight as in every other list. */
    public static boolean listDownKey(int keyCode, boolean modifier, boolean opensList, boolean listOpen) {
        return keyCode == KeyEvent.KEYCODE_DPAD_DOWN && !modifier && opensList && !listOpen;
    }

    /**
     * Whether a hardware character belongs to the Engine before this host reads it as a candidate digit, a paging key or a word-character key: a key the view lists among its spelling symbols (the bopomofo and tone keys the editor claims in its current state, Space included), or a key of the Shift overlay.
     */
    public static boolean engineKey(boolean zhuyin, int unicode, String spellingSymbols) {
        if (!zhuyin || unicode < 32 || unicode > 126) return false;
        String key = String.valueOf((char) unicode);
        return SHIFT_PUNCTUATION.contains(key)
            || (spellingSymbols != null && spellingSymbols.contains(key));
    }

    /** Whether the touch Space key is Dachen input rather than the commit command: the view lists Space among its spelling symbols while a conversion composes with its list closed, where it is tone 1 on a pending syllable and opens the list otherwise. Idle it is a plain space, and with the list open the commit command picks the first row. */
    public static boolean spaceIsEngineKey(boolean zhuyin, String spellingSymbols) {
        return engineKey(zhuyin, ' ', spellingSymbols);
    }
}

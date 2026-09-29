package app.msime.android;

import android.view.KeyEvent;

/**
 * Which hardware keys page the candidate list, and which move the highlight.
 *
 * <p>The source lets the user choose the paging pair — `-`/`=`, `,`/`.`, `Shift+Tab`/`Tab`,
 * `Page Up`/`Page Down` — and uses `↑`/`↓` to move the highlighted candidate. All five are shared
 * `navigation` preferences, so this host reads the same document the desktop hosts do rather than
 * hard-coding a pair.
 *
 * <p>`[`/`]` is offered as a paging pair too, and is off by default because 以词定字 claims it.
 * The two cannot be bound to the same pair; `Preferences::validate` is what enforces that, and
 * {@link WordCharacterPolicy} is consulted first so a pair it owns never reaches this.
 *
 * <p>The Japanese romaji scheme keeps `-` for the long-vowel mark, so that pair never pages there
 * however the preference is set. That is the rule the Windows host already follows, and it is not
 * a preference the user can override: the mark has nowhere else to come from.
 *
 * <p>`Home`/`End` are here rather than with the editor's caret keys because while a list is on
 * screen they belong to it: they move the highlight to the first and last candidate of the whole
 * list. Nothing binds them, and no host offers to rebind them. Leaving them out is what made this
 * host the only one where a composition plus `End` moved the caret instead.
 *
 * <p>A pair the user turned off is not swallowed. The key goes on to whatever it would have done
 * without this feature — for the punctuation pairs that means the Engine ends the composition with
 * the highlighted candidate and inserts the mark, which is what the source does. Consuming it
 * silently would turn an unticked checkbox into a dead key.
 */
public final class CandidateNavigationPolicy {
    /** No shared command for this key. */
    public static final int NONE = -1;
    private static final int NEXT_PAGE = 100;
    private static final int PREVIOUS_PAGE = 101;
    private static final int NEXT_CANDIDATE = 102;
    private static final int PREVIOUS_CANDIDATE = 103;
    private static final int FIRST_CANDIDATE = 104;
    private static final int LAST_CANDIDATE = 105;

    /** The five switches this host can act on, read from the shared `navigation` object. */
    public record Bindings(boolean minusEqual, boolean commaPeriod, boolean brackets,
                           boolean tab, boolean pageUpDown, boolean arrows) {
        /**
         * The shared defaults, which are also what a host sees before any document is loaded.
         *
         * <p>`mouse_wheel` is not here: this host has no candidate window for a wheel to scroll,
         * and its touch strip scrolls by touch.
         */
        public static Bindings defaults() {
            return new Bindings(true, true, false, true, true, true);
        }
    }

    private CandidateNavigationPolicy() {}

    /**
     * The shared command this key asks for, or {@link #NONE}.
     *
     * <p>Only meaningful while something is being composed; the caller checks that. Shift matters
     * for exactly one pair: `Tab` pages forward and `Shift+Tab` pages back, so it is the shifted
     * face that is the second half of that pair rather than a different key.
     */
    public static int commandFor(int keyCode, boolean shift, Bindings bindings, boolean japanese) {
        if (bindings == null) return NONE;
        // The long-vowel mark is entered with this key and has no other source, so the pair is not
        // available for paging in this scheme whatever the preference says.
        if (japanese && (keyCode == KeyEvent.KEYCODE_MINUS || keyCode == KeyEvent.KEYCODE_EQUALS)) {
            return NONE;
        }
        return switch (keyCode) {
            // Home and End jump to the ends of the whole list, not of the page, and releasing the
            // withheld candidates is part of what the End command does. They are not one of the
            // five pairs the user can rebind - the source reserves them - so no binding gates them.
            // With nothing composed the caller never reaches this and they stay the editor's caret
            // keys, which is the same split macOS makes on its own panel visibility.
            case KeyEvent.KEYCODE_MOVE_HOME -> FIRST_CANDIDATE;
            case KeyEvent.KEYCODE_MOVE_END -> LAST_CANDIDATE;
            case KeyEvent.KEYCODE_DPAD_UP ->
                bindings.arrows() ? PREVIOUS_CANDIDATE : NONE;
            case KeyEvent.KEYCODE_DPAD_DOWN ->
                bindings.arrows() ? NEXT_CANDIDATE : NONE;
            case KeyEvent.KEYCODE_PAGE_UP ->
                bindings.pageUpDown() ? PREVIOUS_PAGE : NONE;
            case KeyEvent.KEYCODE_PAGE_DOWN ->
                bindings.pageUpDown() ? NEXT_PAGE : NONE;
            case KeyEvent.KEYCODE_TAB ->
                bindings.tab() ? (shift ? PREVIOUS_PAGE : NEXT_PAGE) : NONE;
            // The remaining pairs carry punctuation on their unshifted face, and the shifted face
            // is a different mark the user is entitled to type while composing.
            case KeyEvent.KEYCODE_MINUS ->
                !shift && bindings.minusEqual() ? PREVIOUS_PAGE : NONE;
            case KeyEvent.KEYCODE_EQUALS ->
                !shift && bindings.minusEqual() ? NEXT_PAGE : NONE;
            case KeyEvent.KEYCODE_COMMA ->
                !shift && bindings.commaPeriod() ? PREVIOUS_PAGE : NONE;
            case KeyEvent.KEYCODE_PERIOD ->
                !shift && bindings.commaPeriod() ? NEXT_PAGE : NONE;
            case KeyEvent.KEYCODE_LEFT_BRACKET ->
                !shift && bindings.brackets() ? PREVIOUS_PAGE : NONE;
            case KeyEvent.KEYCODE_RIGHT_BRACKET ->
                !shift && bindings.brackets() ? NEXT_PAGE : NONE;
            default -> NONE;
        };
    }
}

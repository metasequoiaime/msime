package app.msime.android;

import android.view.KeyEvent;

/**
 * The two maintenance chords a hardware keyboard needs, because touch reaches them by gesture.
 *
 * <p>Deleting a wrong entry from the user dictionary is a long press on the candidate here, and
 * resetting the candidate cache is not on the touch keyboard at all. Neither has an equivalent for
 * somebody typing on a tablet, a Chromebook or a desktop-mode dock, so the source's chords are the
 * only way to reach them: `Ctrl + Shift + Alt + 1..8` deletes the candidate in that slot, and
 * `Ctrl + Shift + Alt + C` drops the cached candidate list.
 *
 * <p>Ctrl+Shift+Alt is deliberately awkward. Both actions change or discard state the user cannot
 * undo from the keyboard, and neither is something to arrive at by a slip while composing.
 */
public final class HardwareMaintenancePolicy {
    /** No maintenance chord; the key belongs to the composition or the application. */
    public static final int NONE = -1;
    /** Reset the cached candidate list rather than a numbered slot. */
    public static final int RESET_CACHE = -2;

    private HardwareMaintenancePolicy() {}

    /**
     * The zero-based candidate slot this chord deletes, {@link #RESET_CACHE}, or {@link #NONE}.
     *
     * <p>Whether that slot exists, and whether its candidate may be deleted at all, is the
     * caller's to check: this only says which key was pressed. The cache chord answers whether or
     * not something is being spelled — the cache belongs to the session, and a stale candidate
     * list is exactly what the user is staring at when they reach for it.
     */
    public static int action(int keyCode, boolean shift, boolean ctrl, boolean alt, boolean meta,
                             int repeatCount, boolean composing) {
        if (repeatCount != 0 || !ctrl || !shift || !alt || meta) return NONE;
        if (keyCode == KeyEvent.KEYCODE_C) return RESET_CACHE;
        if (!composing) return NONE;
        if (keyCode >= KeyEvent.KEYCODE_1 && keyCode <= KeyEvent.KEYCODE_8) {
            return keyCode - KeyEvent.KEYCODE_1;
        }
        return NONE;
    }
}

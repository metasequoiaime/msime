package app.msime.android;

import android.view.KeyEvent;

/**
 * Maps the hardware number row to the visible candidate slot.
 *
 * <p>The row means two different things depending on what is being spelled. While a pinyin or wubi
 * code is open, `1`..`9` pick off the strip, which is what the number row is for on every desktop
 * input method. In the Unicode local mode the same keys are the code point itself, so they have to
 * reach the Engine as input and the pick moves to `Shift+1`..`Shift+9` - which is the arrangement
 * the source uses and the one the settings page promises the user: 「空格上屏；Shift+数字选词」.
 *
 * <p>Without the mode this class could not tell the two apart, and it did not have it: every digit
 * typed after the first hex character of a code point selected a candidate instead, so U mode was
 * unusable from a hardware keyboard while the on-screen keyboard - which never comes through here -
 * worked. HarmonyOS decides it in the same place for the same reason.
 */
public final class NumberRowSelectionPolicy {
    /** The local mode name the shared view uses for hexadecimal code point entry. */
    public static final String UNICODE_MODE = "unicode";

    /** Not a candidate pick. */
    public static final int NONE = -1;

    private NumberRowSelectionPolicy() {}

    /**
     * The candidate slot this key picks, or {@link #NONE}.
     *
     * @param localMode the active local input mode from the shared view, `none` when there is none
     */
    public static int slotForKeyCode(int keyCode, boolean shift, boolean enabled, String localMode) {
        if (!enabled) return NONE;
        if (keyCode < KeyEvent.KEYCODE_1 || keyCode > KeyEvent.KEYCODE_9) return NONE;
        boolean unicode = UNICODE_MODE.equals(localMode);
        // Outside U mode the shifted faces are the marks above the digits, and the user is entitled
        // to type them mid-composition; inside it they are the only way left to pick.
        if (shift != unicode) return NONE;
        return keyCode - KeyEvent.KEYCODE_1;
    }
}

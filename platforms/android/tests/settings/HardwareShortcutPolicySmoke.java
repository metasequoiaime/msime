package app.msime.android;

import android.view.KeyEvent;

public final class HardwareShortcutPolicySmoke {
    public static void main(String[] args) {
        if (HardwareShortcutPolicy.chord(KeyEvent.KEYCODE_SPACE, true, false, false, 0, true, true, true)
                != HardwareShortcutPolicy.Action.TOGGLE_LANGUAGE) throw new AssertionError("shift space");
        if (HardwareShortcutPolicy.chord(KeyEvent.KEYCODE_SPACE, false, true, true, 0, true, true, true)
                != HardwareShortcutPolicy.Action.TOGGLE_LANGUAGE) throw new AssertionError("ctrl alt space");
        if (HardwareShortcutPolicy.chord(KeyEvent.KEYCODE_F, true, true, false, 0, true, true, true)
                != HardwareShortcutPolicy.Action.TOGGLE_CHARACTER_SET) throw new AssertionError("ctrl shift f");
        // The reference reserves Ctrl+Shift+E for its English candidate mode and asserts the two
        // chords are never confused. This host had 简繁 on E, so the assertion that E does nothing
        // is the one that would have caught it - the positive assertion above passed either way
        // while it carried the raw number.
        if (HardwareShortcutPolicy.chord(KeyEvent.KEYCODE_E, true, true, false, 0, true, true, true)
                != HardwareShortcutPolicy.Action.NONE) throw new AssertionError("ctrl shift e");
        if (HardwareShortcutPolicy.chord(KeyEvent.KEYCODE_H, true, false, true, 0, true, true, true)
                != HardwareShortcutPolicy.Action.TOGGLE_FULL_WIDTH) throw new AssertionError("alt shift h");
        if (HardwareShortcutPolicy.chord(KeyEvent.KEYCODE_SPACE, true, false, false, 0, false, true, true)
                != HardwareShortcutPolicy.Action.NONE) throw new AssertionError("disabled");
        // Ctrl + . is the source's punctuation chord. Asserting against KeyEvent's own constant
        // rather than the number in the policy is the point: the policy carries raw key codes, and
        // a wrong one is invisible until somebody presses the key.
        if (HardwareShortcutPolicy.chord(android.view.KeyEvent.KEYCODE_PERIOD, false, true, false,
                0, true, true, true) != HardwareShortcutPolicy.Action.TOGGLE_PUNCTUATION) {
            throw new AssertionError("ctrl period");
        }
        // The shifted face of that key is a mark the user is entitled to type while composing.
        if (HardwareShortcutPolicy.chord(android.view.KeyEvent.KEYCODE_PERIOD, true, true, false,
                0, true, true, true) != HardwareShortcutPolicy.Action.NONE) {
            throw new AssertionError("ctrl shift period");
        }
        if (HardwareShortcutPolicy.chord(android.view.KeyEvent.KEYCODE_PERIOD, false, false, false,
                0, true, true, true) != HardwareShortcutPolicy.Action.NONE) {
            throw new AssertionError("bare period");
        }
        if (HardwareShortcutPolicy.chord(android.view.KeyEvent.KEYCODE_COMMA, false, true, false,
                0, true, true, true) != HardwareShortcutPolicy.Action.NONE) {
            throw new AssertionError("ctrl comma");
        }
    }
}

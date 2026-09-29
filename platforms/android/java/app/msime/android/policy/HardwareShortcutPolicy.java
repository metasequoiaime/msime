package app.msime.android;

import android.view.KeyEvent;

/**
 * Chords shared by desktop settings and Android hardware keyboards.
 *
 * <p>Every key is named with its {@link KeyEvent} constant. This file used to carry the raw numbers
 * and one of them was wrong: 简繁 was bound to 33, which is {@code KEYCODE_E}, not the
 * {@code KEYCODE_F} the preference is named after and the settings page promises. A wrong number
 * here is invisible to every reader and to every gate; the only thing that reveals it is pressing
 * the key. The smoke beside this file makes the same point and must keep asserting through the
 * constants for the same reason.
 */
public final class HardwareShortcutPolicy {
    public enum Action {
        NONE, TOGGLE_LANGUAGE, TOGGLE_CHARACTER_SET, TOGGLE_FULL_WIDTH, TOGGLE_PUNCTUATION
    }
    private HardwareShortcutPolicy() {}

    public static Action chord(int key, boolean shift, boolean ctrl, boolean alt, int repeatCount,
                               boolean language, boolean characterSet, boolean fullWidth) {
        if (repeatCount != 0) return Action.NONE;
        if (key == KeyEvent.KEYCODE_SPACE && language && shift && !ctrl && !alt)
            return Action.TOGGLE_LANGUAGE;
        if (key == KeyEvent.KEYCODE_SPACE && language && ctrl && alt)
            return Action.TOGGLE_LANGUAGE;
        // Ctrl+Shift+F, which is what `toggle_character_set_ctrl_shift_f` is named after and what
        // the settings page tells the user. The reference keeps Ctrl+Shift+E for its English
        // candidate mode and asserts the two are never confused.
        if (key == KeyEvent.KEYCODE_F && characterSet && ctrl && shift && !alt)
            return Action.TOGGLE_CHARACTER_SET;
        if (key == KeyEvent.KEYCODE_H && fullWidth && alt && shift && !ctrl)
            return Action.TOGGLE_FULL_WIDTH;
        // Ctrl + . switches Chinese and English punctuation, as the source does. It is not one of
        // the five shared keybinding switches, so there is no preference gating it: the desktop
        // hosts reserve this chord unconditionally too.
        if (key == KeyEvent.KEYCODE_PERIOD && ctrl && !shift && !alt)
            return Action.TOGGLE_PUNCTUATION;
        return Action.NONE;
    }
}

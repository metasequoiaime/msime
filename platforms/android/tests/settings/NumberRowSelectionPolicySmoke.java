import android.view.KeyEvent;
import app.msime.android.NumberRowSelectionPolicy;

public final class NumberRowSelectionPolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // Asserting through KeyEvent's own constants rather than through the numbers the policy
        // compares against is the point of this file: a wrong key code reads correctly and only
        // shows up when somebody presses the key.
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_1, false, true, "none") == 0, "1");
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_9, false, true, "none") == 8, "9");
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_0, false, true, "none") == -1, "zero");
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_1, false, false, "none") == -1, "disabled");

        // The four quadrants of the mode/shift split. In U mode the plain digits are the code point
        // and must reach the Engine, so the pick is on the shifted face; everywhere else the shifted
        // face is the mark above the digit and the plain one picks.
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_4, false, true, "unicode") == -1,
            "U mode plain digit is a hex digit, not a pick");
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_1, true, true, "unicode") == 0,
            "U mode shift picks");
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_9, true, true, "unicode") == 8,
            "U mode shift picks the ninth");
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_1, true, true, "none") == -1,
            "outside U mode the shifted face is a mark");

        // A disabled row stays disabled in U mode too: the shifted face is not a way around the
        // preference, it is the same feature moved.
        check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_1, true, false, "unicode") == -1,
            "disabled in U mode");
        // Every other local mode keeps the ordinary arrangement; only U spells with digits.
        for (String mode : new String[] {"none", "quick_phrase", "date_time", "emoji", "kaomoji"}) {
            check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_2, false, true, mode) == 1,
                "plain digit picks in " + mode);
            check(NumberRowSelectionPolicy.slotForKeyCode(KeyEvent.KEYCODE_2, true, true, mode) == -1,
                "shifted digit is a mark in " + mode);
        }
        System.out.println("Android number-row candidate selection passed");
    }
}

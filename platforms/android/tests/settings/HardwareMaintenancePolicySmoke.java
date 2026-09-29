import android.view.KeyEvent;
import app.msime.android.HardwareMaintenancePolicy;

/** The two chords a hardware keyboard needs, and everything that must not be mistaken for them. */
public final class HardwareMaintenancePolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    /** Ctrl+Shift+Alt, no Meta, first press, composing. */
    static int chord(int keyCode) {
        return HardwareMaintenancePolicy.action(keyCode, true, true, true, false, 0, true);
    }

    public static void main(String[] args) {
        // Eight slots, zero-based, matching the number row the candidates are shown against.
        check(chord(KeyEvent.KEYCODE_1) == 0, "the first slot is index 0");
        check(chord(KeyEvent.KEYCODE_8) == 7, "the eighth slot is index 7");
        check(chord(KeyEvent.KEYCODE_5) == 4, "the slots run in order");
        check(chord(KeyEvent.KEYCODE_9) == HardwareMaintenancePolicy.NONE,
            "there is no ninth slot in this chord");
        check(chord(KeyEvent.KEYCODE_0) == HardwareMaintenancePolicy.NONE,
            "zero is not a slot");

        check(chord(KeyEvent.KEYCODE_C) == HardwareMaintenancePolicy.RESET_CACHE,
            "C resets the cache rather than naming a slot");
        // The cache belongs to the session, not to a composition, and a stale candidate list is
        // exactly what the user is looking at when they reach for this.
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_C, true, true, true, false, 0,
                false) == HardwareMaintenancePolicy.RESET_CACHE,
            "the cache chord works with nothing being spelled");
        // A slot only means something while candidates are on screen.
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_1, true, true, true, false, 0,
                false) == HardwareMaintenancePolicy.NONE,
            "a slot chord needs a composition to name a candidate in");

        // Ctrl+Shift+Alt is deliberately awkward: both actions discard state the user cannot undo
        // from the keyboard, so every weaker combination has to miss.
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_1, true, true, false, false, 0,
                true) == HardwareMaintenancePolicy.NONE, "Ctrl+Shift+1 is not the chord");
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_1, false, true, true, false, 0,
                true) == HardwareMaintenancePolicy.NONE, "Ctrl+Alt+1 is not the chord");
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_1, true, false, true, false, 0,
                true) == HardwareMaintenancePolicy.NONE, "Shift+Alt+1 is not the chord");
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_1, false, false, false, false, 0,
                true) == HardwareMaintenancePolicy.NONE, "a bare digit selects, it does not delete");
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_C, true, true, true, true, 0,
                true) == HardwareMaintenancePolicy.NONE,
            "adding Meta makes it the system's chord, not ours");

        // Held keys must not delete a run of candidates or reset the cache repeatedly.
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_1, true, true, true, false, 1,
                true) == HardwareMaintenancePolicy.NONE, "a repeat does not delete again");
        check(HardwareMaintenancePolicy.action(KeyEvent.KEYCODE_C, true, true, true, false, 3,
                false) == HardwareMaintenancePolicy.NONE, "a repeat does not reset again");

        check(chord(KeyEvent.KEYCODE_A) == HardwareMaintenancePolicy.NONE,
            "an unrelated letter is not a chord");
        check(HardwareMaintenancePolicy.NONE != HardwareMaintenancePolicy.RESET_CACHE
                && HardwareMaintenancePolicy.RESET_CACHE < 0,
            "the two sentinels are distinct and neither can be read as a slot");
        System.out.println("Android hardware maintenance chords passed");
    }
}

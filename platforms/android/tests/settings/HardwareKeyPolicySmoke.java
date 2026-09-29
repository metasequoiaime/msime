import android.view.KeyEvent;
import app.msime.android.HardwareKeyPolicy;

public final class HardwareKeyPolicySmoke {
    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_DEL) == 0, "backspace");
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_DPAD_LEFT) == 4, "left");
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_DPAD_RIGHT) == 5, "right");
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_MOVE_HOME) == 6, "home");
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_MOVE_END) == 7, "end");
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_FORWARD_DEL) == 8, "forward delete");
        check(HardwareKeyPolicy.commandFor(KeyEvent.KEYCODE_TAB) == -1, "editor key fallback");
        System.out.println("Android hardware key policy: Engine navigation mapping passed");
    }
}

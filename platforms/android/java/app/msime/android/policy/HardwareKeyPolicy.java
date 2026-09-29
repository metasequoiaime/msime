package app.msime.android;

import android.view.KeyEvent;

/** Maps editor hardware navigation keys to the shared Engine command surface. */
public final class HardwareKeyPolicy {
    private HardwareKeyPolicy() {}

    /** Returns the shared command, or -1 when the key belongs to the editor/platform. */
    public static int commandFor(int keyCode) {
        return switch (keyCode) {
            case KeyEvent.KEYCODE_DEL -> 0;
            case KeyEvent.KEYCODE_DPAD_LEFT -> 4;
            case KeyEvent.KEYCODE_DPAD_RIGHT -> 5;
            case KeyEvent.KEYCODE_MOVE_HOME -> 6;
            case KeyEvent.KEYCODE_MOVE_END -> 7;
            case KeyEvent.KEYCODE_FORWARD_DEL -> 8;
            default -> -1;
        };
    }
}

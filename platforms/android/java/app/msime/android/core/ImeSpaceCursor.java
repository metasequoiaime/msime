package app.msime.android;

import android.view.KeyEvent;
import android.view.View;

/** 空格滑动专用的逐步反馈；底栏拖动继续走原有无反馈通道。 */
final class ImeSpaceCursor {
    private ImeSpaceCursor() {}

    static void move(MSIMEInputService s, View source, int offset) {
        var target = s.connection;
        if (offset == 0 || target == null) return;
        s.selectionEcho.invalidate();
        int steps = s.imeTextEditPanel.cursorPlan(target, offset, 0, false).horizontal();
        int keyCode = steps < 0 ? KeyEvent.KEYCODE_DPAD_LEFT : KeyEvent.KEYCODE_DPAD_RIGHT;
        for (int index = 0; index < Math.abs(steps); index++) {
            if (s.connection != target || !s.cursorMovement.isActive()) return;
            if (s.sendEditorKeyEvents(keyCode) && s.connection == target && s.cursorMovement.isActive()
                    && s.hapticsEnabled) {
                KeyboardHaptics.play(s.vibrator, source, s.hapticStrength);
            }
        }
    }
}

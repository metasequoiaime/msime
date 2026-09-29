package app.msime.android;

import android.content.Context;
import android.graphics.drawable.Drawable;

/** Button whose shortcut-bar face stays plain while the shared host still styles its text. */
public final class KeyboardBorderlessButton extends KeyboardPressButton {
    public KeyboardBorderlessButton(Context context) {
        super(context);
        setKeyboardRole(KeyboardKeyRole.GLYPH);
        super.setBackground(null);
    }

    @Override public void setBackground(Drawable background) {
        // The scheme shortcut follows Apple's plain toolbar treatment. Its parent still owns the
        // hit target and skin text color; the common style pass must not put the box back.
    }
}

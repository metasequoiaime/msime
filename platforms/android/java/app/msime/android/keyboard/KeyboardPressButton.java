package app.msime.android;

import android.widget.Button;

/** Matches Apple's key press feedback without changing the button's layout or input timing. */
public class KeyboardPressButton extends Button {
    private KeyboardKeyRole role;

    public KeyboardPressButton(android.content.Context context) {
        super(context);
    }

    /**
     * The face the shared style pass should give this button.
     *
     * <p>It is carried on the view rather than decided at style time because that pass re-walks the
     * whole tree on every render and has no other way to tell a toolbar glyph from a key cap. Left
     * unset, the button keeps following the accessibility description the pass has always read, so
     * only the controls that asked for a role change appearance.
     */
    public void setKeyboardRole(KeyboardKeyRole value) { role = value; }

    /** The requested role, or {@code null} to follow the shared derivation. */
    public KeyboardKeyRole keyboardRole() { return role; }

    @Override public void setPressed(boolean pressed) {
        boolean changed = pressed != isPressed();
        super.setPressed(pressed);
        if (changed) updatePressFeedback();
    }

    @Override public void setEnabled(boolean enabled) {
        super.setEnabled(enabled);
        if (!enabled) updatePressFeedback();
    }

    @Override protected void onDetachedFromWindow() {
        animate().cancel();
        KeyboardPressFeedback.reset(this);
        super.onDetachedFromWindow();
    }

    private void updatePressFeedback() {
        KeyboardPressFeedback.update(this, isPressed());
    }
}

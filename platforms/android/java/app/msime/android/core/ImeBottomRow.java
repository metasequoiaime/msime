package app.msime.android;

import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.Button;
import android.widget.LinearLayout;

/** 键盘底行：按 KeyboardActionRow 排布功能键、空格键滑动移光标、换行键的外观角色；从 MSIMEInputService 原样搬出。 */
final class ImeBottomRow {
    private final MSIMEInputService s;

    ImeBottomRow(MSIMEInputService s) {
        this.s = s;
    }

    /** The return key's face: accent-filled 确认 while composing, the function tint otherwise. */
    KeyboardKeyRole returnKeyRole() {
        boolean composing = s.returnKeyConfirms();
        return composing ? KeyboardKeyRole.RETURN : KeyboardKeyRole.ACCENT;
    }

    void resetSpaceCursor() {
        s.cursorMovement.cancel();
        if (s.spaceButton != null) {
            s.spaceButton.setPressed(false);
            s.spaceButton.setText(s.spaceKeyTitle());
            s.spaceButton.setContentDescription(s.spaceKeyDescription());
        }
        if (s.japaneseSpaceKey != null && s.japaneseSpaceKey != s.spaceButton) {
            s.japaneseSpaceKey.setPressed(false);
            s.japaneseSpaceKey.setText(s.spaceKeyTitle());
            s.japaneseSpaceKey.setContentDescription(s.spaceKeyDescription());
        }
    }

    void moveEditorCursor(int offset) {
        // 方向键由编辑器自己解释（换行、代理对、双向文字），落点算不出来。
        if (offset != 0) s.selectionEcho.invalidate();
        int keyCode = offset < 0 ? KeyEvent.KEYCODE_DPAD_LEFT : KeyEvent.KEYCODE_DPAD_RIGHT;
        for (int index = 0; index < Math.abs(offset); index++) s.sendDownUpKeyEvents(keyCode);
    }

    void bindSpaceCursor(Button button) {
        final float[] origin = new float[2];
        final boolean[] dragging = new boolean[1];
        final boolean[] cancelled = new boolean[1];
        final int touchSlop = ViewConfiguration.get(s).getScaledTouchSlop();
        button.setOnTouchListener((ignored, event) -> {
            switch (event.getActionMasked()) {
                case MotionEvent.ACTION_DOWN -> {
                    origin[0] = event.getX();
                    origin[1] = event.getY();
                    dragging[0] = false;
                    cancelled[0] = false;
                    button.setPressed(true);
                    button.getParent().requestDisallowInterceptTouchEvent(true);
                    return true;
                }
                case MotionEvent.ACTION_MOVE -> {
                    if (!dragging[0] && !cancelled[0]) {
                        float horizontal = event.getX() - origin[0];
                        float vertical = event.getY() - origin[1];
                        if (Math.abs(horizontal) <= touchSlop && Math.abs(vertical) <= touchSlop)
                            return true;
                        if (Math.abs(horizontal) <= Math.abs(vertical) || s.connection == null) {
                            cancelled[0] = true;
                            button.setPressed(false);
                            return true;
                        }
                        s.command(2);
                        s.cursorMovement.begin(origin[0], s.connection);
                        dragging[0] = s.cursorMovement.isActive();
                        cancelled[0] = !dragging[0];
                        button.setPressed(false);
                        if (dragging[0]) {
                            button.setText("移动光标");
                            button.setContentDescription("正在移动光标");
                            moveEditorCursor(s.cursorMovement.advance(
                                event.getX(), s.connection, s.pixels(12)));
                        }
                        return true;
                    }
                    if (dragging[0]) {
                        moveEditorCursor(s.cursorMovement.advance(
                            event.getX(), s.connection, s.pixels(12)));
                        if (!s.cursorMovement.isActive()) {
                            dragging[0] = false;
                            cancelled[0] = true;
                            resetSpaceCursor();
                        }
                    }
                    return true;
                }
                case MotionEvent.ACTION_UP -> {
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    button.setPressed(false);
                    if (dragging[0] || cancelled[0]) {
                        // The thumb still pressed the space bar; dragging it moved the cursor instead of typing.
                        s.countKey(button);
                        resetSpaceCursor();
                    } else button.performClick();
                    return true;
                }
                case MotionEvent.ACTION_CANCEL -> {
                    button.getParent().requestDisallowInterceptTouchEvent(false);
                    dragging[0] = false;
                    cancelled[0] = true;
                    resetSpaceCursor();
                    return true;
                }
                default -> { return true; }
            }
        });
    }

    Button actionRowKey(KeyboardActionRow.Slot slot) {
        return switch (slot) {
            case SYMBOL_PANEL -> s.symbolPanelButton;
            case LAYER -> s.layerButton;
            case GLOBE -> s.globeButton;
            case PUNCTUATION -> s.quickPunctuationButton;
            case SPACE -> s.spaceButton;
            case LANGUAGE -> s.languageButton;
            case RETURN -> s.enterButton;
        };
    }

    /**
     * Lay the bottom row out for the surface on screen.
     *
     * <p>Every control here is a long-lived field with its own listeners and state, so the row is
     * re-parented rather than rebuilt: a fresh set of buttons each time would drop the space key's
     * cursor gesture and the delete key's repeat.
     */
    void updateActionRow() {
        if (s.actionRow == null) return;
        int layout = s.displayedTouchLayout(s.view);
        boolean globe = s.shouldOfferSwitchingToNextInputMethod();
        // Every keystroke reaches render(), and re-parenting eight keys under the pressed one is a
        // relayout the user can see. The row only changes when the surface does.
        String signature = layout + ":" + globe;
        java.util.List<KeyboardActionRow.Entry> entries =
            KeyboardActionRow.entries(layout, globe);
        // Visibility is re-asserted every time: the reply surface hides this row and restores it
        // without the surface itself having changed.
        s.actionRow.setVisibility(entries.isEmpty() ? View.GONE : View.VISIBLE);
        if (signature.equals(s.actionRowSignature)) return;
        s.actionRowSignature = signature;
        s.actionRow.removeAllViews();
        for (KeyboardActionRow.Entry entry : entries) {
            Button key = actionRowKey(entry.slot());
            if (key == null) continue;
            if (key.getParent() instanceof android.view.ViewGroup parent) parent.removeView(key);
            // The shared design tints every function key in this row (123, 中, 换行 and the
            // symbol and globe keys); only the punctuation and space keys wear plain key caps.
            boolean function = entry.slot() != KeyboardActionRow.Slot.SPACE
                && entry.slot() != KeyboardActionRow.Slot.PUNCTUATION;
            KeyboardKeyRole role = entry.slot() == KeyboardActionRow.Slot.RETURN ? returnKeyRole()
                : function ? KeyboardKeyRole.ACCENT : KeyboardKeyRole.KEY;
            if (key instanceof KeyboardPressButton press) press.setKeyboardRole(role);
            key.setVisibility(View.VISIBLE);
            s.actionRow.addView(key, new LinearLayout.LayoutParams(0,
                LinearLayout.LayoutParams.MATCH_PARENT, entry.weight()));
        }
        // The quick punctuation key hides itself when the scheme has no punctuation to offer, and
        // the loop above just told every slot it was visible.
        s.updateQuickPunctuation();
        s.imeStyler.applyKeyboardGeometry();
    }
}

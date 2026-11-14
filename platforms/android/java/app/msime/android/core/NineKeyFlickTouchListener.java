package app.msime.android;

import android.view.MotionEvent;
import android.view.View;

/** 九键 flick 先接管超过阈值的触摸，再按方向决定字母；长按弹窗保留原有拖选。 */
final class NineKeyFlickTouchListener implements View.OnTouchListener {
    private final MSIMEInputService service;
    private final ImeLayoutRows rows;
    private final NineKeyDigitButton button;
    private final String letters;
    private float downX;
    private float downY;
    private boolean consumed;
    private NineKeyFlickPolicy.Letter letter = NineKeyFlickPolicy.Letter.NONE;

    NineKeyFlickTouchListener(MSIMEInputService service, ImeLayoutRows rows,
            NineKeyDigitButton button, NineKeyLayout.Key key) {
        this.service = service;
        this.rows = rows;
        this.button = button;
        letters = key.digit() == 1 ? "" : key.label().toLowerCase(java.util.Locale.ROOT);
    }

    @Override public boolean onTouch(View view, MotionEvent event) {
        service.imeLetterRows.updateKeyPreview(button, event);
        switch (event.getActionMasked()) {
            case MotionEvent.ACTION_DOWN -> {
                if (rows.tracksLetterOptions(button)) rows.dismissNineKeyHoldOptions();
                downX = DimensionPolicy.fromPixels(service, event.getX());
                downY = DimensionPolicy.fromPixels(service, event.getY());
                consumed = false;
                letter = NineKeyFlickPolicy.Letter.NONE;
                return false;
            }
            case MotionEvent.ACTION_MOVE -> {
                if (rows.tracksLetterOptions(button)) {
                    rows.moveLetterOptions(button, event);
                    return true;
                }
                if (consumed) return true;
                if (service.nineKeyHoldPopup != null) return false;
                float x = DimensionPolicy.fromPixels(service, event.getX());
                float y = DimensionPolicy.fromPixels(service, event.getY());
                if (Math.max(Math.abs(x - downX), Math.abs(y - downY)) <= service.nineKeyFlickThreshold)
                    return false;
                consumed = true;
                letter = NineKeyFlickPolicy.letter(downX, downY, x, y, letters, service.nineKeyFlickThreshold);
                cancelButton(event);
                button.setPressed(letter != NineKeyFlickPolicy.Letter.NONE);
                String character = NineKeyFlickPolicy.character(letters, letter);
                if (!character.isEmpty()) service.imeLetterRows.updateKeyPreviewLabel(button, character);
                return true;
            }
            case MotionEvent.ACTION_UP -> {
                if (rows.tracksLetterOptions(button) && rows.finishLetterOptions(button, event)) {
                    cancelButton(event);
                    return true;
                }
                if (!consumed) return false;
                consumed = false;
                button.setPressed(false);
                String character = NineKeyFlickPolicy.character(letters, letter);
                letter = NineKeyFlickPolicy.Letter.NONE;
                if (!character.isEmpty()) {
                    service.imeKeyFeedback.playFeedback(button);
                    service.countKey(button);
                    rows.commitNineKeyHoldOption(character);
                }
                return true;
            }
            case MotionEvent.ACTION_CANCEL -> {
                rows.cancelLetterOptions(button);
                consumed = false;
                letter = NineKeyFlickPolicy.Letter.NONE;
                button.setPressed(false);
                return false;
            }
            default -> { return consumed; }
        }
    }

    private void cancelButton(MotionEvent event) {
        MotionEvent cancel = MotionEvent.obtain(event);
        cancel.setAction(MotionEvent.ACTION_CANCEL);
        try { button.onTouchEvent(cancel); } finally { cancel.recycle(); }
    }
}

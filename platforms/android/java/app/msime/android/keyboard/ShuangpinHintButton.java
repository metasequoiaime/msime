package app.msime.android;

import android.content.Context;

/** A letter key that reserves its lower edge for a double-pinyin hint; the drawing lives in {@link KeyHintButton}. */
public final class ShuangpinHintButton extends KeyHintButton {
    public ShuangpinHintButton(Context context) {
        super(context);
    }
}

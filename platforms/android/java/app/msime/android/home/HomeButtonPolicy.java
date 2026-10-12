package app.msime.android.home;

import android.view.View;
import android.view.accessibility.AccessibilityNodeInfo;
import android.widget.Button;
import android.content.Context;
import android.widget.TextView;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;

/** Shared button construction for settings and detail cards. */
final class HomeButtonPolicy {
    private HomeButtonPolicy() {}

    static View.AccessibilityDelegate buttonDelegate(CharSequence description) {
        return new View.AccessibilityDelegate() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfo info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Button.class.getName());
                info.setContentDescription(description);
            }
        };
    }

    /** Builds a tonal pill button and gives screen readers its action description. */
    static TextView tonalButton(Context context, CharSequence label, CharSequence description, int weight) {
        TextView button = ViewPolicy.pillButton(context, label, Ui.TEXT_BUTTON_SMALL, weight,
            ThemeColorPolicy.accentSoft(context), ThemeColorPolicy.accent(context), Ui.BUTTON_PADDING_H,
            Ui.BUTTON_PADDING_V, Ui.COMPACT_BUTTON_MIN_HEIGHT, 0);
        button.setAccessibilityDelegate(buttonDelegate(description));
        ViewPolicy.setPoliteLiveRegion(button);
        return button;
    }

    static TextView tonalButton(Context context, CharSequence label, CharSequence description, int weight,
            Runnable action) {
        TextView button = tonalButton(context, label, description, weight);
        ViewPolicy.bindClick(button, action);
        return button;
    }
}

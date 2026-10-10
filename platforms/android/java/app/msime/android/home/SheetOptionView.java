package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import android.content.Context;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.FrameLayout;
import android.widget.ImageView;
import android.widget.TextView;
import androidx.core.view.AccessibilityDelegateCompat;
import androidx.core.view.ViewCompat;
import androidx.core.view.accessibility.AccessibilityNodeInfoCompat;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import app.msime.android.KeyboardGeometry;

/** Shared option-row renderer used by the settings bottom sheets. */
final class SheetOptionView {
    private SheetOptionView() {}

    static View create(Context context, CharSequence label, boolean selected, boolean nested,
            int color, boolean bold, Runnable action) {
        FrameLayout row = new FrameLayout(context);
        Ui.setMinimumHeightDp(row, context, Ui.SHEET_OPTION_HEIGHT);
        Ui.makeClickable(row, context, action);

        TextView text = Ui.centeredLabel(context, nested ? label + " ›" : label,
            Ui.TEXT_SHEET_OPTION, bold ? 600 : 400, color);
        FrameLayout.LayoutParams textParams = LayoutPolicy.frameParamsPx(
            ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT,
            Gravity.CENTER);
        textParams.leftMargin = Ui.dp(context, Ui.SHEET_OPTION_TEXT_INSET);
        textParams.rightMargin = Ui.dp(context, Ui.SHEET_OPTION_TEXT_INSET);
        textParams.topMargin = Ui.dp(context, Ui.SHEET_OPTION_TEXT_VERTICAL_INSET);
        textParams.bottomMargin = Ui.dp(context, Ui.SHEET_OPTION_TEXT_VERTICAL_INSET);
        row.addView(text, textParams);

        if (selected) {
            ImageView check = Ui.decorativeIcon(context, R.drawable.ms_w1_a2_check,
                ThemeColorPolicy.accent(context));
            FrameLayout.LayoutParams checkParams = Ui.squareFrameParams(context, Ui.SHEET_CHECK_SIZE);
            checkParams.gravity = Gravity.CENTER_VERTICAL | Gravity.END;
            checkParams.setMarginEnd(Ui.dp(context, Ui.SHEET_CHECK_END_MARGIN));
            row.addView(check, checkParams);
        }

        ViewCompat.setAccessibilityDelegate(row, new AccessibilityDelegateCompat() {
            @Override public void onInitializeAccessibilityNodeInfo(View host, AccessibilityNodeInfoCompat info) {
                super.onInitializeAccessibilityNodeInfo(host, info);
                info.setClassName(Button.class.getName());
                info.setContentDescription(nested ? label + "，更多选项" : label);
                if (selected) info.setStateDescription("已选择");
            }
        });
        return row;
    }
}

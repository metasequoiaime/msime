package app.msime.android.home;

import android.content.Context;
import android.graphics.Typeface;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.R;
import com.google.android.material.bottomsheet.BottomSheetDialog;

/**
 * 设置项的底部面板：一个标题，一列内容。
 *
 * <p>These pages are lists of settings, and a bottom sheet is what puts one in front of the user
 * without a navigation stack the shell does not have. The chrome is shared so the six entries on
 * the keyboard tab do not each grow their own layout file for a title and a scroll view.
 */
public final class SettingsSheet {
    private final BottomSheetDialog dialog;
    private final LinearLayout content;
    private final Context context;

    public SettingsSheet(Context context, String title, @Nullable String subtitle) {
        this.context = context;
        dialog = new BottomSheetDialog(context);
        LinearLayout root = new LinearLayout(context);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(20), dp(18), dp(20), dp(20));

        TextView heading = new TextView(context);
        heading.setText(title);
        // M3 headline small: the sheet's title is a heading, not a bold label.
        heading.setTextSize(22);
        heading.setTypeface(Typeface.DEFAULT);
        heading.setAccessibilityHeading(true);
        heading.setTextColor(ContextCompat.getColor(context, R.color.ink));
        root.addView(heading);

        if (subtitle != null && !subtitle.isEmpty()) {
            TextView note = new TextView(context);
            note.setText(subtitle);
            note.setTextSize(14);
            note.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            params.topMargin = dp(4);
            root.addView(note, params);
        }

        content = new LinearLayout(context);
        content.setOrientation(LinearLayout.VERTICAL);
        NestedScrollView scroll = new NestedScrollView(context);
        scroll.addView(content, new ViewGroup.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        LinearLayout.LayoutParams scrollParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        scrollParams.topMargin = dp(12);
        root.addView(scroll, scrollParams);
        dialog.setContentView(root);
    }

    /** The column every row is added to. */
    public LinearLayout content() { return content; }

    /** An M3 group title between rows: accent colour, 14sp, medium weight. */
    public void addHeading(String text) {
        TextView heading = new TextView(context);
        heading.setText(text);
        heading.setTextSize(14);
        heading.setTypeface(Typeface.create(Typeface.DEFAULT, 500, false));
        heading.setAccessibilityHeading(true);
        heading.setTextColor(ContextCompat.getColor(context, R.color.forest));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.topMargin = dp(16);
        params.bottomMargin = dp(2);
        content.addView(heading, params);
    }

    /** A footnote at the end of the column. */
    public void addNote(String text) {
        TextView note = new TextView(context);
        note.setText(text);
        note.setTextSize(12);
        note.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        params.topMargin = dp(14);
        content.addView(note, params);
    }

    /** A status line the sheet can rewrite after a save succeeds or fails. */
    public TextView addStatus() {
        TextView status = new TextView(context);
        status.setTextSize(12);
        status.setGravity(Gravity.CENTER_VERTICAL);
        status.setTextColor(ContextCompat.getColor(context, R.color.text_secondary));
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, dp(20));
        params.topMargin = dp(10);
        content.addView(status, params);
        return status;
    }

    public void add(View row) {
        content.addView(row, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
    }

    public void show() { dialog.show(); }

    public void dismiss() { dialog.dismiss(); }

    private int dp(int value) {
        return Math.round(value * context.getResources().getDisplayMetrics().density);
    }
}

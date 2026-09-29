package app.msime.client.home;

import android.content.Context;
import android.graphics.Typeface;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import app.msime.client.R;
import com.google.android.material.imageview.ShapeableImageView;

/**
 * Material 3 list rows and group titles for the host's pages.
 *
 * <p>One place for the row the design uses everywhere on Android -- leading glyph, title, value as a supporting line, no chevron -- so the settings tab, 我的 and the download page cannot drift into three slightly different rows again.
 */
final class ListRows {
    private ListRows() {}

    /** Add a row; a null action leaves it visibly disabled rather than silently dead. */
    static View add(ViewGroup parent, @DrawableRes int icon, CharSequence title,
            @Nullable CharSequence value, @Nullable Runnable action) {
        View row = LayoutInflater.from(parent.getContext())
            .inflate(R.layout.item_setting_row, parent, false);
        ShapeableImageView badge = row.findViewById(R.id.row_badge);
        badge.setImageResource(icon);
        ((TextView) row.findViewById(R.id.row_title)).setText(title);
        TextView detail = row.findViewById(R.id.row_value);
        detail.setText(value);
        detail.setVisibility(value == null || value.length() == 0 ? View.GONE : View.VISIBLE);
        row.setEnabled(action != null);
        row.setAlpha(action != null ? 1f : 0.5f);
        row.setOnClickListener(action == null ? null : ignored -> action.run());
        parent.addView(row);
        return row;
    }

    /** An M3 group title: accent colour, 14sp, medium weight, aligned with the rows' glyphs. */
    static TextView heading(ViewGroup parent, CharSequence text) {
        Context context = parent.getContext();
        TextView heading = new TextView(context);
        heading.setText(text);
        heading.setTextSize(14);
        heading.setTypeface(Typeface.create(Typeface.DEFAULT, 500, false));
        heading.setTextColor(ContextCompat.getColor(context, R.color.forest));
        heading.setAccessibilityHeading(true);
        heading.setPadding(dp(context, 24), dp(context, 16), dp(context, 24), dp(context, 4));
        parent.addView(heading, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));
        return heading;
    }

    /** The space the design puts between groups instead of a rule. */
    static void gap(ViewGroup parent) {
        View space = new View(parent.getContext());
        parent.addView(space, new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, dp(parent.getContext(), 20)));
    }

    static int dp(Context context, int value) {
        return Math.round(value * context.getResources().getDisplayMetrics().density);
    }
}

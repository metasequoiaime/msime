package app.msime.android.home;

import android.content.Context;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.Nullable;
import app.msime.android.R;
import app.msime.android.ViewPolicy;
import com.google.android.material.imageview.ShapeableImageView;

/**
 * 宿主各页共用的导航行和组标题。
 *
 * <p>设计在 Android 上到处用同一种行：前面一个线框图标、标题、下面一行当前值、没有 ›。集中在这里，设置 tab、我的、下载页就不会再长出三种略有差别的行。行的样式在 `item_setting_row.xml`，颜色全部取 M3 主题属性；放进圆角卡片时用 {@link HomeNavGroup}。
 */
final class ListRows {
    private ListRows() {}

    /** 加一行；action 为 null 时这一行显示为禁用，而不是看起来能点却没反应。 */
    static View add(ViewGroup parent, @DrawableRes int icon, CharSequence title,
            @Nullable CharSequence value, @Nullable Runnable action) {
        View row = LayoutInflater.from(parent.getContext())
            .inflate(R.layout.item_setting_row, parent, false);
        ShapeableImageView badge = row.findViewById(R.id.row_badge);
        badge.setImageResource(icon);
        ((TextView) row.findViewById(R.id.row_title)).setText(title);
        TextView detail = row.findViewById(R.id.row_value);
        detail.setText(value);
        ViewPolicy.setVisibilityForText(detail, value);
        ViewPolicy.setEnabledWithAlpha(row, action != null, 0.5f);
        ViewPolicy.bindOptionalClick(row, action);
        parent.addView(row);
        return row;
    }

    /** M3 组标题：强调色、14sp、500 字重，与行里的图标左对齐。 */
    static TextView heading(ViewGroup parent, CharSequence text) {
        Context context = parent.getContext();
        TextView heading = Ui.groupHeading(context, text);
        Ui.setPaddingDp(heading, context, Ui.NAV_ROW_PADDING_H, 16,
            Ui.NAV_ROW_PADDING_H, 4);
        parent.addView(heading, Ui.matchWidth());
        return heading;
    }

    /** 设计在两组之间留的空白，代替分隔线。 */
    static void gap(ViewGroup parent) {
        View space = new View(parent.getContext());
        parent.addView(space, Ui.matchWidthHeight(parent.getContext(), Ui.GROUP_GAP));
    }
}

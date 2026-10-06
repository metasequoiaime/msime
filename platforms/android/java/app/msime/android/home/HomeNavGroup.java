package app.msime.android.home;

import android.content.Context;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.DrawableRes;
import androidx.annotation.Nullable;
import app.msime.android.R;

/**
 * 设置首页的一组导航行：r24 的 andCard 卡片，行紧挨着排、没有分隔线；每行 60dp，24dp 线框图标、标题，当前值作为下面一行副标题，没有 ›（那是 iOS 的写法）。
 *
 * <p>行本身就是 {@link ListRows} 那种行，只是放进了圆角卡片；首页各块之间隔 12dp。
 */
public final class HomeNavGroup {
    /** 首页各块（搜索框、状态卡、各组）之间的间距。 */
    public static final int BLOCK_GAP = 12;

    private final LinearLayout card;

    private HomeNavGroup(ViewGroup parent) {
        Context context = parent.getContext();
        card = new LinearLayout(context);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setBackground(Ui.rounded(Ui.card(context), Ui.dp(context, Ui.NAV_GROUP_RADIUS)));
        // 按压波纹裁在 24dp 的圆角里。
        card.setClipToOutline(true);
        LinearLayout.LayoutParams params = Ui.matchWidth();
        if (parent.getChildCount() > 0) params.topMargin = Ui.dp(context, BLOCK_GAP);
        parent.addView(card, params);
    }

    /** 在 `parent` 末尾加一组。 */
    public static HomeNavGroup add(ViewGroup parent) {
        return new HomeNavGroup(parent);
    }

    /** 整张卡片，搜索时整组隐藏用。 */
    public LinearLayout view() { return card; }

    /** 加一行；`action` 为 null 时这一行显示为禁用。 */
    public Row add(@DrawableRes int icon, CharSequence title, @Nullable CharSequence value,
            @Nullable Runnable action) {
        return new Row(ListRows.add(card, icon, title, value, action));
    }

    /** 首页的一行；值会随设置变化，页面原地改写。 */
    public static final class Row {
        private final View view;
        private final TextView value;

        private Row(View view) {
            this.view = view;
            value = view.findViewById(R.id.row_value);
        }

        public View view() { return view; }

        public void setValue(@Nullable CharSequence text) {
            value.setText(text);
            Ui.setVisibilityForText(value, text);
        }

        public void setVisible(boolean visible) {
            view.setVisibility(visible ? View.VISIBLE : View.GONE);
        }
    }
}

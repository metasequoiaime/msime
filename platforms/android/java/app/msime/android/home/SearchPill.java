package app.msime.android.home;

import app.msime.android.DrawablePolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.text.InputType;
import android.util.AttributeSet;
import android.view.inputmethod.EditorInfo;
import android.widget.EditText;
import android.widget.ImageView;
import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import app.msime.android.R;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import java.util.function.Consumer;

/**
 * 设置首页和词库详情的搜索框：52dp 高的 andCard 胶囊，左右 18，16dp 的放大镜，与文字间 14，占位文字 16sp 次要文字色。
 */
public final class SearchPill extends LinearLayout {
    private final EditText field;

    public SearchPill(Context context) {
        this(context, null);
    }

    public SearchPill(Context context, @Nullable AttributeSet attrs) {
        super(context, attrs);
        setOrientation(HORIZONTAL);
        ViewPolicy.setCenteredVertically(this);
        Ui.setMinimumHeightDp(this, context, Ui.SEARCH_HEIGHT);
        Ui.setHorizontalPaddingDp(this, context, 18);
        setBackground(DrawablePolicy.pill(ThemeColorPolicy.card(context)));

        ImageView glyph = Ui.decorativeIcon(context, R.drawable.ic_search, ThemeColorPolicy.subText(context));
        int icon = Ui.dp(context, 16);
        addView(glyph, new LayoutParams(icon, icon));

        field = Ui.styledInput(context, Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        ViewPolicy.clearBackground(field);
        ViewPolicy.clearPadding(field);
        ViewPolicy.setSingleLine(field);
        field.setInputType(InputType.TYPE_CLASS_TEXT);
        field.setImeOptions(EditorInfo.IME_ACTION_SEARCH);
        field.setHintTextColor(ThemeColorPolicy.subText(context));
        field.setHint("搜索");
        LayoutParams params = new LayoutParams(0, Ui.dp(context, Ui.SEARCH_HEIGHT), 1f);
        params.setMarginStart(Ui.dp(context, 14));
        addView(field, params);
        // 点到胶囊的任何地方都把焦点交给输入框，而不只是那一行字。
        ViewPolicy.bindClick(this, field::requestFocus);
    }

    /** 输入框本身，需要监听回车或改样式时用。 */
    public EditText field() { return field; }

    public void setHint(CharSequence hint) {
        field.setHint(hint);
    }

    public String query() {
        return TextPolicy.trimmed(field.getText().toString());
    }

    /** 每次输入变化回调去掉首尾空白后的查询。 */
    public void setOnQueryChange(Consumer<String> listener) {
        Ui.afterTextChanged(field, ignored -> listener.accept(query()));
    }
}

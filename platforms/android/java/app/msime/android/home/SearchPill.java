package app.msime.android.home;

import app.msime.android.DimensionPolicy;

import app.msime.android.ImageViewPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.ThemeColorPolicy;
import android.content.Context;
import android.text.InputType;
import android.util.AttributeSet;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputMethodManager;
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
        ViewPolicy.setMinimumHeightDp(context, this, Ui.SEARCH_HEIGHT);
        ViewPolicy.setHorizontalPaddingDp(context, this, 18);
        setBackground(DrawablePolicy.pill(ThemeColorPolicy.card(context)));

        ImageView glyph = ImageViewPolicy.decorative(context, R.drawable.ic_search, ThemeColorPolicy.subText(context));
        int icon = DimensionPolicy.pixels(context, 16);
        addView(glyph, new LayoutParams(icon, icon));

        field = ViewPolicy.styledInput(context, Ui.TEXT_ROW_TITLE, 400, ThemeColorPolicy.text(context));
        ViewPolicy.clearBackground(field);
        ViewPolicy.clearPadding(field);
        ViewPolicy.setSingleLine(field);
        field.setInputType(InputType.TYPE_CLASS_TEXT);
        field.setImeOptions(EditorInfo.IME_ACTION_SEARCH);
        field.setHintTextColor(ThemeColorPolicy.subText(context));
        field.setHint("搜索");
        LayoutParams params = new LayoutParams(0, DimensionPolicy.pixels(context, Ui.SEARCH_HEIGHT), 1f);
        params.setMarginStart(DimensionPolicy.pixels(context, 14));
        addView(field, params);
        // 点到胶囊的任何地方（放大镜、左右留白）都和点那一行字一样：焦点交给输入框并弹出键盘。只 requestFocus 不会弹键盘，光标亮着却没法打字。
        ViewPolicy.bindClick(this, this::focusField);
    }

    private void focusField() {
        field.requestFocus();
        getContext().getSystemService(InputMethodManager.class).showSoftInput(field, 0);
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
        ViewPolicy.afterTextChanged(field, ignored -> listener.accept(query()));
    }
}

package app.msime.android.home;

import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import app.msime.android.DrawablePolicy;
import android.content.Context;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.text.Editable;
import android.text.TextWatcher;
import android.util.TypedValue;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ImageView;
import android.widget.EditText;
import android.widget.TextView;
import app.msime.android.ColorPolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.TextPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.WindowInsetsPolicy;
import androidx.annotation.ColorInt;
import androidx.annotation.DrawableRes;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.bottomsheet.BottomSheetDragHandleView;
import java.util.function.Consumer;

/**
 * 宿主界面共用的尺寸、时长和颜色读取。
 *
 * <p>数值取自设计令牌（`design-tokens.md` §2–§4、§7），单位是 dp / sp / 毫秒。颜色一律从 M3 主题属性读：属性到设计令牌的对应见实施计划 §2.1（accent = `colorPrimary`，accentSoft = `colorPrimaryContainer`，andCard = `colorSurfaceContainer`，rowBg = `colorSurfaceContainerLowest`，开关关闭轨道 = `colorSurfaceContainerHighest`，正文 / 次要文字 = `colorOnSurface` / `colorOnSurfaceVariant`，描边 / 分隔线 = `colorOutline` / `colorOutlineVariant`，危险 = `colorError`，toast = `colorSurfaceInverse` / `colorOnSurfaceInverse`）。季节主题只改这些属性的值，所以这里和各组件都不写死任何季节色。
 */
public final class Ui {
    private Ui() {}

    // ---- 尺寸（dp） ----

    /** 详情页顶栏高度，也是大标题那一行的高度。 */
    public static final int TOP_BAR_HEIGHT = 64;
    /** 顶栏返回按钮的圆形点按区。 */
    public static final int BACK_BUTTON_SIZE = 40;
    /** 返回按钮距屏幕左缘。 */
    public static final int BACK_BUTTON_START = 6;
    /** 返回箭头。 */
    public static final int BACK_ICON_SIZE = 22;
    /** 收起后的栏标题与大标题共用的起始位置，给返回按钮留出位置。 */
    public static final int TITLE_START = 64;
    /** 滚动超过这个距离，大标题收进顶栏。 */
    public static final int COLLAPSE_THRESHOLD = 28;
    /** 底部导航栏高度（不含手势区）。 */
    public static final int TAB_BAR_HEIGHT = 80;

    /** 详情页内容的左右边距。 */
    public static final int PAGE_PADDING = 16;
    /** 详情页内容顶部留白。 */
    public static final int PAGE_PADDING_TOP = 4;
    /** 详情页内容底部留白（再加底部导航栏和手势区）。 */
    public static final int PAGE_PADDING_BOTTOM = 24;
    /** 详情页两组之间的间距。 */
    public static final int GROUP_GAP = 20;
    /** 组标题相对卡片左缘的缩进。 */
    public static final int GROUP_TITLE_INSET = 8;

    /** 详情页分组卡片圆角。 */
    public static final int GROUP_RADIUS = 16;
    /** 设置首页导航组、状态卡圆角。 */
    public static final int NAV_GROUP_RADIUS = 24;
    /** 居中对话框圆角。 */
    public static final int DIALOG_RADIUS = 28;
    /** 底部面板顶角。 */
    public static final int SHEET_RADIUS = 28;

    /** 详情页行的最小高度。 */
    public static final int ROW_MIN_HEIGHT = 64;
    /** 单行无副标题时使用的紧凑详情行高度。 */
    public static final int COMPACT_ROW_MIN_HEIGHT = 52;
    /** 详情页行的左右内边距。 */
    public static final int ROW_PADDING_H = 16;
    /** 详情页行的上下内边距。 */
    public static final int ROW_PADDING_V = 8;
    /** 行标题块与行尾控件之间的间距。 */
    public static final int ROW_GAP = 14;
    /** 行尾值文字最多占行内可用宽度的比例，剩下的始终留给左侧标题块。 */
    public static final float TRAILING_VALUE_MAX_SHARE = 0.5f;
    /** 操作按钮的水平内边距。 */
    public static final int BUTTON_PADDING_H = 14;
    /** 操作按钮的垂直内边距。 */
    public static final int BUTTON_PADDING_V = 5;
    /** 紧凑操作按钮的最小高度。 */
    public static final int COMPACT_BUTTON_MIN_HEIGHT = 32;
    /** 主要操作按钮的最小高度。 */
    public static final int ACTION_BUTTON_MIN_HEIGHT = 52;
    /** 设置首页导航行的最小高度。 */
    public static final int NAV_ROW_MIN_HEIGHT = 60;
    /** 设置首页导航行的左右内边距。 */
    public static final int NAV_ROW_PADDING_H = 20;
    /** 行尾的 › 。 */
    public static final int CHEVRON_SIZE = 16;
    /** 缩略图的标准边长。 */
    public static final int THUMBNAIL_SIZE = 64;

    /** 搜索框高度。 */
    public static final int SEARCH_HEIGHT = 52;
    /** 开关轨道。 */
    public static final int SWITCH_WIDTH = 52;
    public static final int SWITCH_HEIGHT = 32;
    /** 滑块：手机上的宽度、轨道粗细、条形滑块尺寸。 */
    public static final int SLIDER_WIDTH = 110;
    public static final int SLIDER_TRACK = 6;
    public static final int SLIDER_THUMB_WIDTH = 4;
    public static final int SLIDER_THUMB_HEIGHT = 28;
    public static final int SLIDER_TOUCH_HEIGHT = 30;
    /** 滑块右边数值标签的宽度。 */
    public static final int SLIDER_LABEL_WIDTH = 46;
    /** 选择面板每个选项的高度。 */
    public static final int SHEET_OPTION_HEIGHT = 56;
    /** 选择面板选项文字为勾选图标预留的左右空间。 */
    public static final int SHEET_OPTION_TEXT_INSET = 48;
    /** 选择面板选项文字的上下内边距。 */
    public static final int SHEET_OPTION_TEXT_VERTICAL_INSET = 8;
    /** 选择面板勾选图标尺寸。 */
    public static final int SHEET_CHECK_SIZE = 18;
    /** 选择面板勾选图标距右边的间距。 */
    public static final int SHEET_CHECK_END_MARGIN = 20;
    /** 对话框的最大宽度。 */
    public static final int DIALOG_WIDTH = 280;
    /** 页码点：高度、未选中宽度、选中宽度、间距。 */
    public static final int DOT_SIZE = 6;
    public static final int DOT_ACTIVE_WIDTH = 16;
    public static final int DOT_GAP = 6;
    /** toast 下缘距内容区底部：80dp 的底部导航栏再往上 22dp（手势区另加）。 */
    public static final int TOAST_BOTTOM = 102;

    // ---- 字号（sp） ----

    public static final int TEXT_LARGE_TITLE = 28;
    public static final int TEXT_BAR_TITLE = 22;
    public static final int TEXT_ROW_TITLE = 16;
    public static final int TEXT_ROW_SUBTITLE = 14;
    public static final int TEXT_GROUP_TITLE = 14;
    public static final int TEXT_SEGMENT = 13;
    public static final int TEXT_BUTTON_SMALL = 13;
    public static final int TEXT_SHEET_HEADER = 13;
    public static final int TEXT_SHEET_OPTION = 19;
    public static final int TEXT_DIALOG_TITLE = 17;
    public static final int TEXT_TOAST = 15;

    // ---- 动效 ----

    /** M3 fade-through：新页面从 94 % 缩放淡入。 */
    public static final long FADE_THROUGH_MILLIS = 280;
    /** 顶栏底色与标题在折叠时的过渡。 */
    public static final long APP_BAR_FADE_MILLIS = 180;
    /** toast 停留时长。 */
    public static final long TOAST_MILLIS = 1600;
    /** toast 与遮罩的淡入淡出。 */
    public static final long FADE_MILLIS = 200;
    /** 页码点宽度变化。 */
    public static final long DOT_MILLIS = 200;

    // ---- 读取 ----

    public static int dp(Context context, float value) {
        return DimensionPolicy.pixels(context, value);
    }

    /** Apply the standard detail-row horizontal and vertical insets to a view. */
    public static void setRowPadding(View view, Context context) {
        int horizontal = dp(context, ROW_PADDING_H);
        int vertical = dp(context, ROW_PADDING_V);
        ViewPolicy.setPadding(view, horizontal, vertical, horizontal, vertical);
    }

    /** Apply the standard minimum height for a detail row. */
    public static void setRowMinimumHeight(View view, Context context) {
        setMinimumHeightDp(view, context, ROW_MIN_HEIGHT);
    }

    /** Set a view's minimum height from a density-independent value. */
    public static void setMinimumHeightDp(View view, Context context, float heightDp) {
        ViewPolicy.setMinimumHeight(view, dp(context, heightDp));
    }

    /** Set a text view's line-aware minimum height from a density-independent value. */
    public static void setTextMinHeightDp(TextView view, Context context, float heightDp) {
        ViewPolicy.setTextMinHeight(view, dp(context, heightDp));
    }

    /** Apply the standard compact action-button insets to a view. */
    public static void setButtonPadding(View view, Context context) {
        int horizontal = dp(context, BUTTON_PADDING_H);
        int vertical = dp(context, BUTTON_PADDING_V);
        ViewPolicy.setPadding(view, horizontal, vertical, horizontal, vertical);
    }

    /** Apply symmetric padding expressed in density-independent pixels. */
    public static void setSymmetricPaddingDp(View view, Context context,
                                             float horizontalDp, float verticalDp) {
        int horizontal = dp(context, horizontalDp);
        int vertical = dp(context, verticalDp);
        ViewPolicy.setPadding(view, horizontal, vertical, horizontal, vertical);
    }

    /** Apply equal padding on all sides when the value is already in pixels. */
    public static void setSymmetricPaddingPx(View view, int padding) {
        ViewPolicy.setPadding(view, padding, padding, padding, padding);
    }

    /** Apply equal horizontal dp padding with no vertical padding. */
    public static void setHorizontalPaddingDp(View view, Context context, float horizontalDp) {
        int horizontal = dp(context, horizontalDp);
        ViewPolicy.setPadding(view, horizontal, 0, horizontal, 0);
    }

    /** Apply four-sided padding expressed in density-independent pixels. */
    public static void setPaddingDp(View view, Context context, float leftDp, float topDp,
                                    float rightDp, float bottomDp) {
        ViewPolicy.setPadding(view, dp(context, leftDp), dp(context, topDp),
            dp(context, rightDp), dp(context, bottomDp));
    }

    /** 让页面底部避开导航栏、底部标签栏和输入法，并保留标准内容留白。 */
    public static void bindPageBottomInsets(View view) {
        Context context = view.getContext();
        int base = dp(context, PAGE_PADDING_BOTTOM);
        int tabs = dp(context, TAB_BAR_HEIGHT);
        ViewCompat.setOnApplyWindowInsetsListener(view, (target, insets) -> {
            Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars());
            Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());
            int bottom = WindowInsetsPolicy.bottomContentInset(bars.bottom, tabs, ime.bottom, base);
            ViewPolicy.setBottomPadding(target, bottom);
            return insets;
        });
        ViewCompat.requestApplyInsets(view);
    }

    /** Exclude a decorative view from the accessibility tree. */
    public static void hideFromAccessibility(View view) {
        ViewPolicy.hideFromAccessibility(view);
    }

    /** Layout parameters for a view that fills the parent width at its measured height. */
    public static LinearLayout.LayoutParams matchWidth() {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.WRAP_CONTENT);
    }

    /** Full-width layout parameters with a top margin expressed in dp. */
    public static LinearLayout.LayoutParams matchWidth(Context context, int topMarginDp) {
        LinearLayout.LayoutParams params = matchWidth();
        params.topMargin = dp(context, topMarginDp);
        return params;
    }

    /** Full-width layout parameters with a height expressed in dp. */
    public static LinearLayout.LayoutParams matchWidthHeight(Context context, int heightDp) {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            dp(context, heightDp));
    }

    /** Create a theme-coloured one-pixel divider in either orientation. */
    public static View divider(Context context, boolean horizontal) {
        View view = ViewPolicy.newColorView(context, ThemeColorPolicy.hairline(context));
        int thin = DimensionPolicy.atLeastOnePixel(context, 0.5f);
        view.setLayoutParams(horizontal
            ? LayoutPolicy.matchWidthHeightPx(thin)
            : new LinearLayout.LayoutParams(thin, ViewGroup.LayoutParams.MATCH_PARENT));
        return view;
    }

    /** Create the page-coloured separation band used between sheet options and the cancel row. */
    public static View sheetSeparator(Context context) {
        View view = new View(context);
        ViewPolicy.setBackgroundColor(view, ThemeColorPolicy.page(context));
        view.setLayoutParams(matchWidthHeight(context, 8));
        return view;
    }

    /** Create the full-width Material bottom-sheet drag handle. */
    public static BottomSheetDragHandleView sheetDragHandle(Context context) {
        BottomSheetDragHandleView handle = new BottomSheetDragHandleView(context);
        handle.setLayoutParams(matchWidth());
        return handle;
    }

    /** Create a vertical linear container for stacked host content. */
    public static LinearLayout column(Context context) {
        return ViewPolicy.newColumn(context);
    }

    /** Create a horizontal linear container for inline host content. */
    public static LinearLayout row(Context context) {
        return ViewPolicy.newRow(context);
    }

    /** Layout parameters for a weighted child that wraps its height. */
    public static LinearLayout.LayoutParams weightWrap(float weight) {
        return new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, weight);
    }

    /** Layout parameters for a view that wraps both its content dimensions. */
    public static LinearLayout.LayoutParams wrap() {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT);
    }

    /** Layout parameters for a trailing row control with the standard row gap. */
    public static LinearLayout.LayoutParams rowGapParams(Context context) {
        LinearLayout.LayoutParams params = wrap();
        params.setMarginStart(dp(context, ROW_GAP));
        return params;
    }

    /** Layout parameters for a square child with a size expressed in dp. */
    public static LinearLayout.LayoutParams squareParams(Context context, float sizeDp) {
        int size = dp(context, sizeDp);
        return new LinearLayout.LayoutParams(size, size);
    }

    /** Frame layout parameters for a square child with a size expressed in dp. */
    public static FrameLayout.LayoutParams squareFrameParams(Context context, float sizeDp) {
        int size = dp(context, sizeDp);
        return new FrameLayout.LayoutParams(size, size);
    }

    /** Frame layout parameters for a pixel-sized square with explicit gravity. */
    public static FrameLayout.LayoutParams squareFrameParamsPx(int size, int gravity) {
        return new FrameLayout.LayoutParams(size, size, gravity);
    }

    /** 主题的按压反馈（`selectableItemBackground`），行在代码里构造时用它。 */
    @androidx.annotation.Nullable public static Drawable ripple(Context context) {
        TypedValue value = new TypedValue();
        if (!context.getTheme().resolveAttribute(androidx.appcompat.R.attr.selectableItemBackground, value, true)) return null;
        return androidx.core.content.ContextCompat.getDrawable(context, value.resourceId);
    }

    /** 有底色的按压反馈：底色画在波纹下面，波纹裁在 `radiusPx` 的圆角里；底色透明时波纹照样可见。 */
    /** Create a pill-shaped press ripple with a fully rounded mask. */
    public static Drawable pillRipple(Context context, @ColorInt int fill) {
        return rippleOn(context, fill, 9999f);
    }

    public static Drawable rippleOn(Context context, @ColorInt int fill, float radiusPx) {
        int pressed = ColorPolicy.withAlpha(ThemeColorPolicy.text(context), 0.10f);
        return DrawablePolicy.ripple(pressed, DrawablePolicy.rounded(fill, radiusPx),
            DrawablePolicy.rounded(Color.WHITE, radiusPx));
    }

    /** Create a text label with the supplied text, size in sp, and colour. */
    public static TextView label(Context context, CharSequence text, float sizeSp, @ColorInt int color) {
        TextView view = new TextView(context);
        view.setText(text);
        ViewPolicy.setTextSizeSp(view, sizeSp);
        ViewPolicy.setTextColor(view, color);
        return view;
    }

    /** Create a text label with the shared size, weight, and colour policy. */
    public static TextView styledLabel(Context context, CharSequence text, int sizeSp, int weight,
                                       @ColorInt int color) {
        TextView view = new TextView(context);
        view.setText(text);
        style(view, sizeSp, weight, color);
        return view;
    }

    /** 创建带字重的单行标签。 */
    public static TextView singleLineLabel(Context context, CharSequence text, int sizeSp, int weight,
                                           @ColorInt int color) {
        TextView singleLine = styledLabel(context, text, sizeSp, weight, color);
        ViewPolicy.setSingleLine(singleLine);
        return singleLine;
    }

    /** 创建双轴居中的普通标签。 */
    public static TextView centeredLabel(Context context, CharSequence text, float sizeSp,
                                         @ColorInt int color) {
        TextView centered = label(context, text, sizeSp, color);
        ViewPolicy.setCentered(centered);
        return centered;
    }

    /** 创建带字重且双轴居中的标签。 */
    public static TextView centeredLabel(Context context, CharSequence text, int sizeSp, int weight,
                                         @ColorInt int color) {
        TextView centered = styledLabel(context, text, sizeSp, weight, color);
        ViewPolicy.setCentered(centered);
        return centered;
    }

    /** 创建带字重、双轴居中的单行标签。 */
    public static TextView centeredSingleLineLabel(Context context, CharSequence text, int sizeSp,
                                                   int weight, @ColorInt int color) {
        TextView singleLine = centeredLabel(context, text, sizeSp, weight, color);
        ViewPolicy.setSingleLine(singleLine);
        return singleLine;
    }

    /** 创建带无障碍标题语义的普通标签。 */
    public static TextView headingLabel(Context context, CharSequence text, float sizeSp,
                                        @ColorInt int color) {
        TextView heading = label(context, text, sizeSp, color);
        heading.setAccessibilityHeading(true);
        return heading;
    }

    /** 创建带字重与无障碍标题语义的标签。 */
    public static TextView headingLabel(Context context, CharSequence text, int sizeSp, int weight,
                                        @ColorInt int color) {
        TextView heading = styledLabel(context, text, sizeSp, weight, color);
        heading.setAccessibilityHeading(true);
        return heading;
    }

    /** 创建会由辅助功能礼貌播报变化的空状态文本。 */
    public static TextView liveStatus(Context context, int sizeSp) {
        TextView status = styledLabel(context, "", sizeSp, 400, ThemeColorPolicy.subText(context));
        ViewPolicy.setPoliteLiveRegion(status);
        return status;
    }

    /**
     * 行尾的值文字：单行，最宽只占行内可用宽度的 {@link #TRAILING_VALUE_MAX_SHARE}，再长就在末尾省略。
     *
     * <p>设置行是横向 LinearLayout，左侧标题块按权重分剩余宽度，而不带权重的行尾值先量、要多宽给多宽：值一长（例如完整的端点 URL），标题块就被挤得只剩一两个字宽，竖着折成很多行（#5790）。宽度在这里按父布局给的上限封顶，不靠各页面自己先把值截短。
     */
    public static TextView trailingValue(Context context, @androidx.annotation.Nullable CharSequence text,
                                         int sizeSp, @ColorInt int color) {
        TextView view = new TextView(context) {
            @Override protected void onMeasure(int widthMeasureSpec, int heightMeasureSpec) {
                if (View.MeasureSpec.getMode(widthMeasureSpec) != View.MeasureSpec.UNSPECIFIED) {
                    int limit = Math.round(View.MeasureSpec.getSize(widthMeasureSpec) * TRAILING_VALUE_MAX_SHARE);
                    widthMeasureSpec = View.MeasureSpec.makeMeasureSpec(limit, View.MeasureSpec.AT_MOST);
                }
                super.onMeasure(widthMeasureSpec, heightMeasureSpec);
            }
        };
        view.setText(text);
        style(view, sizeSp, 400, color);
        ViewPolicy.setSingleLineEllipsized(view);
        return view;
    }

    /** Create an editable field with the shared size, weight, and colour policy. */
    public static EditText styledInput(Context context, int sizeSp, int weight, @ColorInt int color) {
        EditText view = new EditText(context);
        style(view, sizeSp, weight, color);
        return view;
    }

    /** 只在文字修改完成后回调，省去不需要的另外两个 `TextWatcher` 方法。 */
    public static void afterTextChanged(TextView view, Consumer<Editable> listener) {
        view.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

            @Override public void afterTextChanged(Editable text) { listener.accept(text); }
        });
    }

    /** Create the standard accent-coloured group heading. */
    public static TextView groupHeading(Context context, CharSequence text) {
        return headingLabel(context, text, TEXT_GROUP_TITLE, 500, ThemeColorPolicy.accent(context));
    }

    /** Create a filled accent pill button; callers add their content description and action. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      @ColorInt int ink, float horizontalPaddingDp, float verticalPaddingDp,
                                      float minHeightDp, float minWidthDp) {
        return pillButton(context, label, sizeSp, weight, ThemeColorPolicy.accent(context), ink,
            horizontalPaddingDp, verticalPaddingDp, minHeightDp, minWidthDp, null);
    }

    /** Create a filled accent pill button and bind its action. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      @ColorInt int ink, float horizontalPaddingDp, float verticalPaddingDp,
                                      float minHeightDp, float minWidthDp, Runnable action) {
        return pillButton(context, label, sizeSp, weight, ThemeColorPolicy.accent(context), ink,
            horizontalPaddingDp, verticalPaddingDp, minHeightDp, minWidthDp, action);
    }

    /** Create a filled accent pill button with an explicit fill colour. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      @ColorInt int fill, @ColorInt int ink,
                                      float horizontalPaddingDp, float verticalPaddingDp,
                                      float minHeightDp, float minWidthDp) {
        return pillButton(context, label, sizeSp, weight, fill, ink, horizontalPaddingDp, verticalPaddingDp,
            minHeightDp, minWidthDp, null);
    }

    /** Create a pill button with an explicit fill colour and bind its action. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      @ColorInt int fill, @ColorInt int ink,
                                      float horizontalPaddingDp, float verticalPaddingDp,
                                      float minHeightDp, float minWidthDp, Runnable action) {
        TextView button = centeredSingleLineLabel(context, label, Math.round(sizeSp), weight, ink);
        ViewPolicy.setBackground(button, pillRipple(context, fill));
        setSymmetricPaddingDp(button, context, horizontalPaddingDp, verticalPaddingDp);
        setTextMinHeightDp(button, context, minHeightDp);
        if (minWidthDp > 0) ViewPolicy.setTextMinWidth(button, dp(context, minWidthDp));
        bindClick(button, action);
        return button;
    }

    /** Create a centered, clickable text button with caller-supplied background and ink. */
    public static TextView textButton(Context context, CharSequence label, int sizeSp, int weight,
                                      @ColorInt int ink, Drawable background, float minHeightDp) {
        return textButton(context, label, sizeSp, weight, ink, background, minHeightDp, null);
    }

    /** Create a centered text button and bind its action. */
    public static TextView textButton(Context context, CharSequence label, int sizeSp, int weight,
                                      @ColorInt int ink, Drawable background, float minHeightDp,
                                      Runnable action) {
        TextView button = centeredLabel(context, label, sizeSp, weight, ink);
        ViewPolicy.setBackground(button, background);
        setTextMinHeightDp(button, context, minHeightDp);
        bindClick(button, action);
        return button;
    }

    /** Create a square, centered icon button with the standard detail-page touch target. */
    public static ImageView iconButton(Context context, int icon, @ColorInt int tint,
                                       CharSequence description, float sizeDp, Runnable action) {
        return iconButton(context, context.getDrawable(icon), tint, description, sizeDp, action);
    }

    /** Create an icon button from a runtime drawable with the standard detail-page touch target. */
    public static ImageView iconButton(Context context, Drawable icon, @ColorInt int tint,
                                       CharSequence description, float sizeDp, Runnable action) {
        ImageView button = new ImageView(context);
        button.setImageDrawable(icon);
        ImageViewPolicy.setTint(button, tint);
        button.setScaleType(ImageView.ScaleType.CENTER);
        ViewPolicy.setBackground(button, ripple(context));
        button.setContentDescription(description);
        bindClick(button, action);
        int size = dp(context, sizeDp);
        button.setLayoutParams(ViewPolicy.newSquareParamsPx(size));
        setSymmetricPaddingPx(button, size / 5);
        return button;
    }

    /** Create a non-interactive, accessibility-hidden image tinted for a surrounding surface. */
    public static ImageView decorativeIcon(Context context, @DrawableRes int icon,
                                           @ColorInt int tint) {
        ImageView view = new ImageView(context);
        view.setImageResource(icon);
        ImageViewPolicy.setTint(view, tint);
        hideFromAccessibility(view);
        return view;
    }

    /** Create a decorative image without applying a tint. */
    public static ImageView decorativeIcon(Context context, @DrawableRes int icon) {
        ImageView view = new ImageView(context);
        view.setImageResource(icon);
        hideFromAccessibility(view);
        return view;
    }

    /** Create a decorative image from a runtime drawable without applying a tint. */
    public static ImageView decorativeIcon(Context context, Drawable icon) {
        ImageView view = new ImageView(context);
        view.setImageDrawable(icon);
        hideFromAccessibility(view);
        return view;
    }

    /** 去掉名称首尾空白后返回第一个 Unicode 码点；名称为空时返回后备值。 */
    public static String trimmedInitial(CharSequence name, String fallback) {
        String trimmed = TextPolicy.trimmed(name == null ? null : name.toString());
        return TextPolicy.initial(trimmed, fallback);
    }

    /** Create the muted, accessibility-hidden chevron used by navigable rows. */
    public static ImageView chevron(Context context) {
        ImageView view = new ImageView(context);
        view.setImageResource(app.msime.android.R.drawable.ms_w1_a2_chevron);
        ImageViewPolicy.setTint(view, ThemeColorPolicy.subText(context));
        ViewPolicy.hideFromAccessibility(view);
        return view;
    }

    /** Apply the standard ripple and keyboard-accessible click behavior to a view. */
    public static void makeClickable(View view, Context context, Runnable action) {
        ViewPolicy.setBackground(view, ripple(context));
        bindClick(view, action);
    }

    private static void bindClick(View view, Runnable action) {
        ViewPolicy.setInteractive(view, true);
        ViewPolicy.bindOptionalClick(view, action);
    }

    /** Create a vertically arranged rounded surface for page cards. */
    public static LinearLayout verticalCard(Context context, float radiusDp) {
        LinearLayout card = column(context);
        ViewPolicy.setBackground(card,
            DrawablePolicy.rounded(ThemeColorPolicy.card(context), dp(context, radiusDp)));
        return card;
    }

    /** 设置字号（sp）与字重。 */
    public static void style(TextView view, int sizeSp, int weight, @ColorInt int color) {
        ViewPolicy.setTextSizeSp(view, sizeSp);
        ViewPolicy.setTypefaceWeight(view, weight);
        ViewPolicy.setTextColor(view, color);
    }

}

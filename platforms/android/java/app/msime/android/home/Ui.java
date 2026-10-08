package app.msime.android.home;

import app.msime.android.DrawablePolicy;
import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.util.TypedValue;
import android.view.View;
import android.view.ViewGroup;
import android.view.animation.PathInterpolator;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ImageView;
import android.widget.EditText;
import android.widget.TextView;
import app.msime.android.BoundsPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.TextPolicy;
import app.msime.android.ViewPolicy;
import androidx.annotation.AttrRes;
import androidx.annotation.ColorInt;
import app.msime.android.ColorPolicy;
import androidx.annotation.DrawableRes;
import com.google.android.material.color.MaterialColors;
import com.google.android.material.bottomsheet.BottomSheetDragHandleView;

/**
 * 宿主界面共用的尺寸、时长和颜色读取。
 *
 * <p>数值取自设计令牌（`design-tokens.md` §2–§4、§7），单位是 dp / sp / 毫秒。颜色一律从 M3 主题属性读：属性到设计令牌的对应见实施计划 §2.1（accent = `colorPrimary`，accentSoft = `colorPrimaryContainer`，andCard = `colorSurfaceContainer`，rowBg = `colorSurfaceContainerLowest`，开关关闭轨道 = `colorSurfaceContainerHighest`，正文 / 次要文字 = `colorOnSurface` / `colorOnSurfaceVariant`，描边 / 分隔线 = `colorOutline` / `colorOutlineVariant`，危险 = `colorError`，toast = `colorSurfaceInverse` / `colorOnSurfaceInverse`）。季节主题只改这些属性的值，所以这里和各组件都不写死任何季节色。
 */
public final class Ui {
    private Ui() {}

    /** Bottom content inset that keeps page content above either system navigation or the IME. */
    public static int bottomContentInset(int systemBottom, int tabs, int imeBottom, int base) {
        return BoundsPolicy.atLeast(systemBottom + tabs, imeBottom) + base;
    }

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

    private static final class MotionCurves {
        static final PathInterpolator EMPHASIZED = new PathInterpolator(0.2f, 0f, 0f, 1f);
    }

    /** 设计的动效曲线 `cubic-bezier(.2, 0, 0, 1)`。 */
    public static PathInterpolator emphasized() { return MotionCurves.EMPHASIZED; }

    // ---- 读取 ----

    public static int dp(Context context, float value) {
        return KeyboardGeometry.pixels(context, value);
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
        view.setMinimumHeight(dp(context, heightDp));
    }

    /** Set a text view's line-aware minimum height from a density-independent value. */
    public static void setTextMinHeightDp(TextView view, Context context, float heightDp) {
        view.setMinHeight(dp(context, heightDp));
    }

    /** Set a text view's line-aware minimum width from a density-independent value. */
    public static void setTextMinWidthDp(TextView view, Context context, float widthDp) {
        view.setMinWidth(dp(context, widthDp));
    }

    /** Apply the standard compact action-button insets to a view. */
    public static void setButtonPadding(View view, Context context) {
        int horizontal = dp(context, BUTTON_PADDING_H);
        int vertical = dp(context, BUTTON_PADDING_V);
        ViewPolicy.setPadding(view, horizontal, vertical, horizontal, vertical);
    }

    /** Apply the shared bottom-sheet title-area insets to a view. */
    public static void setSheetHeaderPadding(View view, Context context) {
        int horizontal = dp(context, 16);
        ViewPolicy.setPadding(view, horizontal, 0, horizontal, dp(context, 12));
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

    /** Apply equal horizontal padding when the value is already in pixels. */
    public static void setHorizontalPaddingPx(View view, int horizontal) {
        ViewPolicy.setPadding(view, horizontal, 0, horizontal, 0);
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

    /** Replace only the bottom padding while preserving the other three sides. */
    public static void setBottomPadding(View view, int bottomPixels) {
        ViewPolicy.setPadding(view, view.getPaddingLeft(), view.getPaddingTop(), view.getPaddingRight(),
            bottomPixels);
    }

    /** Show a view only when the supplied text is non-null and non-empty. */
    public static void setVisibilityForText(View view, CharSequence text) {
        ViewPolicy.setVisibilityForText(view, text);
    }

    /** Apply a single tint to an image view through the platform state-list wrapper. */
    public static void setImageTint(ImageView view, int color) {
        view.setImageTintList(ColorStateList.valueOf(color));
    }

    /** Exclude a decorative view from the accessibility tree. */
    public static void hideFromAccessibility(View view) {
        ViewPolicy.hideFromAccessibility(view);
    }

    /** Return whether the supplied context currently uses the system night configuration. */
    public static boolean isNight(Context context) {
        return KeyboardGeometry.isNight(context);
    }

    /** Return the current display width in physical pixels. */
    public static int screenWidthPixels(Context context) {
        return KeyboardGeometry.screenWidthPixels(context);
    }

    /** Convert a density-independent dimension without rounding, for canvas geometry. */
    public static float dpFloat(Context context, float value) {
        return KeyboardGeometry.floatPixels(context, value);
    }

    /** Convert scalable text units to pixels using the context display metrics. */
    public static float sp(Context context, float value) {
        return KeyboardGeometry.sp(context, value);
    }

    /** Parse a theme or skin colour, returning the supplied fallback for missing or invalid input. */
    public static int parseColor(String value, int fallback) {
        return ColorPolicy.parse(value, fallback);
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

    /** Full-width layout parameters with an already pixel-sized height. */
    public static LinearLayout.LayoutParams matchWidthHeightPx(int heightPixels) {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, heightPixels);
    }

    /** Convert a density-independent size while guaranteeing at least one physical pixel. */
    public static int atLeastOnePx(Context context, float value) {
        return KeyboardGeometry.atLeastOnePixel(context, value);
    }

    /** Return the minimum one-pixel thickness for a 0.5 dp separator. */
    public static int hairlinePx(Context context) {
        return KeyboardGeometry.atLeastOnePixel(context, 0.5f);
    }

    /** Create a view filled with the standard hairline colour for separators. */
    public static View hairlineView(Context context) {
        View view = new View(context);
        ViewPolicy.setBackgroundColor(view, hairline(context));
        return view;
    }

    /** Create a theme-coloured one-pixel divider in either orientation. */
    public static View divider(Context context, boolean horizontal) {
        View view = hairlineView(context);
        int thin = hairlinePx(context);
        view.setLayoutParams(horizontal
            ? matchWidthHeightPx(thin)
            : new LinearLayout.LayoutParams(thin, ViewGroup.LayoutParams.MATCH_PARENT));
        return view;
    }

    /** Create the page-coloured separation band used between sheet options and the cancel row. */
    public static View sheetSeparator(Context context) {
        View view = new View(context);
        ViewPolicy.setBackgroundColor(view, page(context));
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
        LinearLayout view = new LinearLayout(context);
        view.setOrientation(LinearLayout.VERTICAL);
        return view;
    }

    /** Create a horizontal linear container for inline host content. */
    public static LinearLayout row(Context context) {
        LinearLayout view = new LinearLayout(context);
        view.setOrientation(LinearLayout.HORIZONTAL);
        return view;
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

    /** Layout parameters for a view that wraps its width and uses a dp height. */
    public static LinearLayout.LayoutParams wrapHeight(Context context, float heightDp) {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT,
            dp(context, heightDp));
    }

    /** Layout parameters for a square child with a size expressed in dp. */
    public static LinearLayout.LayoutParams squareParams(Context context, float sizeDp) {
        int size = dp(context, sizeDp);
        return new LinearLayout.LayoutParams(size, size);
    }

    /** Layout parameters for a square child when its size is already in pixels. */
    public static LinearLayout.LayoutParams squareParamsPx(int size) {
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

    /** Frame layout parameters for a child that fills width and uses a dp height. */
    public static FrameLayout.LayoutParams frameMatchWidthHeight(Context context, float heightDp) {
        return new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            dp(context, heightDp));
    }

    /** Frame layout parameters for a content-sized child with explicit gravity. */
    public static FrameLayout.LayoutParams frameWrap(int gravity) {
        return new FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT, gravity);
    }

    /** Layout parameters for a weighted child with a fixed height in dp. */
    public static LinearLayout.LayoutParams weightedHeight(Context context, float heightDp, float weight) {
        return new LinearLayout.LayoutParams(0, dp(context, heightDp), weight);
    }

    /** Layout parameters for a weighted child that fills the parent's height. */
    public static LinearLayout.LayoutParams weightedMatchParent(float weight) {
        return new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, weight);
    }

    /** Layout parameters for a weighted child that fills the parent's width. */
    public static LinearLayout.LayoutParams weightedWidth(float weight) {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, weight);
    }

    /** 读一个颜色主题属性；属性缺失时退回洋红，让漏配的属性在截图里一眼可见，而不是悄悄显示成别的颜色。 */
    @ColorInt public static int color(Context context, @AttrRes int attr) {
        return MaterialColors.getColor(context, attr, Color.MAGENTA);
    }

    @ColorInt public static int accent(Context context) {
        return color(context, androidx.appcompat.R.attr.colorPrimary);
    }

    @ColorInt public static int onAccent(Context context) {
        return color(context, com.google.android.material.R.attr.colorOnPrimary);
    }

    @ColorInt public static int accentSoft(Context context) {
        return color(context, com.google.android.material.R.attr.colorPrimaryContainer);
    }

    @ColorInt public static int card(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurfaceContainer);
    }

    @ColorInt public static int rowBackground(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurfaceContainerLowest);
    }

    @ColorInt public static int sheetBackground(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurfaceContainerLow);
    }

    @ColorInt public static int page(Context context) {
        return color(context, com.google.android.material.R.attr.colorSurface);
    }

    @ColorInt public static int text(Context context) {
        return color(context, com.google.android.material.R.attr.colorOnSurface);
    }

    @ColorInt public static int subText(Context context) {
        return color(context, com.google.android.material.R.attr.colorOnSurfaceVariant);
    }

    @ColorInt public static int outline(Context context) {
        return color(context, com.google.android.material.R.attr.colorOutline);
    }

    @ColorInt public static int hairline(Context context) {
        return color(context, com.google.android.material.R.attr.colorOutlineVariant);
    }

    @ColorInt public static int danger(Context context) {
        return color(context, androidx.appcompat.R.attr.colorError);
    }

    /** 按 0–1 的不透明度改写颜色的 alpha，乘在原有 alpha 上。 */
    @ColorInt public static int withAlpha(@ColorInt int color, float alpha) {
        return ColorPolicy.withAlpha(color, alpha);
    }

    /** Create a filled circular drawable. */
    public static GradientDrawable circle(@ColorInt int color) {
        return DrawablePolicy.circle(color);
    }

    /** Filled circle with a visible outline. */
    public static GradientDrawable circleOutlined(@ColorInt int fillColor, int strokeWidth,
                                                  @ColorInt int strokeColor) {
        return DrawablePolicy.circleOutlined(fillColor, strokeWidth, strokeColor);
    }

    /** 纯色圆角矩形。 */
    public static GradientDrawable rounded(@ColorInt int color, float radiusPx) {
        return DrawablePolicy.rounded(color, radiusPx);
    }

    /** Filled rounded rectangle with a theme-aware outline. */
    public static GradientDrawable outlined(@ColorInt int fillColor, float radiusPx,
                                            int strokeWidth, @ColorInt int strokeColor) {
        return DrawablePolicy.outlined(fillColor, radiusPx, strokeWidth, strokeColor);
    }

    /** Filled rounded rectangle with a dashed outline. */
    public static GradientDrawable outlinedDashed(@ColorInt int fillColor, float radiusPx,
                                                  int strokeWidth, @ColorInt int strokeColor,
                                                  float dashWidth, float dashGap) {
        return DrawablePolicy.outlinedDashed(fillColor, radiusPx, strokeWidth, strokeColor,
            dashWidth, dashGap);
    }

    /** 胶囊形状：GradientDrawable 会把过大的圆角夹到短边的一半，所以高度怎么变两端都是半圆。 */
    public static GradientDrawable pill(@ColorInt int color) {
        return rounded(color, 9999f);
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
        int pressed = withAlpha(text(context), 0.10f);
        return new RippleDrawable(ColorStateList.valueOf(pressed), rounded(fill, radiusPx),
            rounded(Color.WHITE, radiusPx));
    }

    /** 沿着 ContextWrapper 链找到所在的 Activity；不在任何 Activity 里时返回 null。 */
    @androidx.annotation.Nullable public static android.app.Activity activityOf(Context context) {
        Context current = context;
        while (current instanceof android.content.ContextWrapper wrapper) {
            if (current instanceof android.app.Activity activity) return activity;
            current = wrapper.getBaseContext();
        }
        return null;
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

    /** Apply the shared completion or warning mark used by setup checks. */
    public static void applyStatusMark(TextView mark, Context context, boolean done) {
        mark.setText(done ? "✓" : "!");
        ViewPolicy.setTextColor(mark, done ? onAccent(context) : 0xFFFFFFFF);
        ViewPolicy.setBackground(mark, circle(done ? accent(context) : color(context, app.msime.android.R.attr.msWarn)));
        ViewPolicy.hideFromAccessibility(mark);
    }

    /** Create the centered title used by option-style bottom sheets. */
    public static TextView sheetHeading(Context context, CharSequence text) {
        TextView heading = new TextView(context);
        heading.setText(text);
        ViewPolicy.setCentered(heading);
        style(heading, TEXT_SHEET_HEADER, 600, subText(context));
        heading.setAccessibilityHeading(true);
        return heading;
    }

    /** Create the centered subtitle used by option-style bottom sheets. */
    public static TextView sheetSubtitle(Context context, CharSequence text) {
        TextView subtitle = new TextView(context);
        subtitle.setText(text);
        ViewPolicy.setCentered(subtitle);
        style(subtitle, TEXT_SHEET_HEADER, 400, subText(context));
        return subtitle;
    }

    /** Create the standard accent-coloured group heading. */
    public static TextView groupHeading(Context context, CharSequence text) {
        TextView heading = new TextView(context);
        heading.setText(text);
        style(heading, TEXT_GROUP_TITLE, 500, accent(context));
        heading.setAccessibilityHeading(true);
        return heading;
    }

    /** Create a filled accent pill button; callers add their content description and action. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      @ColorInt int ink, float horizontalPaddingDp, float verticalPaddingDp,
                                      float minHeightDp, float minWidthDp) {
        return pillButton(context, label, sizeSp, weight, accent(context), ink,
            horizontalPaddingDp, verticalPaddingDp, minHeightDp, minWidthDp, null);
    }

    /** Create a filled accent pill button and bind its action. */
    public static TextView pillButton(Context context, CharSequence label, float sizeSp, int weight,
                                      @ColorInt int ink, float horizontalPaddingDp, float verticalPaddingDp,
                                      float minHeightDp, float minWidthDp, Runnable action) {
        return pillButton(context, label, sizeSp, weight, accent(context), ink,
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
        TextView button = new TextView(context);
        button.setText(label);
        ViewPolicy.setCentered(button);
        ViewPolicy.setSingleLine(button);
        style(button, Math.round(sizeSp), weight, ink);
        ViewPolicy.setBackground(button, pillRipple(context, fill));
        setSymmetricPaddingDp(button, context, horizontalPaddingDp, verticalPaddingDp);
        setTextMinHeightDp(button, context, minHeightDp);
        if (minWidthDp > 0) setTextMinWidthDp(button, context, minWidthDp);
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
        TextView button = new TextView(context);
        button.setText(label);
        ViewPolicy.setCentered(button);
        style(button, sizeSp, weight, ink);
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
        setImageTint(button, tint);
        button.setScaleType(ImageView.ScaleType.CENTER);
        ViewPolicy.setBackground(button, ripple(context));
        button.setContentDescription(description);
        bindClick(button, action);
        int size = dp(context, sizeDp);
        button.setLayoutParams(squareParamsPx(size));
        setSymmetricPaddingPx(button, size / 5);
        return button;
    }

    /** Create a non-interactive, accessibility-hidden image tinted for a surrounding surface. */
    public static ImageView decorativeIcon(Context context, @DrawableRes int icon,
                                           @ColorInt int tint) {
        ImageView view = new ImageView(context);
        view.setImageResource(icon);
        setImageTint(view, tint);
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

    /** Return the first Unicode code point of a name, or the caller's fallback when empty. */
    public static String initial(CharSequence name, String fallback) {
        if (name == null || name.length() == 0) return fallback;
        return new String(Character.toChars(Character.codePointAt(name, 0)));
    }

    /** Return the first Unicode code point after trimming a name, or the fallback when empty. */
    public static String trimmedInitial(CharSequence name, String fallback) {
        String trimmed = TextPolicy.trimmed(name == null ? null : name.toString());
        return initial(trimmed, fallback);
    }

    /** Whether the optional Tauri management activity is present in this APK. */
    public static boolean tauriAvailable() {
        try {
            Class.forName("app.msime.android.MainActivity");
            return true;
        } catch (ClassNotFoundException absent) {
            return false;
        }
    }

    /** Create the muted, accessibility-hidden chevron used by navigable rows. */
    public static ImageView chevron(Context context) {
        ImageView view = new ImageView(context);
        view.setImageResource(app.msime.android.R.drawable.ms_w1_a2_chevron);
        setImageTint(view, subText(context));
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
        ViewPolicy.setBackground(card, rounded(card(context), dp(context, radiusDp)));
        return card;
    }

    /** 设置字号（sp）与字重。 */
    public static void style(TextView view, int sizeSp, int weight, @ColorInt int color) {
        ViewPolicy.setTextSizeSp(view, sizeSp);
        ViewPolicy.setTypefaceWeight(view, weight);
        ViewPolicy.setTextColor(view, color);
    }

    /** 把一个 view 的透明度和可点按状态一起切换；禁用的行仍然可见，只是变淡且不响应。 */
    public static void setEnabledLook(View view, boolean enabled) {
        setEnabledLook(view, enabled, 0.38f);
    }

    /** Apply enabled state and a caller-selected inactive opacity to a home control. */
    public static void setEnabledLook(View view, boolean enabled, float inactiveAlpha) {
        ViewPolicy.setEnabledWithAlpha(view, enabled, inactiveAlpha);
    }
}

package app.msime.android.home;

import android.content.Context;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.util.TypedValue;
import android.view.View;
import android.view.animation.PathInterpolator;
import android.widget.TextView;
import androidx.annotation.AttrRes;
import androidx.annotation.ColorInt;
import com.google.android.material.color.MaterialColors;

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
    /** 详情页行的左右内边距。 */
    public static final int ROW_PADDING_H = 16;
    /** 详情页行的上下内边距。 */
    public static final int ROW_PADDING_V = 8;
    /** 行标题块与行尾控件之间的间距。 */
    public static final int ROW_GAP = 14;
    /** 设置首页导航行的最小高度。 */
    public static final int NAV_ROW_MIN_HEIGHT = 60;
    /** 设置首页导航行的左右内边距。 */
    public static final int NAV_ROW_PADDING_H = 20;
    /** 行尾的 › 。 */
    public static final int CHEVRON_SIZE = 16;

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

    /** 设计的动效曲线 `cubic-bezier(.2, 0, 0, 1)`。 */
    public static PathInterpolator emphasized() { return new PathInterpolator(0.2f, 0f, 0f, 1f); }

    // ---- 读取 ----

    public static int dp(Context context, float value) {
        return Math.round(value * context.getResources().getDisplayMetrics().density);
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
        int base = Color.alpha(color);
        return (color & 0x00FFFFFF) | (Math.round(base * alpha) << 24);
    }

    /** 纯色圆角矩形。 */
    public static GradientDrawable rounded(@ColorInt int color, float radiusPx) {
        GradientDrawable shape = new GradientDrawable();
        shape.setColor(color);
        shape.setCornerRadius(radiusPx);
        return shape;
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

    /** 设置字号（sp）与字重。 */
    public static void style(TextView view, int sizeSp, int weight, @ColorInt int color) {
        view.setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp);
        view.setTypeface(Typeface.create(Typeface.DEFAULT, weight, false));
        view.setTextColor(color);
    }

    /** 把一个 view 的透明度和可点按状态一起切换；禁用的行仍然可见，只是变淡且不响应。 */
    public static void setEnabledLook(View view, boolean enabled) {
        view.setEnabled(enabled);
        view.setAlpha(enabled ? 1f : 0.38f);
    }
}

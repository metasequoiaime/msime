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
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.ImageView;
import android.widget.TextView;
import app.msime.android.ColorPolicy;
import app.msime.android.ImageViewPolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.TextPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.WindowInsetsPolicy;
import androidx.annotation.ColorInt;
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
    /** 滑块右边数值标签的最小宽度；较长数值按当前字体测量后扩宽。 */
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


}

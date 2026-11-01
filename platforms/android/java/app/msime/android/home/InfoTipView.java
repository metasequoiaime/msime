package app.msime.android.home;

import android.content.Context;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.InsetDrawable;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.PopupWindow;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.ScreenCaptureProtection;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;

/**
 * 设置项和分组标题旁的「?」：解释性说明不常驻页面，点一下在气泡里显示，点别处或按返回收起。手机没有悬停，所以只认点按；这与共享设置界面（macOS、Linux、HarmonyOS）和 iOS 的「?」是同一套设计语言。
 *
 * <p>外观是 16dp 的描边圆圈，里面一个 11sp 的问号，颜色取次要文字色；可点区域是 32dp 见方，圆圈居中画在里面，上下用负边距抵掉多出的高度，不会把标题行撑高。
 *
 * <p>读屏不必打开气泡：「?」的内容描述就是「标题 + 说明 + 正文」，TalkBack 移到它上面时直接读出全文。
 */
public final class InfoTipView extends TextView {
    private static final int GLYPH = 16;
    private static final int TARGET = 32;
    private static final int BUBBLE_MAX_WIDTH = 300;

    private CharSequence subject = "";
    private CharSequence help = "";
    @Nullable private PopupWindow bubble;

    public InfoTipView(Context context) {
        super(context);
        int sub = ThemeColorPolicy.subText(context);
        ViewPolicy.style(this, 11, 600, sub);
        setText("?");
        setIncludeFontPadding(false);
        setGravity(Gravity.CENTER);
        int inset = DimensionPolicy.pixels(context, (TARGET - GLYPH) / 2f);
        Drawable ring = DrawablePolicy.circleOutlined(android.graphics.Color.TRANSPARENT,
            DimensionPolicy.atLeastOnePixel(context, 1.2f), sub);
        setBackground(new InsetDrawable(ring, inset));
        ViewPolicy.setInteractive(this, true);
        ViewPolicy.bindClick(this, this::toggle);
    }

    /** 「?」旁边那个设置项或分组的名字，用在读屏的描述里。 */
    public void setSubject(@Nullable CharSequence subject) {
        this.subject = subject == null ? "" : subject;
        describe();
    }

    /** 气泡里的说明；为空时整个「?」不显示。 */
    public void setHelp(@Nullable CharSequence help) {
        this.help = help == null ? "" : help;
        ViewPolicy.setVisibilityForText(this, this.help);
        describe();
        if (bubble != null && bubble.isShowing()) bubble.dismiss();
    }

    public CharSequence help() { return help; }

    /** 放在标题后面时用的布局参数：32dp 见方，左边贴近标题，上下负边距抵掉超出文字行高的部分。 */
    public static LinearLayout.LayoutParams params(Context context) {
        int size = DimensionPolicy.pixels(context, TARGET);
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(size, size);
        int vertical = -DimensionPolicy.pixels(context, (TARGET - GLYPH) / 2f - 2);
        params.setMargins(-DimensionPolicy.pixels(context, 4), vertical, -DimensionPolicy.pixels(context, 8), vertical);
        params.gravity = Gravity.CENTER_VERTICAL;
        return params;
    }

    private void describe() {
        String prefix = subject.length() == 0 ? "说明：" : subject + "的说明：";
        setContentDescription(prefix + help);
    }

    private void toggle() {
        if (bubble != null && bubble.isShowing()) {
            bubble.dismiss();
            return;
        }
        if (help.length() == 0) return;
        Context context = getContext();
        TextView body = ViewPolicy.styledLabel(context, help, Ui.TEXT_ROW_SUBTITLE, 400, ThemeColorPolicy.text(context));
        ViewPolicy.setSymmetricPaddingDp(context, body, 14, 10);
        int margin = DimensionPolicy.pixels(context, 16);
        int screen = DimensionPolicy.screenWidthPixels(context);
        int maxWidth = Math.min(DimensionPolicy.pixels(context, BUBBLE_MAX_WIDTH), screen - 2 * margin);
        body.setMaxWidth(maxWidth);
        body.measure(View.MeasureSpec.makeMeasureSpec(maxWidth, View.MeasureSpec.AT_MOST),
            View.MeasureSpec.makeMeasureSpec(0, View.MeasureSpec.UNSPECIFIED));
        PopupWindow popup = new PopupWindow(body, ViewGroup.LayoutParams.WRAP_CONTENT,
            ViewGroup.LayoutParams.WRAP_CONTENT, true);
        popup.setBackgroundDrawable(DrawablePolicy.outlined(ThemeColorPolicy.card(context),
            DimensionPolicy.pixels(context, 12), DimensionPolicy.atLeastOnePixel(context, 0.5f),
            ThemeColorPolicy.hairline(context)));
        popup.setOutsideTouchable(true);
        ViewPolicy.setElevation(popup, DimensionPolicy.pixels(context, 8));
        popup.setOnDismissListener(() -> bubble = null);
        bubble = popup;
        // 气泡从「?」下面展开；右边放不下时整体左移，离屏幕右缘至少留 16dp。下方放不下时系统会翻到上方。
        int[] anchor = new int[2];
        getLocationOnScreen(anchor);
        int left = Math.max(margin, Math.min(anchor[0] + DimensionPolicy.pixels(context, 2),
            screen - margin - body.getMeasuredWidth()));
        popup.showAsDropDown(this, left - anchor[0], -DimensionPolicy.pixels(context, 4));
        // 气泡是独立的窗口，「防止截屏」要单独给它加 FLAG_SECURE。
        ScreenCaptureProtection.apply(popup);
    }

    @Override protected void onDetachedFromWindow() {
        if (bubble != null) bubble.dismiss();
        super.onDetachedFromWindow();
    }
}

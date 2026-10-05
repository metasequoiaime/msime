package app.msime.android.home;

import android.animation.ValueAnimator;
import android.graphics.drawable.Drawable;
import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowInsetsCompat;
import androidx.core.widget.NestedScrollView;
import app.msime.android.HostDeepLink;
import app.msime.android.R;

/**
 * 详情页基类：顶栏的返回按钮、28sp 的大标题（滚过 28dp 后收成顶栏里 22sp 的栏标题）、`NestedScrollView` 里一列内容，以及底部导航栏与键盘的避让。
 *
 * <p>详情页由 {@link SettingsNavigator} 压进 HomeActivity 的 `home_content`，底部导航一直可见，返回走 Activity 的 `OnBackPressedDispatcher`。子类只在 {@link #buildContent} 里往内容列加东西，通常是一组组 {@link GroupCard}。
 *
 * <p><b>子类的约束</b>（`recreate()`、换深浅模式和进程被杀后恢复都要靠它们重建页面）：
 * <ul>
 *   <li>必须是 `public` 类，并且有 `public` 的无参构造器——Fragment 由 `FragmentFactory.instantiate` 按类名反射创建；</li>
 *   <li>参数全部放在 {@link #getArguments()} 的 Bundle 里，不要用构造器参数或 setter 传，否则恢复后就没了；</li>
 *   <li>Bundle 里带 {@link HostDeepLink#ARG_EXTERNAL} 时，参数来自别的应用发来的深链，只能用于选择显示什么和初始滚动位置，不得据此联网、删除、注销、登录或上传；</li>
 *   <li>在 {@link PageId} 里登记类名，标题默认取那里的标题。</li>
 * </ul>
 *
 * <p>页面从别的页面返回、或整个应用回到前台时会调 {@link #onBecameVisible()}；需要刷新数据的页面覆盖它。
 */
public abstract class DetailPage extends HomeTabFragment {
    @Nullable private TextView largeTitle;
    @Nullable private TextView barTitle;
    @Nullable private View bar;
    @Nullable private ValueAnimator barFade;
    private boolean collapsed;

    /** 往内容列里加这一页的设置。视图每次重建都会再调一次，所以这里不要保留跨视图的状态。 */
    protected abstract void buildContent(LinearLayout column, Bundle args);

    /** 页面标题；默认取 {@link PageId} 里登记的标题，需要按参数决定标题的页面（例如词库详情）覆盖它。 */
    protected CharSequence title() {
        PageId page = PageId.forClassName(getClass().getName());
        return page == null ? "" : page.title();
    }

    /** 默认什么也不做；需要在回到眼前时刷新的页面覆盖它。 */
    @Override protected void onBecameVisible() {}

    @Override public final View onCreateView(@NonNull LayoutInflater inflater, @Nullable ViewGroup container,
            @Nullable Bundle state) {
        return inflater.inflate(R.layout.ms_w1_a2_detail_page, container, false);
    }

    @Override public final void onViewCreated(@NonNull View view, @Nullable Bundle state) {
        super.onViewCreated(view, state);
        largeTitle = view.findViewById(R.id.ms_detail_title);
        barTitle = view.findViewById(R.id.ms_detail_bar_title);
        bar = view.findViewById(R.id.ms_detail_bar);
        bar.setBackgroundColor(Ui.page(requireContext()));
        bar.getBackground().setAlpha(0);
        collapsed = false;
        setTitle(title());

        View back = view.findViewById(R.id.ms_detail_back);
        back.setContentDescription("返回");
        back.setOnClickListener(ignored -> requireActivity().getOnBackPressedDispatcher().onBackPressed());

        NestedScrollView scroll = view.findViewById(R.id.ms_detail_scroll);
        int threshold = Ui.dp(requireContext(), Ui.COLLAPSE_THRESHOLD);
        scroll.setOnScrollChangeListener((NestedScrollView.OnScrollChangeListener)
            (ignored, x, y, oldX, oldY) -> setCollapsed(y > threshold, true));

        // home_content 已经让开了状态栏和左右的刘海，这里只管底部：内容要能滚到底部导航栏（80dp 加手势区）之上，键盘弹出时再让到键盘之上。
        int base = Ui.dp(requireContext(), Ui.PAGE_PADDING_BOTTOM);
        int tabs = Ui.dp(requireContext(), Ui.TAB_BAR_HEIGHT);
        ViewCompat.setOnApplyWindowInsetsListener(scroll, (target, insets) -> {
            Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars());
            Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());
            int bottom = Math.max(bars.bottom + tabs, ime.bottom) + base;
            target.setPadding(target.getPaddingLeft(), target.getPaddingTop(), target.getPaddingRight(), bottom);
            return insets;
        });
        ViewCompat.requestApplyInsets(scroll);

        Bundle args = getArguments();
        buildContent(view.findViewById(R.id.ms_detail_column), args == null ? new Bundle() : args);
        // 重建后 NestedScrollView 自己恢复滚动位置，顶栏随之直接落在对应的状态，不再播放过渡。
        scroll.post(() -> setCollapsed(scroll.getScrollY() > threshold, false));
    }

    @Override public void onDestroyView() {
        if (barFade != null) barFade.cancel();
        barFade = null;
        largeTitle = null;
        barTitle = null;
        bar = null;
        super.onDestroyView();
    }

    /** 改标题（大标题和栏标题一起改），例如词库改名之后。 */
    protected final void setTitle(CharSequence text) {
        if (largeTitle != null) largeTitle.setText(text);
        if (barTitle != null) barTitle.setText(text);
    }

    /** 大标题行右侧放页面自己的操作（例如词库页的「刷新 / 导出 / 导入」）的容器。 */
    protected final LinearLayout headerActions() {
        return requireView().findViewById(R.id.ms_detail_actions);
    }

    /** 页面的滚动区，需要滚到某一组时用。 */
    protected final NestedScrollView scrollView() {
        return requireView().findViewById(R.id.ms_detail_scroll);
    }

    private void setCollapsed(boolean value, boolean animate) {
        if (bar == null || barTitle == null || largeTitle == null) return;
        if (value == collapsed && animate) return;
        collapsed = value;
        float target = value ? 1f : 0f;
        long duration = animate ? Ui.APP_BAR_FADE_MILLIS : 0;
        barTitle.animate().cancel();
        barTitle.animate().alpha(target).setDuration(duration).start();
        largeTitle.animate().cancel();
        largeTitle.animate().alpha(1f - target).setDuration(duration).start();
        if (barFade != null) barFade.cancel();
        Drawable fill = bar.getBackground();
        barFade = ValueAnimator.ofInt(fill.getAlpha(), value ? 255 : 0);
        barFade.setDuration(duration);
        barFade.addUpdateListener(animation -> fill.setAlpha((int) animation.getAnimatedValue()));
        barFade.start();
        // 收起后读屏要能从顶栏读到标题，展开时标题在大标题那里。
        barTitle.setImportantForAccessibility(value ? View.IMPORTANT_FOR_ACCESSIBILITY_YES : View.IMPORTANT_FOR_ACCESSIBILITY_NO);
    }
}

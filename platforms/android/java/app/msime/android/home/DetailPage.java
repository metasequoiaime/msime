package app.msime.android.home;

import app.msime.android.DimensionPolicy;

import app.msime.android.ThemeColorPolicy;
import android.animation.ValueAnimator;
import android.graphics.drawable.Drawable;
import android.os.Bundle;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.FrameLayout;
import android.widget.TextView;
import java.util.LinkedHashMap;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.core.widget.NestedScrollView;
import app.msime.android.HostDeepLink;
import app.msime.android.R;
import app.msime.android.ViewPolicy;

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
 *
 * <p>详情页也可以经 {@link #asTabRoot} 直接作为底部某个 tab 的根页（社区 tab 的 {@link CommunityTabPage}）：根页没有可返回的地方，不显示返回按钮，标题对齐设置 tab 的顶栏。
 */
public abstract class DetailPage extends HomeTabFragment {
    /** tab 根页在 arguments 里的标记。键名带连字符，外部深链的参数键（`HostDeepLink.isAllowedArgKey`）伪造不了它。 */
    private static final String ARG_TAB_ROOT = "detail-tab-root";
    /** 根页的标题起始边距，与设置 tab 收起后的顶栏（`page_keyboard.xml` 的 `keyboard_bar`）一致。 */
    private static final int TAB_ROOT_TITLE_START = 20;

    /** 把详情页标成 tab 根页，HomeActivity 建 tab 页时用；标记放在 arguments 里，进程恢复后仍在。 */
    static <T extends DetailPage> T asTabRoot(T page) {
        Bundle args = page.getArguments() == null ? new Bundle() : new Bundle(page.getArguments());
        args.putBoolean(ARG_TAB_ROOT, true);
        page.setArguments(args);
        return page;
    }

    /** 是否作为 tab 根页承载。 */
    protected final boolean isTabRoot() {
        Bundle args = getArguments();
        return args != null && args.getBoolean(ARG_TAB_ROOT, false);
    }

    @Nullable private TextView largeTitle;
    @Nullable private TextView barTitle;
    @Nullable private View bar;
    @Nullable private ValueAnimator barFade;
    private boolean collapsed;
    private boolean fixedNavigation;
    private String section = "";
    private final Map<String, LinearLayout> sections = new LinkedHashMap<>();
    @Nullable private SegmentedControl sectionTabs;
    private List<String> visibleSections = List.of();
    private String shownSection = "";

    /** 页面显式声明分类顺序；空列表表示没有分类导航。 */
    protected List<String> sectionTitles() { return List.of(); }

    /** 专用入口首次打开时显示的分类；旋转和进程恢复仍使用用户已选的分类。 */
    protected String initialSection() { return ""; }

    /** 分类内容列独立控制可见性，组内原有的禁用、隐藏与异步状态不受切换影响。 */
    protected final LinearLayout sectionColumn(LinearLayout parent, String title) {
        if (!sectionTitles().contains(title)) throw new IllegalArgumentException("Unknown detail section: " + title);
        LinearLayout column = sections.get(title);
        if (column == null || column.getParent() != parent) {
            column = app.msime.android.LayoutPolicy.column(parent.getContext());
            sections.put(title, column);
            ViewPolicy.setVisible(column, title.equals(section));
            parent.addView(column, app.msime.android.LayoutPolicy.matchWidthWrapParams());
        }
        return column;
    }

    @Override public void onCreate(@Nullable Bundle state) {
        super.onCreate(state);
        Bundle args = getArguments();
        // 深链带来的分类（HostDeepLink.ARG_SECTION）优先于 initialSection。
        String requested = args == null ? null : args.getString(HostDeepLink.ARG_SECTION);
        String first = requested != null && sectionTitles().contains(requested) ? requested : initialSection();
        section = state == null ? first : state.getString("detail-section", first);
        if (!sectionTitles().contains(section) && !sectionTitles().isEmpty()) section = sectionTitles().get(0);
    }

    @Override public void onSaveInstanceState(@NonNull Bundle state) {
        super.onSaveInstanceState(state);
        state.putString("detail-section", section);
    }

    /** 重绘后只列实际存在的分类，版本裁剪、登录状态和服务选择不会留下空 Tab。 */
    private void updateSections(LinearLayout parent) {
        if (sectionTabs == null) return;
        List<String> labels = new ArrayList<>();
        for (String title : sectionTitles()) {
            LinearLayout column = sections.get(title);
            if (column != null && column.getParent() == parent && column.getChildCount() > 0) labels.add(title);
        }
        if (!labels.isEmpty() && !labels.contains(section)) section = labels.get(0);
        // 只有一类内容时没有可切换的目标，隐藏整条导航并收回占位。
        View navigation = (View) sectionTabs.getParent();
        ViewPolicy.setVisible(navigation, labels.size() > 1);
        updateNavigationInset(navigation);
        if (labels.equals(visibleSections) && section.equals(shownSection)) {
            for (Map.Entry<String, LinearLayout> entry : sections.entrySet())
                ViewPolicy.setVisible(entry.getValue(), entry.getKey().equals(section));
            return;
        }
        shownSection = section;
        if (!labels.equals(visibleSections)) {
            visibleSections = List.copyOf(labels);
            sectionTabs.setFillWidth(true);
            sectionTabs.setOptions(labels, labels.indexOf(section));
        } else sectionTabs.setSelection(labels.indexOf(section));
        for (Map.Entry<String, LinearLayout> entry : sections.entrySet()) {
            ViewPolicy.setVisible(entry.getValue(), entry.getKey().equals(section));
        }
    }

    private void updateNavigationInset(View navigation) {
        View root = getView();
        if (root == null) return;
        int margin = ((FrameLayout.LayoutParams) navigation.getLayoutParams()).topMargin
            + (navigation.getVisibility() == View.VISIBLE ? navigation.getHeight() : 0);
        NestedScrollView scroll = root.findViewById(R.id.ms_detail_scroll);
        FrameLayout.LayoutParams params = (FrameLayout.LayoutParams) scroll.getLayoutParams();
        if (params.topMargin == margin) return;
        params.topMargin = margin;
        scroll.setLayoutParams(params);
    }

    /** 往内容列里加这一页的设置。视图每次重建都会再调一次，所以这里不要保留跨视图的状态。 */
    protected abstract void buildContent(LinearLayout column, Bundle args);

    /** 需要常驻分类导航的详情页在标题下提供一个控件，其余页面沿用滚动标题。 */
    @Nullable protected View fixedNavigation() {
        if (sectionTitles().isEmpty()) return null;
        FrameLayout navigation = new FrameLayout(requireContext());
        navigation.setMinimumHeight(DimensionPolicy.pixels(requireContext(), 48));
        ViewPolicy.setSymmetricPaddingDp(requireContext(), navigation, Ui.PAGE_PADDING, 4);
        ViewPolicy.setBackgroundColor(navigation, ThemeColorPolicy.page(requireContext()));
        sectionTabs = new SegmentedControl(requireContext());
        sectionTabs.setMinimumHeight(DimensionPolicy.pixels(requireContext(), 40));
        ViewPolicy.setBackground(sectionTabs, app.msime.android.DrawablePolicy.pill(ThemeColorPolicy.card(requireContext())));
        sectionTabs.setOptions(sectionTitles(), sectionTitles().indexOf(section));
        sectionTabs.setOnSelect(index -> {
            if (index >= visibleSections.size()) return;
            section = visibleSections.get(index);
            View focus = requireView().findFocus();
            if (focus != null) {
                focus.clearFocus();
                androidx.core.view.WindowInsetsControllerCompat controller =
                    androidx.core.view.WindowCompat.getInsetsController(requireActivity().getWindow(), focus);
                if (controller != null) controller.hide(androidx.core.view.WindowInsetsCompat.Type.ime());
            }
            updateSections(requireView().findViewById(R.id.ms_detail_column));
            scrollView().scrollTo(0, 0);
        });
        navigation.addView(sectionTabs, new FrameLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, DimensionPolicy.pixels(requireContext(), 40)));
        return navigation;
    }

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
        ViewPolicy.setBackgroundColor(bar, ThemeColorPolicy.page(requireContext()));
        bar.getBackground().setAlpha(0);
        collapsed = false;
        setTitle(title());

        View back = view.findViewById(R.id.ms_detail_back);
        if (isTabRoot()) {
            // tab 根页没有上一页；系统返回键仍由 HomeActivity 处理（回到第一个 tab）。
            ViewPolicy.hide(back);
            FrameLayout.LayoutParams titleParams = (FrameLayout.LayoutParams) barTitle.getLayoutParams();
            titleParams.setMarginStart(DimensionPolicy.pixels(requireContext(), TAB_ROOT_TITLE_START));
            barTitle.setLayoutParams(titleParams);
            ViewPolicy.setPaddingDp(requireContext(), view.findViewById(R.id.ms_detail_header),
                TAB_ROOT_TITLE_START, 0, Ui.PAGE_PADDING, 0);
        } else {
            back.setContentDescription("返回");
            ViewPolicy.bindClick(back,
                () -> requireActivity().getOnBackPressedDispatcher().onBackPressed());
        }

        NestedScrollView scroll = view.findViewById(R.id.ms_detail_scroll);
        View navigation = fixedNavigation();
        fixedNavigation = navigation != null;
        if (navigation != null) {
            ViewPolicy.hide(view.findViewById(R.id.ms_detail_header));
            FrameLayout.LayoutParams navParams = new FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            navParams.topMargin = DimensionPolicy.pixels(requireContext(), Ui.TOP_BAR_HEIGHT);
            ((FrameLayout) view).addView(navigation, navParams);
            navigation.addOnLayoutChangeListener((target, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom)
                -> updateNavigationInset(target));
        }
        int threshold = DimensionPolicy.pixels(requireContext(), Ui.COLLAPSE_THRESHOLD);
        scroll.setOnScrollChangeListener((NestedScrollView.OnScrollChangeListener)
            (ignored, x, y, oldX, oldY) -> setCollapsed(y > threshold, true));

        // home_content 已经让开了状态栏和左右的刘海，这里只管底部：内容要能滚到底部导航栏（80dp 加手势区）之上，键盘弹出时再让到键盘之上。
        PageInsetsPolicy.bind(scroll);

        Bundle args = getArguments();
        LinearLayout column = view.findViewById(R.id.ms_detail_column);
        buildContent(column, args == null ? new Bundle() : args);
        updateSections(column);
        if (sectionTabs != null) column.getViewTreeObserver().addOnGlobalLayoutListener(() -> updateSections(column));
        // 重建后 NestedScrollView 自己恢复滚动位置，顶栏随之直接落在对应的状态，不再播放过渡。
        scroll.post(() -> setCollapsed(scroll.getScrollY() > threshold, false));
    }

    @Override public void onDestroyView() {
        sections.clear();
        sectionTabs = null;
        visibleSections = List.of();
        shownSection = "";
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

    /** 大标题行右侧放页面自己的操作的容器。窄屏上这里的按钮会挤占大标题的宽度，按钮多于一个时放进内容里的卡片（词库页就是这样，#5682）。 */
    protected final LinearLayout headerActions() {
        return requireView().findViewById(R.id.ms_detail_actions);
    }

    /** 页面的滚动区，需要滚到某一组时用。 */
    protected final NestedScrollView scrollView() {
        return requireView().findViewById(R.id.ms_detail_scroll);
    }

    private void setCollapsed(boolean value, boolean animate) {
        if (bar == null || barTitle == null || largeTitle == null) return;
        if (fixedNavigation) { value = true; animate = false; }
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
        ViewPolicy.setImportantForAccessibility(barTitle,
            value ? View.IMPORTANT_FOR_ACCESSIBILITY_YES : View.IMPORTANT_FOR_ACCESSIBILITY_NO);
    }
}

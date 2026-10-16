package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.view.View;
import android.widget.FrameLayout;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.fragment.app.FragmentManager;
import app.msime.android.CommunityRequest;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.R;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import java.util.List;

/**
 * 底部「社区」tab 的根页：标题下固定的「皮肤 / 词库」两个分类，每个分类直接承载那一类作品的完整列表（嵌入模式的 {@link CommunityFragment}，带搜索、皮肤分类筛选、详情、举报和获取/使用/添加）。
 *
 * <p>由 HomeActivity 经 {@link DetailPage#asTabRoot} 承载，没有返回按钮；它只做 tab 根页、不经 {@link SettingsNavigator} 压栈，所以不在 {@link PageId} 里登记，标题由 {@link #title()} 给出。各处打开皮肤或词库完整列表的入口（`CommunityFragment.browse`、指向 `COMMUNITY` 的深链）都切到这个 tab 并选中对应分类，见 {@link SettingsNavigator}；短语和 AI 回复模板仍然压在各自的功能页上面。
 *
 * <p>与皮肤页的社区分类同一种做法：子列表放在固定 id 的容器里，旋转和恢复视图后重新关联；切换分类只藏起另一个容器、收起它的搜索键盘，不重建子列表，切回来时搜索词、筛选和已载入的列表都还在。没打开过的分类不建子列表，也就不联网。所选分类随 Fragment 状态保存。
 */
public final class CommunityTabPage extends ColumnDetailPage {
    /** 分类的顺序，与顶部选项一一对应。 */
    private static final List<CommunityRequest.Kind> KINDS =
        List.of(CommunityRequest.Kind.SKIN, CommunityRequest.Kind.DICTIONARY);
    private static final List<String> LABELS = List.of("皮肤", "词库");
    /** 子列表的容器 id，按 {@link #KINDS} 的顺序；固定 id 让 FragmentManager 在视图重建后把子列表放回原处。 */
    private static final int[] CONTAINERS = {R.id.community_tab_skins, R.id.community_tab_dictionaries};
    private static final String STATE_SECTION = "community-tab-section";

    private int section;
    @Nullable private FrameLayout navigation;
    @Nullable private SegmentedControl tabs;
    @Nullable private FrameLayout[] containers;

    /** 这一类作品的完整列表是不是这个 tab 的一个分类。 */
    static boolean hosts(CommunityRequest.Kind kind) {
        return KINDS.contains(kind);
    }

    @Override public void onCreate(@Nullable Bundle state) {
        super.onCreate(state);
        if (state != null) section = Math.max(0, Math.min(KINDS.size() - 1, state.getInt(STATE_SECTION, 0)));
    }

    @Override public void onSaveInstanceState(@NonNull Bundle state) {
        super.onSaveInstanceState(state);
        state.putInt(STATE_SECTION, section);
    }

    @Override protected CharSequence title() { return "社区"; }

    @Override protected View fixedNavigation() {
        Context context = requireContext();
        navigation = new FrameLayout(context);
        ViewPolicy.setSymmetricPaddingDp(context, navigation, Ui.PAGE_PADDING, 4);
        ViewPolicy.setBackgroundColor(navigation, ThemeColorPolicy.page(context));
        SegmentedControl control = new SegmentedControl(context);
        control.setFillWidth(true);
        control.setMinimumHeight(DimensionPolicy.pixels(context, 40));
        ViewPolicy.setBackground(control, DrawablePolicy.pill(ThemeColorPolicy.card(context)));
        control.setOptions(LABELS, section);
        control.setOnSelect(index -> {
            section = index;
            showSection();
        });
        tabs = control;
        navigation.addView(control, LayoutPolicy.frameMatchWidthWrapParams());
        navigation.addOnLayoutChangeListener((view, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom) ->
            positionContainers());
        return navigation;
    }

    @Override protected void onColumnReady(Bundle args) {
        // 列表各自滚动，详情页自带的滚动区在这一页没有内容。
        ViewPolicy.hide(scrollView());
        FrameLayout root = (FrameLayout) requireView();
        containers = new FrameLayout[CONTAINERS.length];
        for (int index = 0; index < CONTAINERS.length; index++) {
            FrameLayout container = new FrameLayout(requireContext());
            container.setId(CONTAINERS[index]);
            root.addView(container, LayoutPolicy.frameMatchParentParams());
            containers[index] = container;
        }
        positionContainers();
        showSection();
    }

    @Override protected void onContentDestroyed() {
        navigation = null;
        tabs = null;
        containers = null;
    }

    /** 回到这个 tab 时更新卡片上的「使用」「已添加」标记：别的 tab 里可能刚换过皮肤库或词库。 */
    @Override protected void onBecameVisible() {
        CommunityFragment shown = list(section);
        if (shown != null) shown.refreshTakenMarks();
    }

    /** 选中某一类作品的分类；HomeActivity 切到这个 tab 之后调用。视图还没建好时只记下来，建好时按它显示。 */
    void select(CommunityRequest.Kind kind) {
        int index = KINDS.indexOf(kind);
        if (index < 0) return;
        section = index;
        if (tabs != null) tabs.setSelection(index);
        if (containers != null) showSection();
    }

    private void positionContainers() {
        if (containers == null || navigation == null) return;
        int top = DimensionPolicy.pixels(requireContext(), Ui.TOP_BAR_HEIGHT)
            + (navigation.getHeight() > 0 ? navigation.getHeight() : DimensionPolicy.pixels(requireContext(), 48));
        for (FrameLayout container : containers) {
            FrameLayout.LayoutParams params = (FrameLayout.LayoutParams) container.getLayoutParams();
            if (params.topMargin == top) continue;
            params.topMargin = top;
            container.setLayoutParams(params);
        }
    }

    /** 只显示所选分类的容器；第一次显示时才建子列表，离开的那一类收起搜索键盘。 */
    private void showSection() {
        if (containers == null) return;
        FragmentManager manager = getChildFragmentManager();
        for (int index = 0; index < containers.length; index++) {
            boolean visible = index == section;
            ViewPolicy.setVisible(containers[index], visible);
            CommunityFragment list = list(index);
            if (!visible) {
                if (list != null) list.closeSearchKeyboard();
            } else if (list == null) {
                if (!manager.isStateSaved()) {
                    manager.beginTransaction()
                        .add(CONTAINERS[index], CommunityFragment.embedded(KINDS.get(index)), tag(index))
                        .commitNow();
                }
            } else {
                list.refreshTakenMarks();
            }
        }
    }

    @Nullable private CommunityFragment list(int index) {
        if (!isAdded()) return null;
        return (CommunityFragment) getChildFragmentManager().findFragmentByTag(tag(index));
    }

    private static String tag(int index) {
        return "community-tab-" + KINDS.get(index).id();
    }
}

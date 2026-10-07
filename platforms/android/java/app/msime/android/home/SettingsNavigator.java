package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.util.Log;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import androidx.fragment.app.FragmentManager;
import app.msime.android.HostDeepLink;
import app.msime.android.R;
import java.util.List;

/**
 * 打开详情页的唯一入口：先切到页面所属的 tab，再把页面压进 HomeActivity 的 `home_content`。
 *
 * <p>导航用 FragmentManager 的回退栈，整个宿主只有这一个栈：返回一层弹出一页，切 tab 先清空栈（HomeActivity 负责），所以「返回」永远回到眼前这个 tab 里的上一页，不会跳回别的 tab。页面之间是 M3 fade-through（新页面 0.28 秒从 94 % 缩放淡入，曲线 `cubic-bezier(.2, 0, 0, 1)`，旧页面快速淡出），与 tab 切换是同一种动效。
 *
 * <p>页面按 {@link PageId} 里登记的类名经 `FragmentFactory.instantiate` 创建：类还不存在（后续切片才建）时只记一条日志并提示「即将推出」，不切 tab，也不崩。不在 HomeActivity 里调用时（例如旧的二级 Activity），改为发一个指向 HomeActivity 的深链。
 */
public final class SettingsNavigator {
    private static final String TAG = "MSIMENavigator";
    /** 压进栈的页面 Fragment 的 tag 前缀，后面是 PageId 名。 */
    static final String PAGE_TAG_PREFIX = "page:";

    private SettingsNavigator() {}

    /**
     * 打开一页。
     *
     * @param args 页面参数，可空；会原样成为页面的 arguments，所以只放 Bundle 能保存的值
     */
    public static void open(Context context, PageId page, @Nullable Bundle args) {
        if (!(Ui.activityOf(context) instanceof HomeActivity home)) {
            context.startActivity(HostDeepLink.page(context, page.name(), args));
            return;
        }
        push(home, page, args);
    }

    /** HomeActivity 处理深链时也走这里；已经切好 tab 的情况下同样适用。 */
    static void push(HomeActivity home, PageId page, @Nullable Bundle args) {
        FragmentManager manager = home.getSupportFragmentManager();
        // 保存状态之后不能再提交导航；这时 Activity 已不在前台，用户也点不到任何入口，丢掉这次请求比崩溃好。
        if (manager.isStateSaved() || home.isFinishing()) return;

        Fragment fragment;
        try {
            fragment = manager.getFragmentFactory().instantiate(home.getClassLoader(), page.className());
        } catch (Fragment.InstantiationException missing) {
            Log.w(TAG, "Page " + page.name() + " is not available in this build", missing);
            MsToast.show(home, "即将推出");
            return;
        }
        fragment.setArguments(args == null ? new Bundle() : new Bundle(args));

        if (home.selectedTabIndex() != page.tab()) home.selectTabIndex(page.tab());

        // 连点两下只压一页：栈顶已经是这一页时不再压。
        int depth = manager.getBackStackEntryCount();
        if (depth > 0 && page.name().equals(manager.getBackStackEntryAt(depth - 1).getName())) return;

        Fragment below = visibleIn(manager);
        androidx.fragment.app.FragmentTransaction transaction = manager.beginTransaction()
            .setReorderingAllowed(true)
            .setCustomAnimations(R.anim.fade_through_in, R.anim.fade_through_out,
                R.anim.fade_through_in, R.anim.fade_through_out);
        // 下面那一页藏起来而不是移走：它的滚动位置和正在进行的加载都留着，返回时原样出现；藏起来也让读屏只读到最上面这一页。
        if (below != null) transaction.hide(below);
        transaction.add(R.id.home_content, fragment, PAGE_TAG_PREFIX + page.name())
            .addToBackStack(page.name())
            .commit();
        // 立即执行，让紧接着的第二次点击能从栈顶看到这一页。
        manager.executePendingTransactions();
    }

    /** `home_content` 里当前看得见的那一个 Fragment（最后加入且未隐藏的）。 */
    @Nullable static Fragment visibleIn(FragmentManager manager) {
        List<Fragment> fragments = manager.getFragments();
        for (int i = fragments.size() - 1; i >= 0; i--) {
            Fragment fragment = fragments.get(i);
            if (fragment.getId() == R.id.home_content && fragment.isAdded() && !fragment.isHidden()) return fragment;
        }
        return null;
    }
}

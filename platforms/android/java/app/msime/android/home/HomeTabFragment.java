package app.msime.android.home;

import android.view.animation.AlphaAnimation;
import android.view.animation.Animation;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;

/**
 * 一个 tab 页；它需要知道自己什么时候重新回到眼前。
 *
 * <p>The tabs are kept alive and hidden rather than torn down, which is what makes switching keep
 * scroll position and in-flight state. The cost is that `onResume` no longer marks the moment a
 * page comes back: a hidden fragment stays resumed, so switching away and back never calls it, and
 * returning to the app calls it on all four including the three nobody is looking at.
 *
 * <p>So the moment is stated explicitly here. Both callers matter: the keyboard is a separate
 * process that can change a setting while this screen is in the background, and the keyboard's own
 * pickers write the same preferences these pages read.
 */
public abstract class HomeTabFragment extends Fragment {
    /** Called when this page becomes the one on screen, and not while it is hidden. */
    protected abstract void onBecameVisible();

    @Override public void onResume() {
        super.onResume();
        if (!isHidden()) onBecameVisible();
    }

    @Override public void onHiddenChanged(boolean hidden) {
        super.onHiddenChanged(hidden);
        if (!hidden) onBecameVisible();
    }

    @Override @Nullable public Animation onCreateAnimation(int transit, boolean enter, int nextAnim) {
        Animation none = leavingWithoutAnimation(this);
        return none != null ? none : super.onCreateAnimation(transit, enter, nextAnim);
    }

    /**
     * 换 tab 前清栈时，替换回退栈记录里的回退动画；其余时候返回 null，照常用记录里的动画。
     *
     * <p>那一次弹栈里动到的页面都马上要离开屏幕：详情页被移走，弹回来的 tab 根页紧接着被切 tab 藏起来。动画一旦开始，被移走的视图要等动画结束才离开容器，被藏起来的视图虽然已是 GONE，父布局在动画结束前也照样画它，所以这里给一段零时长、全透明的动画：仍要画的那一帧是透明的，不和目标 tab 的淡入叠在一起。返回 null 不行：那样会退回去加载记录里的 fade_through 动画。
     *
     * <p>不继承这个类、却会被压进 `home_content` 回退栈的页面（{@link CommunityFragment}）也要在自己的 `onCreateAnimation` 里调它。
     */
    @Nullable static Animation leavingWithoutAnimation(Fragment page) {
        if (!(page.getActivity() instanceof HomeActivity home) || !home.clearingForTabSwitch()) return null;
        Animation none = new AlphaAnimation(0f, 0f);
        none.setDuration(0);
        return none;
    }
}

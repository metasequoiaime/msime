package app.msime.android.home;

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
}

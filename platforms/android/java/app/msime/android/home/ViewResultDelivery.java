package app.msime.android.home;

import android.view.View;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import androidx.lifecycle.Lifecycle;
import app.msime.android.MainThreadPolicy;

/** Delivers an asynchronous result only to the Fragment view that started the work. */
final class ViewResultDelivery {
    private final Fragment fragment;
    private final View view;
    private final Lifecycle lifecycle;

    private ViewResultDelivery(Fragment fragment, View view, Lifecycle lifecycle) {
        this.fragment = fragment;
        this.view = view;
        this.lifecycle = lifecycle;
    }

    @Nullable static ViewResultDelivery capture(Fragment fragment) {
        View view = fragment.getView();
        if (view == null) return null;
        return new ViewResultDelivery(fragment, view, fragment.getViewLifecycleOwner().getLifecycle());
    }

    void post(Runnable done) {
        MainThreadPolicy.mainHandler().post(() -> {
            if (!fragment.isAdded() || fragment.getView() != view) return;
            if (!lifecycle.getCurrentState().isAtLeast(Lifecycle.State.CREATED)) return;
            done.run();
        });
    }
}

package app.msime.android.home;

import app.msime.android.MainThreadPolicy;
import android.content.Context;
import android.os.Handler;
import android.view.View;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import androidx.lifecycle.Lifecycle;
import app.msime.android.ThreadPolicy;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.function.Consumer;
import java.util.function.Function;

/**
 * 把一次阻塞的宿主调用放到工作线程上，回到主线程交结果。
 *
 * <p>Every call into the shared store takes a file lock the input service also takes, so none of
 * them may run on the main thread. The result is delivered only while the fragment still has a view:
 * a settings screen the user has already left must not be writing into detached widgets.
 *
 * <p>The application context is captured on the calling thread. Reaching for the fragment's own
 * context from the worker is a race against the fragment being detached.
 */
public final class HostTask {
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-settings-host"));
    /** HTTP calls get their own threads: one can block for a full connect plus read timeout, and the shared store's reads and writes must not queue behind it. */
    private static final ExecutorService NETWORK = Executors.newCachedThreadPool(
        ThreadPolicy.namedDaemonFactory("msime-settings-network"));
    private static final Handler MAIN = MainThreadPolicy.mainHandler();

    private HostTask() {}

    /** Run `work` off the main thread and hand its result to `done` if the fragment is still up. */
    public static <T> void run(Fragment fragment, Function<Context, T> work,
            Consumer<T> done) {
        submit(WORKER, fragment, work, done);
    }

    /** Like {@link #run}, but for HTTP calls, which can block for a full connect plus read timeout; store calls stay on {@link #run} so they remain serialized on one thread. */
    public static <T> void runNetwork(Fragment fragment, Function<Context, T> work, Consumer<T> done) {
        submit(NETWORK, fragment, work, done);
    }

    private static <T> void submit(ExecutorService executor, Fragment fragment, Function<Context, T> work,
            Consumer<T> done) {
        View ownerView = fragment.getView();
        if (ownerView == null) return;
        Lifecycle ownerLifecycle = fragment.getViewLifecycleOwner().getLifecycle();
        Context context = fragment.getContext();
        if (context == null) return;
        Context application = context.getApplicationContext();
        executor.execute(() -> {
            final T result;
            try {
                result = work.apply(application);
            } catch (RuntimeException | LinkageError error) {
                // The host being unavailable is a state every caller renders; it is not a crash.
                MAIN.post(() -> deliver(fragment, ownerView, ownerLifecycle, done, null));
                return;
            }
            MAIN.post(() -> deliver(fragment, ownerView, ownerLifecycle, done, result));
        });
    }

    private static <T> void deliver(Fragment fragment, View ownerView, Lifecycle ownerLifecycle,
            Consumer<T> done, @Nullable T result) {
        // 视图销毁后可能很快重建；旧任务只能交给发起它的那棵视图。
        if (!fragment.isAdded() || fragment.getView() != ownerView) return;
        if (!ownerLifecycle.getCurrentState().isAtLeast(Lifecycle.State.CREATED)) return;
        done.accept(result);
    }
}

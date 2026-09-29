package app.msime.android.home;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import androidx.lifecycle.Lifecycle;
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
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(runnable -> {
        Thread thread = new Thread(runnable, "msime-settings-host");
        thread.setDaemon(true);
        return thread;
    });
    private static final Handler MAIN = new Handler(Looper.getMainLooper());

    private HostTask() {}

    /** Run `work` off the main thread and hand its result to `done` if the fragment is still up. */
    public static <T> void run(Fragment fragment, Function<Context, T> work,
            Consumer<T> done) {
        Context context = fragment.getContext();
        if (context == null) return;
        Context application = context.getApplicationContext();
        WORKER.execute(() -> {
            final T result;
            try {
                result = work.apply(application);
            } catch (RuntimeException | LinkageError error) {
                // The host being unavailable is a state every caller renders; it is not a crash.
                MAIN.post(() -> deliver(fragment, done, null));
                return;
            }
            MAIN.post(() -> deliver(fragment, done, result));
        });
    }

    private static <T> void deliver(Fragment fragment, Consumer<T> done, @Nullable T result) {
        if (!fragment.isAdded() || fragment.getView() == null) return;
        if (!fragment.getViewLifecycleOwner().getLifecycle().getCurrentState()
                .isAtLeast(Lifecycle.State.CREATED)) return;
        done.accept(result);
    }
}

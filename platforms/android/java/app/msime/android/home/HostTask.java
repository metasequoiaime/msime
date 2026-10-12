package app.msime.android.home;

import android.content.Context;
import androidx.fragment.app.Fragment;
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
        ViewResultDelivery owner = ViewResultDelivery.capture(fragment);
        if (owner == null) return;
        Context context = fragment.getContext();
        if (context == null) return;
        Context application = context.getApplicationContext();
        executor.execute(() -> {
            final T result;
            try {
                result = work.apply(application);
            } catch (RuntimeException | LinkageError error) {
                // The host being unavailable is a state every caller renders; it is not a crash.
                owner.post(() -> done.accept(null));
                return;
            }
            owner.post(() -> done.accept(result));
        });
    }
}

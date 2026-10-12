package app.msime.android.home;

import androidx.annotation.Nullable;
import androidx.fragment.app.Fragment;
import app.msime.android.ThreadPolicy;
import java.util.concurrent.Callable;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.function.Consumer;

/** Runs blocking network work without holding up the shared store's serialized worker. */
final class NetworkTask {
    /** Large downloads must not make short clipboard and feedback requests wait. */
    private static final ExecutorService WORKER = Executors.newCachedThreadPool(
        ThreadPolicy.namedDaemonFactory("msime-home-network"));

    private NetworkTask() {}

    record Outcome<T>(@Nullable T value, @Nullable Exception error) {}

    /** Returns a cancellable task, or null when the fragment has no view. */
    @Nullable static <T> Future<?> run(Fragment fragment, Callable<T> work, Consumer<Outcome<T>> done) {
        ViewResultDelivery owner = ViewResultDelivery.capture(fragment);
        if (owner == null) return null;
        return WORKER.submit(() -> {
            Outcome<T> outcome;
            try {
                outcome = new Outcome<>(work.call(), null);
            } catch (Exception error) {
                outcome = new Outcome<>(null, error);
            }
            Outcome<T> result = outcome;
            owner.post(() -> done.accept(result));
        });
    }
}

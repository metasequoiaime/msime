import app.msime.android.AccountTaskExecutor;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

public final class AccountTaskExecutorSmoke {
    static void check(boolean value) { if (!value) throw new AssertionError(); }

    public static void main(String[] arguments) throws Exception {
        CountDownLatch started = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        ThreadPoolExecutor executor = AccountTaskExecutor.create();
        try {
            executor.execute(() -> {
                started.countDown();
                try { release.await(2, TimeUnit.SECONDS); }
                catch (InterruptedException error) { Thread.currentThread().interrupt(); }
            });
            check(started.await(2, TimeUnit.SECONDS));
            executor.execute(() -> {});
            check(executor.getQueue().size() == 1);
            try {
                executor.execute(() -> {});
                throw new AssertionError("account task queue must be bounded");
            } catch (RejectedExecutionException expected) {
                // expected
            }
        } finally {
            release.countDown();
            executor.shutdownNow();
            check(executor.awaitTermination(2, TimeUnit.SECONDS));
        }
    }
}

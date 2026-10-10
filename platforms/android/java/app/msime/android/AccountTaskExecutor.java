package app.msime.android;

import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;

/** Bounded worker used by account plugin commands that may perform network or bootstrap work. */
public final class AccountTaskExecutor {
    private static final int QUEUE_CAPACITY = 1;

    private AccountTaskExecutor() {}

    public static ThreadPoolExecutor create() {
        return new ThreadPoolExecutor(
            1,
            1,
            0,
            TimeUnit.MILLISECONDS,
            new ArrayBlockingQueue<>(QUEUE_CAPACITY),
            ThreadPolicy.namedDaemonFactory("msime-account"),
            new ThreadPoolExecutor.AbortPolicy());
    }
}

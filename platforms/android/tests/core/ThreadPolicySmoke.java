import app.msime.android.ThreadPolicy;
import java.util.concurrent.ThreadFactory;
import java.util.concurrent.atomic.AtomicBoolean;

public final class ThreadPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws InterruptedException {
        AtomicBoolean namedRan = new AtomicBoolean(false);
        ThreadFactory namedFactory = ThreadPolicy.namedFactory("msime-smoke-named");
        Thread named = namedFactory.newThread(() -> namedRan.set(true));
        check("msime-smoke-named".equals(named.getName()), "named factory preserves the name");
        check(!named.isDaemon(), "named factory preserves the default non-daemon lifetime");
        check(named.getPriority() == Thread.NORM_PRIORITY, "named factory preserves the default priority");
        check(named.getState() == Thread.State.NEW, "named factory does not start the thread");
        named.start();
        named.join();
        check(namedRan.get(), "named factory task runs");

        AtomicBoolean ran = new AtomicBoolean(false);
        ThreadFactory factory = ThreadPolicy.namedDaemonFactory("msime-smoke-worker");
        Thread thread = factory.newThread(() -> ran.set(true));

        check("msime-smoke-worker".equals(thread.getName()), "thread name is preserved");
        check(thread.isDaemon(), "worker thread is a daemon");
        check(thread.getState() == Thread.State.NEW, "factory does not start the thread");

        thread.start();
        thread.join();
        check(ran.get(), "worker task runs");

        AtomicBoolean directRan = new AtomicBoolean(false);
        Thread direct = ThreadPolicy.namedDaemonThread("msime-smoke-direct", () -> directRan.set(true));
        check("msime-smoke-direct".equals(direct.getName()), "direct thread name is preserved");
        check(direct.isDaemon(), "direct worker thread is a daemon");
        check(direct.getState() == Thread.State.NEW, "direct thread is not started");
        direct.start();
        direct.join();
        check(directRan.get(), "direct worker task runs");

        AtomicBoolean startedRan = new AtomicBoolean(false);
        Thread started = ThreadPolicy.startNamedThread("msime-smoke-started", () -> startedRan.set(true));
        started.join();
        check("msime-smoke-started".equals(started.getName()), "started thread name is preserved");
        check(started.isDaemon() == Thread.currentThread().isDaemon(),
            "started thread preserves inherited daemon state");
        check(startedRan.get(), "started worker task runs");

        Thread unstarted = ThreadPolicy.namedThread("msime-smoke-unstarted", () -> {});
        check("msime-smoke-unstarted".equals(unstarted.getName()), "unstarted thread name is preserved");
        check(unstarted.getState() == Thread.State.NEW, "named thread is not started");
        check(unstarted.isDaemon() == Thread.currentThread().isDaemon(),
            "named thread preserves inherited daemon state");
        System.out.println("Android named thread policy passed");
    }
}

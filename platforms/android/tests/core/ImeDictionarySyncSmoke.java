package app.msime.android;

import app.msime.android.policy.DictionarySyncPolicy;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.AbstractExecutorService;
import java.util.concurrent.TimeUnit;

/** 工作线程和主线程都由队列驱动，验证真实键盘调度器，不依赖 Android 的时钟或任意等待。 */
public final class ImeDictionarySyncSmoke {
    private static final class Worker extends AbstractExecutorService {
        final ArrayDeque<Runnable> tasks = new ArrayDeque<>();
        boolean stopped;
        public void execute(Runnable task) { tasks.add(task); }
        public void shutdown() { stopped = true; }
        public List<Runnable> shutdownNow() { stopped = true; return List.of(); }
        public boolean isShutdown() { return stopped; }
        public boolean isTerminated() { return stopped && tasks.isEmpty(); }
        public boolean awaitTermination(long timeout, TimeUnit unit) { return isTerminated(); }
        void drain() { while (!tasks.isEmpty()) tasks.remove().run(); }
    }

    private static final class Main implements ImeDictionarySync.MainThread {
        long now;
        final ArrayDeque<Runnable> posts = new ArrayDeque<>();
        Runnable timer;
        long delay;
        public long uptimeMillis() { return now; }
        public void post(Runnable task) { posts.add(task); }
        public void postDelayed(Runnable task, long millis) { timer = task; delay = millis; }
        public void removeCallbacks(Runnable task) { if (timer == task) timer = null; }
        void drain() { while (!posts.isEmpty()) posts.remove().run(); }
        void fireTimer() { Runnable task = timer; timer = null; task.run(); }
    }

    private static final class Connection implements ImeDictionarySync.Connection {
        int pending;
        int statusReads;
        final List<String> runs = new ArrayList<>();
        public DictionaryWebDavSync.Status status() {
            statusReads++;
            return new DictionaryWebDavSync.Status("0123456789abcdef0123456789abcdef", null,
                true, true, 1, 0, pending, null, false, null, null, null);
        }
        public DictionaryWebDavSync.Run run(String trigger, boolean confirm, boolean restore) {
            runs.add(trigger);
            pending = 0;
            return new DictionaryWebDavSync.Run(true, 1_800_000L, status(), null, null);
        }
    }

    public static void main(String[] args) {
        Main main = new Main();
        Worker worker = new Worker();
        Connection connection = new Connection();
        ImeDictionarySync sync = new ImeDictionarySync(main, worker, () -> connection);
        sync.start();
        main.fireTimer();
        worker.drain();
        main.drain();
        check(connection.runs.size() == 1 && main.delay == 1_800_000L,
            "启动后沿用共享层的半小时定时");

        // 设置 App 在键盘占着词库时拉到了远端改动，IME 自己上次的 pending 仍为零。
        connection.pending = 2;
        int reads = connection.statusReads;
        sync.idle();
        check(connection.statusReads == reads, "收起键盘只排队，不在输入线程读共享状态");
        worker.drain();
        main.drain();
        check(connection.runs.size() == 2 && connection.pending == 0,
            "收起后读取另一进程留下的待应用改动，不能等旧的半小时定时");
        check(connection.runs.get(1).equals(DictionarySyncPolicy.TRIGGER_PERIODIC),
            "远端应用交回共享层的 periodic，不增设宿主联网规则");

        // 已经应用完时收起只检查状态，不额外同步，也不撤掉原定时器。
        main.now += DictionarySyncPolicy.APPLY_RETRY_MILLIS;
        sync.idle();
        worker.drain();
        main.drain();
        check(connection.runs.size() == 2 && main.timer != null && main.delay == 1_800_000L,
            "没有待应用改动时不调用 run，保留共享层原来的定时");
        reads = connection.statusReads;
        sync.idle();
        worker.drain();
        check(connection.statusReads == reads, "连续收起合并为有界状态检查");
        sync.stop();
        check(main.timer == null && worker.isShutdown(), "销毁撤掉定时并关闭工作线程");
        System.out.println("ImeDictionarySyncSmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

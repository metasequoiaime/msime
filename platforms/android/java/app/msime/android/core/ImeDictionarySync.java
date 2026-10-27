package app.msime.android;

import android.content.Context;
import android.os.Handler;
import android.os.SystemClock;
import app.msime.android.policy.DictionarySyncPolicy;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.atomic.AtomicReference;

/**
 * 键盘进程（`:ime`）里的「我的词条」网盘同步（`docs/dictionary-webdav-sync.md`）：用户在键盘里改了词（候选栏删除、设置页排进队列的改动被键盘写进词库）之后以 `local_change` 记一次，之后按 Rust 给的 `next_check_ms` 以 `periodic` 定时再来。要不要联网、额度、防抖和退避都由 Rust 决定，这里只管什么时候调、在哪条线程上调。
 *
 * <p>线程：每次调用都在自己的一条工作线程上，从不在主线程或输入线程上，也不排在键盘建会话用的 `preferencesWorker` 后面（一次同步可能阻塞几十秒，那样键盘会一直停在「准备中」）。Rust 写本机词库时每批只「试着」取一次独占维护权、写完就放，从不跨网络请求持有；输入会话开着时它拿不到，这一轮就不写（`deferred`），会话结束后重新读取共享的待应用状态并重试。键盘建会话时撞上这短短一批的维护权，走的是已有的「dictionary maintenance busy」重试。所以输入线程既不等网络，也不等这里的任何锁。
 *
 * <p>定时用主线程的 Handler，与键盘里其他延时任务相同，没有拿 WorkManager 或闹钟排任务：输入法服务被系统绑定时进程一直活着，被回收时定时跟着没了，下一次键盘起来（{@link #start}）或收起（{@link #idle}）时再接上。系统深度休眠时 Handler 的时间不走，那段时间本来就不该联网。
 *
 * <p>设备身份：本机还没有词库密钥串里的设备编号时一律不跑（{@link DictionarySyncPolicy#imeMayRun}），第一次登记只由设置页做，键盘进程从不生成身份；没配置网盘或还没登记时不起定时器，只在键盘收起时隔一分钟以上看一眼偏好。
 */
final class ImeDictionarySync {
    /** 只检查其它进程留下的待应用状态，不是交给 Rust 的同步触发名。 */
    private static final String CHECK_PENDING_APPLY = "check_pending_apply";
    interface MainThread {
        long uptimeMillis();
        void post(Runnable task);
        void postDelayed(Runnable task, long delay);
        void removeCallbacks(Runnable task);
    }

    interface Connection {
        DictionaryWebDavSync.Status status() throws DictionaryWebDavSync.Failure;
        DictionaryWebDavSync.Run run(String trigger, boolean confirm, boolean restore) throws DictionaryWebDavSync.Failure;
    }

    @FunctionalInterface interface Connections {
        Connection open() throws DictionaryWebDavSync.Failure;
    }

    private final MainThread main;
    private final ExecutorService worker;
    private final Connections connections;
    /** 还没开始跑的那一次的触发；为 null 时工作线程上没有排着的同步。 */
    private final AtomicReference<String> queued = new AtomicReference<>();
    private final Runnable periodic = () -> submit(DictionarySyncPolicy.TRIGGER_PERIODIC);
    /** 以下只在主线程上读写。 */
    private boolean stopped;
    private boolean scheduled;
    private long nextIdleCheck;
    private long nextApplyRetry;
    /** 最近一次读取共享状态时待应用的远端改动数；键盘收起时仍须重读，不能依赖进程缓存。 */
    private volatile int pendingApply;

    ImeDictionarySync(Context context, Handler main) {
        this(new MainThread() {
            public long uptimeMillis() { return SystemClock.uptimeMillis(); }
            public void post(Runnable task) { main.post(task); }
            public void postDelayed(Runnable task, long delay) { main.postDelayed(task, delay); }
            public void removeCallbacks(Runnable task) { main.removeCallbacks(task); }
        }, Executors.newSingleThreadExecutor(ThreadPolicy.namedFactory("msime-dictionary-sync")),
            () -> open(context.getApplicationContext()));
    }

    ImeDictionarySync(MainThread main, ExecutorService worker, Connections connections) {
        this.main = main;
        this.worker = worker;
        this.connections = connections;
    }

    private static Connection open(Context context) throws DictionaryWebDavSync.Failure {
        DictionaryWebDavSync source = DictionaryWebDavSync.open(context);
        return new Connection() {
            public DictionaryWebDavSync.Status status() throws DictionaryWebDavSync.Failure { return source.status(); }
            public DictionaryWebDavSync.Run run(String trigger, boolean confirm, boolean restore) throws DictionaryWebDavSync.Failure {
                return source.run(trigger, confirm, restore);
            }
        };
    }

    /** 服务创建后稍等一会儿（不和第一次建会话抢读词库）看一眼是否配置过、本机是否已登记，是的话跑一次定时同步（Rust 按上次的时间决定这次要不要联网）。只在主线程上调。 */
    void start() {
        nextIdleCheck = main.uptimeMillis() + DictionarySyncPolicy.IDLE_RECHECK_MILLIS;
        scheduled = true;
        main.postDelayed(periodic, DictionarySyncPolicy.START_DELAY_MILLIS);
    }

    /** 键盘里改了「我的词条」之后：只记录本机改动，不联网。任何线程上都可以调。 */
    void localChange() {
        submit(DictionarySyncPolicy.TRIGGER_LOCAL_CHANGE);
    }

    /** 输入会话刚结束（键盘收起）：在后台重读其它进程留下的待应用状态；没起定时器时，隔一分钟以上再看一眼配置。只在主线程上调。 */
    void idle() {
        if (stopped) return;
        long now = main.uptimeMillis();
        if (scheduled || pendingApply > 0) {
            if (now < nextApplyRetry) return;
            nextApplyRetry = now + DictionarySyncPolicy.APPLY_RETRY_MILLIS;
            submit(CHECK_PENDING_APPLY);
            return;
        }
        if (now < nextIdleCheck) return;
        nextIdleCheck = now + DictionarySyncPolicy.IDLE_RECHECK_MILLIS;
        submit(DictionarySyncPolicy.TRIGGER_PERIODIC);
    }

    /** 服务销毁：撤掉定时器，正在跑的那一次让它跑完（写盘和本机词库都是一步一步提交的，中断也不丢不重）。只在主线程上调。 */
    void stop() {
        stopped = true;
        main.removeCallbacks(periodic);
        scheduled = false;
        worker.shutdown();
    }

    private void submit(String trigger) {
        String previous = queued.getAndUpdate(current -> {
            if (current == null || CHECK_PENDING_APPLY.equals(current)) return trigger;
            if (CHECK_PENDING_APPLY.equals(trigger)) return current;
            return DictionarySyncPolicy.mergeImeTrigger(current, trigger);
        });
        if (previous != null) return;
        try {
            worker.execute(this::runQueued);
        } catch (RejectedExecutionException shutDown) {
            queued.set(null);
        }
    }

    private void runQueued() {
        String trigger = queued.getAndSet(null);
        if (trigger == null) return;
        boolean pendingOnly = CHECK_PENDING_APPLY.equals(trigger);
        long delay = -1;
        try {
            Connection sync = connections.open();
            DictionaryWebDavSync.Status status = sync.status();
            pendingApply = status.pendingApply();
            // 设置 App 也会持久化 deferred；只看 IME 上轮的缓存会漏掉它。没有待应用项时保留原定时，不增加网络请求。
            if (pendingOnly && pendingApply == 0) return;
            if (DictionarySyncPolicy.imeMayRun(status.deviceId())) {
                DictionaryWebDavSync.Run run = sync.run(
                    pendingOnly ? DictionarySyncPolicy.TRIGGER_PERIODIC : trigger, false, false);
                pendingApply = run.status().pendingApply();
                delay = DictionarySyncPolicy.nextDelayMillis(run.nextCheckMs());
            } else {
                pendingApply = 0;
            }
        } catch (DictionaryWebDavSync.Failure failure) {
            // 没配置网盘、偏好读不了、请求不合规：不起定时器，键盘收起时再看。原因不记日志。
            pendingApply = 0;
        } catch (RuntimeException | LinkageError unavailable) {
            pendingApply = 0;
        }
        // 一次只读检查失败不取消原来的周期；真正同步的失败仍按已有规则重排。
        if (pendingOnly && delay < 0) return;
        long next = delay;
        main.post(() -> reschedule(next));
    }

    private void reschedule(long delay) {
        if (stopped) return;
        main.removeCallbacks(periodic);
        scheduled = delay >= 0;
        if (scheduled) main.postDelayed(periodic, delay);
    }
}

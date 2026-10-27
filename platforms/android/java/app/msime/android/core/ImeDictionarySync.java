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
 * <p>线程：每次调用都在自己的一条工作线程上，从不在主线程或输入线程上，也不排在键盘建会话用的 `preferencesWorker` 后面（一次同步可能阻塞几十秒，那样键盘会一直停在「准备中」）。Rust 写本机词库时每批只「试着」取一次独占维护权、写完就放，从不跨网络请求持有；输入会话开着时它拿不到，这一轮就不写（`deferred`），等会话结束后的下一次定时。键盘建会话时撞上这短短一批的维护权，走的是已有的「dictionary maintenance busy」重试。所以输入线程既不等网络，也不等这里的任何锁。
 *
 * <p>定时用主线程的 Handler，与键盘里其他延时任务相同，没有拿 WorkManager 或闹钟排任务：输入法服务被系统绑定时进程一直活着，被回收时定时跟着没了，下一次键盘起来（{@link #start}）或收起（{@link #idle}）时再接上。系统深度休眠时 Handler 的时间不走，那段时间本来就不该联网。
 *
 * <p>设备身份：本机还没有词库密钥串里的设备编号时一律不跑（{@link DictionarySyncPolicy#imeMayRun}），第一次登记只由设置页做，键盘进程从不生成身份；没配置网盘或还没登记时不起定时器，只在键盘收起时隔一分钟以上看一眼偏好。
 */
final class ImeDictionarySync {
    private final Context context;
    private final Handler main;
    private final ExecutorService worker = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedFactory("msime-dictionary-sync"));
    /** 还没开始跑的那一次的触发；为 null 时工作线程上没有排着的同步。 */
    private final AtomicReference<String> queued = new AtomicReference<>();
    private final Runnable periodic = () -> submit(DictionarySyncPolicy.TRIGGER_PERIODIC);
    /** 以下只在主线程上读写。 */
    private boolean stopped;
    private boolean scheduled;
    private long nextIdleCheck;
    private long nextApplyRetry;
    /** 上一次同步后还有多少远端改动没写进本机词库；键盘收起时有就马上再来一次，趁没有输入会话把它们写进去。 */
    private volatile int pendingApply;

    ImeDictionarySync(Context context, Handler main) {
        this.context = context.getApplicationContext();
        this.main = main;
    }

    /** 服务创建后稍等一会儿（不和第一次建会话抢读词库）看一眼是否配置过、本机是否已登记，是的话跑一次定时同步（Rust 按上次的时间决定这次要不要联网）。只在主线程上调。 */
    void start() {
        nextIdleCheck = SystemClock.uptimeMillis() + DictionarySyncPolicy.IDLE_RECHECK_MILLIS;
        scheduled = true;
        main.postDelayed(periodic, DictionarySyncPolicy.START_DELAY_MILLIS);
    }

    /** 键盘里改了「我的词条」之后：只记录本机改动，不联网。任何线程上都可以调。 */
    void localChange() {
        submit(DictionarySyncPolicy.TRIGGER_LOCAL_CHANGE);
    }

    /** 输入会话刚结束（键盘收起）：有没写进本机的远端改动就马上再来一次；没起定时器时，隔一分钟以上再看一眼配置。只在主线程上调。 */
    void idle() {
        if (stopped) return;
        long now = SystemClock.uptimeMillis();
        if (pendingApply > 0) {
            if (now < nextApplyRetry) return;
            nextApplyRetry = now + DictionarySyncPolicy.APPLY_RETRY_MILLIS;
            submit(DictionarySyncPolicy.TRIGGER_PERIODIC);
            return;
        }
        if (scheduled || now < nextIdleCheck) return;
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
        String previous = queued.getAndUpdate(current ->
            current == null ? trigger : DictionarySyncPolicy.mergeImeTrigger(current, trigger));
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
        long delay = -1;
        try {
            DictionaryWebDavSync sync = DictionaryWebDavSync.open(context);
            DictionaryWebDavSync.Status status = sync.status();
            if (DictionarySyncPolicy.imeMayRun(status.deviceId())) {
                DictionaryWebDavSync.Run run = sync.run(trigger, false, false);
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

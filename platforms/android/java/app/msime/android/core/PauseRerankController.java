package app.msime.android;

import java.util.function.Consumer;
import java.util.function.LongSupplier;

/** 在会话线程上安排停顿评分；视图和模型状态仍由共享运行时拥有。 */
public final class PauseRerankController<V> {
    public interface Scheduler {
        void post(Runnable task, long delayMillis);
        void remove(Runnable task);
    }
    public interface Bridge<V> {
        Reply<V> start(long session) throws Exception;
        Reply<V> poll(long session, long token, boolean accept) throws Exception;
    }
    /** moved 仅表示文字或顺序变化，view 仍可能包含必须接收的新候选代次。 */
    public record Reply<V>(String status, long token, boolean moved, V view) {}
    private static final long QUIET_MILLIS = 300;
    private static final long POLL_MILLIS = 16;
    private static final class Job {
        final long session;
        final long token;
        boolean accepting = true;
        Runnable discardTimer;
        Job(long session, long token) { this.session = session; this.token = token; }
    }
    private final Scheduler scheduler;
    private final Bridge<V> bridge;
    private final LongSupplier currentSession;
    private final Consumer<V> publish;
    private final Runnable onError;
    private Runnable timer;
    private long epoch;
    private boolean rendering;
    private Job job;

    public PauseRerankController(Scheduler scheduler, Bridge<V> bridge, LongSupplier currentSession,
            Consumer<V> publish, Runnable onError) {
        this.scheduler = scheduler;
        this.bridge = bridge;
        this.currentSession = currentSession;
        this.publish = publish;
        this.onError = onError;
    }
    public void schedule(boolean eligible) {
        if (rendering) return;
        cancel();
        long handle = currentSession.getAsLong();
        if (handle != 0 && eligible) post(epoch, handle, QUIET_MILLIS);
    }
    public void cancel() {
        epoch++;
        if (timer != null) scheduler.remove(timer);
        timer = null;
        if (job != null) {
            job.accepting = false;
            if (job.discardTimer == null) discard(job);
        }
    }
    private boolean current(long generation, long handle) {
        return generation == epoch && handle != 0 && handle == currentSession.getAsLong();
    }
    private void post(long generation, long handle, long delay) {
        timer = () -> {
            // 已进入消息队列的旧回调也不能清掉新任务的定时器引用。
            if (!current(generation, handle)) return;
            timer = null;
            run(generation, handle);
        };
        scheduler.post(timer, delay);
    }
    private void run(long generation, long handle) {
        if (job != null && job.session != handle) forget(job);
        // 原生start会重复返回未收集的令牌；旧评分归还前，不能认领成新停顿。
        if (job != null && !job.accepting) {
            post(generation, handle, POLL_MILLIS);
            return;
        }
        Reply<V> result;
        try {
            result = job == null ? bridge.start(handle) : bridge.poll(handle, job.token, true);
            if ("pending".equals(result.status())) {
                if (result.token() <= 0 || (job != null && job.token != result.token()))
                    throw new IllegalStateException("Invalid pause rerank token");
                if (job == null) job = new Job(handle, result.token());
            } else if ("done".equals(result.status()) || "continue".equals(result.status())) {
                if (job != null) forget(job);
            } else {
                throw new IllegalStateException("Invalid pause rerank status");
            }
        } catch (Exception | LinkageError error) {
            cancel();
            onError.run();
            return;
        }
        if (result.view() != null) {
            rendering = true;
            try { publish.accept(result.view()); }
            finally { rendering = false; }
        }
        if (current(generation, handle)) {
            if ("continue".equals(result.status())) post(generation, handle, 0);
            else if ("pending".equals(result.status())) post(generation, handle, POLL_MILLIS);
        }
    }
    /** 取消不等于已归还：Running阶段accept=false仍会返回pending。 */
    private void discard(Job discarded) {
        if (job != discarded || discarded.accepting) return;
        if (currentSession.getAsLong() != discarded.session) {
            // 会话销毁由Rust abandon收尾；新句柄不得再碰旧任务。
            forget(discarded);
            return;
        }
        discarded.discardTimer = null;
        try {
            Reply<V> result = bridge.poll(discarded.session, discarded.token, false);
            if ("done".equals(result.status())) {
                forget(discarded);
                return;
            }
        } catch (Exception | LinkageError error) {
            // 读不懂回复不能证明已归还，继续丢弃到明确完成或会话销毁。
        }
        discarded.discardTimer = () -> discard(discarded);
        scheduler.post(discarded.discardTimer, POLL_MILLIS);
    }
    private void forget(Job completed) {
        if (completed.discardTimer != null) scheduler.remove(completed.discardTimer);
        if (job == completed) job = null;
    }
}

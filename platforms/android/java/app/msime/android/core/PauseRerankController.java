package app.msime.android;

import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import java.util.function.LongSupplier;

/**
 * 在会话线程上安排停顿评分；视图和模型状态仍由共享运行时拥有。
 *
 * <p>计时只从用户的输入算起（{@link #schedule}），别的重绘只能在不再适用时取消它（{@link #keep}），不能重新计时：释义、在线行这些异步刷新各有各的时机，挂在它们上面，换序什么时候发生就由不相干的事决定。手指按着候选时不起跑、不接续，松手后再接着做：每一片都推进候选代次，按下时画出来的那个候选到松手时就选不中了。
 *
 * <p>展开面板开着时同样不起跑、不接续，但不轮询：停在原处，面板收起时宿主调 {@link #resume()} 接着做。重排只换顺序、不添新词，用户要找的词还在面板里；面板里的格子绑着打开时的代次，这时换代，面板会换成新一代并回到顶部。
 */
public final class PauseRerankController<V> {
    public interface Scheduler {
        void post(Runnable task, long delayMillis);
        void remove(Runnable task);
    }
    public interface Bridge<V> {
        Reply<V> start(long session) throws Exception;
        Reply<V> poll(long session, long token, boolean accept) throws Exception;
    }
    /**
     * moved 仅表示文字或顺序变化，view 仍可能包含必须接收的新候选代次。deferred 是原生回复里的同名字段：会话挂着延后到停顿的逐键模型。
     */
    public record Reply<V>(String status, long token, boolean moved, V view, boolean deferred) {
        public Reply(String status, long token, boolean moved, V view) {
            this(status, token, moved, view, false);
        }
    }
    private static final long QUIET_MILLIS = 300;
    /**
     * 延后到停顿的模型在会话线程上分片跑，一片约 8 ms，打字的空隙里就该起跑：分数备忘在同一段组字里保留，每次停顿只算新的前缀，换序紧跟在按键之后，而不是在用户伸手去点候选的时候。后台的落定模型一次要一两百毫秒，仍等 {@link #QUIET_MILLIS}。
     */
    private static final long DEFERRED_QUIET_MILLIS = 40;
    private static final long POLL_MILLIS = 16;
    /** {@link #settleNow} 当场最多跑的片数：点选读音时多等这几片，换来这一帧就画出模型的顺序；没解完的照常在后面接着做。 */
    private static final int SETTLE_NOW_SLICES = 3;
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
    private final BooleanSupplier candidateHeld;
    private final BooleanSupplier panelOpen;
    private Runnable timer;
    /** 面板开着时停下的那一步；{@link #resume()} 接着做，{@link #cancel()} 清掉。 */
    private long parkedGeneration = -1;
    private long parkedSession;
    /** 最近一次回复说挂着延后模型的会话句柄；0 表示还不知道或没有。 */
    private long deferredSession;
    private long epoch;
    private boolean rendering;
    private Job job;

    public PauseRerankController(Scheduler scheduler, Bridge<V> bridge, LongSupplier currentSession,
            Consumer<V> publish, Runnable onError) {
        this(scheduler, bridge, currentSession, publish, onError, () -> false);
    }
    /** {@code candidateHeld} 为真时有手指按着候选（或它的点击还在排队）。 */
    public PauseRerankController(Scheduler scheduler, Bridge<V> bridge, LongSupplier currentSession,
            Consumer<V> publish, Runnable onError, BooleanSupplier candidateHeld) {
        this(scheduler, bridge, currentSession, publish, onError, candidateHeld, () -> false);
    }
    /** {@code panelOpen} 为真时展开面板开着。 */
    public PauseRerankController(Scheduler scheduler, Bridge<V> bridge, LongSupplier currentSession,
            Consumer<V> publish, Runnable onError, BooleanSupplier candidateHeld, BooleanSupplier panelOpen) {
        this.scheduler = scheduler;
        this.bridge = bridge;
        this.currentSession = currentSession;
        this.publish = publish;
        this.onError = onError;
        this.candidateHeld = candidateHeld;
        this.panelOpen = panelOpen;
    }
    /** 用户的一次输入之后调用：从这一刻重新计时。 */
    public void schedule(boolean eligible) {
        if (rendering) return;
        cancel();
        long handle = currentSession.getAsLong();
        if (handle != 0 && eligible)
            post(epoch, handle, handle == deferredSession ? DEFERRED_QUIET_MILLIS : QUIET_MILLIS);
    }
    /** 输入以外的重绘调用：不再适用时取消，否则让已经排着的停顿照原来的时刻进行。 */
    public void keep(boolean eligible) {
        if (rendering) return;
        long handle = currentSession.getAsLong();
        if (!eligible || handle == 0 || (job != null && job.session != handle)) cancel();
    }
    /**
     * 用户点选了读音这类不是打字的输入之后调用：会话挂着延后的模型时，当场重解最多 {@link #SETTLE_NOW_SLICES} 片，只发布最后一片的视图。调用方在同一次主线程回调里画出结果，不带模型的那一帧就不会出现在屏幕上。没解完的照常排下一片，后台评分和手指按着候选时什么都不做（计时照旧）。
     */
    public void settleNow() {
        long handle = currentSession.getAsLong();
        if (rendering || handle == 0 || handle != deferredSession || job != null
                || candidateHeld.getAsBoolean()) return;
        cancel();
        long generation = epoch;
        V last = null;
        Reply<V> result = null;
        for (int slice = 0; slice < SETTLE_NOW_SLICES; slice++) {
            result = step(handle);
            if (result == null) return;
            if (result.view() != null) last = result.view();
            if (!"continue".equals(result.status())) break;
        }
        publishView(last);
        follow(generation, handle, result.status());
    }
    /** 面板收起了：面板开着时停下的那一步接着做；没有停下的什么也不做。 */
    public void resume() {
        long generation = parkedGeneration;
        long handle = parkedSession;
        parkedGeneration = -1;
        if (generation >= 0 && current(generation, handle) && timer == null) post(generation, handle, 0);
    }
    public void cancel() {
        epoch++;
        parkedGeneration = -1;
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
        // 按着候选时不起跑也不接续：松手（连同排队的点击）之后再做。归还取消的评分不经这里，照常进行。
        if (candidateHeld.getAsBoolean()) {
            post(generation, handle, POLL_MILLIS);
            return;
        }
        if (panelOpen.getAsBoolean()) {
            parkedGeneration = generation;
            parkedSession = handle;
            return;
        }
        Reply<V> result = step(handle);
        if (result == null) return;
        publishView(result.view());
        follow(generation, handle, result.status());
    }
    /** 调一次原生接口（新起一片或收集后台评分），记下 deferred；读不懂回复时取消并报错，返回 null。 */
    private Reply<V> step(long handle) {
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
            return null;
        }
        if (result.deferred()) deferredSession = handle;
        else if (deferredSession == handle) deferredSession = 0;
        return result;
    }
    private void publishView(V view) {
        if (view == null) return;
        rendering = true;
        try { publish.accept(view); }
        finally { rendering = false; }
    }
    private void follow(long generation, long handle, String status) {
        if (!current(generation, handle)) return;
        if ("continue".equals(status)) post(generation, handle, 0);
        else if ("pending".equals(status)) post(generation, handle, POLL_MILLIS);
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

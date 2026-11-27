package app.msime.android;

import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;
import java.util.function.BooleanSupplier;

/**
 * 手指按着候选时，把会换掉候选代次的异步结果留到松手之后再用；展开面板开着时，云/AI 候选再留到面板收起或用户点「刷新」。
 *
 * <p>按着候选时候选条不重画，按钮上留着按下时的候选和身份；这时换代，松手的点击就落到过期的身份上，什么也选不中，或按槽位选到另一个词。等松手用轮询：松手后排队的点击还要先执行完，按压标记在那之后才摘掉，所以「松手」这件事本身不是可靠的时刻。
 *
 * <p>面板开着不轮询：面板会开很久，收起和「刷新」都是宿主自己知道的时刻，由它调 {@link #release()}。排在面板后面等的任务挡住它后面的任务，先后顺序不变。只在主线程上用。
 */
public final class CandidateHoldGate {
    public interface Scheduler {
        void post(Runnable task, long delayMillis);
    }

    static final long RECHECK_MILLIS = 16;
    private static final class Waiting {
        final Object tag;
        final Runnable task;
        final BooleanSupplier abandoned;
        /** 只等松手；面板开着也照常执行。 */
        boolean released;
        Waiting(Object tag, Runnable task, BooleanSupplier abandoned, boolean released) {
            this.tag = tag;
            this.task = task;
            this.abandoned = abandoned;
            this.released = released;
        }
    }
    private final Scheduler scheduler;
    private final BooleanSupplier held;
    private final BooleanSupplier panelOpen;
    private final ArrayDeque<Waiting> waiting = new ArrayDeque<>();
    private boolean checking;
    /** 排出去的复查里只有最新的一次算数：放行要马上复查，不能等已经排着的那次一帧后的复查。 */
    private long recheckToken;

    public CandidateHoldGate(Scheduler scheduler, BooleanSupplier held) {
        this(scheduler, held, () -> false);
    }

    public CandidateHoldGate(Scheduler scheduler, BooleanSupplier held, BooleanSupplier panelOpen) {
        this.scheduler = scheduler;
        this.held = held;
        this.panelOpen = panelOpen;
    }

    /** 没按着、也没有更早的在等时当场执行；否则排在后面，每隔 {@link #RECHECK_MILLIS} 再看一次，松手后按排队的先后执行。{@code abandoned} 变成真之后不再执行。 */
    public void runWhenFree(Runnable task, BooleanSupplier abandoned) {
        enqueue(new Waiting(null, task, abandoned, true));
    }

    /** 同 {@link #runWhenFree}，另外面板开着时等到 {@link #release()}。{@code tag} 由 {@link #waitingTags()} 报给提示。 */
    public void runWhenPanelFree(Object tag, Runnable task, BooleanSupplier abandoned) {
        enqueue(new Waiting(tag, task, abandoned, false));
    }

    /** 面板收起了，或用户点了「刷新」：此刻在等面板的都放行（仍要等松手），之后来的照常等。 */
    public void release() {
        boolean any = false;
        for (Waiting next : waiting) {
            any |= !next.released;
            next.released = true;
        }
        if (any) recheckNow();
    }

    /** 正在等面板、还没放弃的任务带的标记，按排队的先后。 */
    public List<Object> waitingTags() {
        List<Object> tags = new ArrayList<>();
        for (Waiting next : waiting)
            if (!next.released && next.tag != null && !next.abandoned.getAsBoolean()) tags.add(next.tag);
        return tags;
    }

    private void enqueue(Waiting next) {
        if (next.abandoned.getAsBoolean()) return;
        if (waiting.isEmpty() && free(next)) {
            next.task.run();
            return;
        }
        waiting.add(next);
        recheckLater(RECHECK_MILLIS);
    }

    private boolean free(Waiting next) {
        return !held.getAsBoolean() && (next.released || !panelOpen.getAsBoolean());
    }

    private void recheckLater(long delay) {
        if (checking) return;
        schedule(delay);
    }

    /** 已经排着一次延后的复查也重排一次马上的，旧的那次到时什么也不做。 */
    private void recheckNow() {
        schedule(0);
    }

    private void schedule(long delay) {
        checking = true;
        long token = ++recheckToken;
        scheduler.post(() -> {
            if (token == recheckToken) recheck();
        }, delay);
    }

    private void recheck() {
        checking = false;
        while (!waiting.isEmpty()) {
            Waiting next = waiting.peek();
            if (next.abandoned.getAsBoolean()) {
                waiting.poll();
                continue;
            }
            if (!free(next)) break;
            waiting.poll();
            next.task.run();
        }
        // 头一个只在等面板：不轮询，等 release()。在等松手才隔一帧再看。
        if (!waiting.isEmpty() && held.getAsBoolean()) recheckLater(RECHECK_MILLIS);
    }
}

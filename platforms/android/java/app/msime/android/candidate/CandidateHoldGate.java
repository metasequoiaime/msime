package app.msime.android;

import java.util.ArrayDeque;
import java.util.function.BooleanSupplier;

/**
 * 手指按着候选时，把会换掉候选代次的异步结果留到松手之后再用。
 *
 * <p>按着候选时候选条不重画，按钮上留着按下时的候选和身份；这时换代，松手的点击就落到过期的身份上，什么也选不中，或按槽位选到另一个词。等待用轮询：松手后排队的点击还要先执行完，按压标记在那之后才摘掉，所以「松手」这件事本身不是可靠的时刻。只在主线程上用。
 */
public final class CandidateHoldGate {
    public interface Scheduler {
        void post(Runnable task, long delayMillis);
    }

    static final long RECHECK_MILLIS = 16;
    private record Waiting(Runnable task, BooleanSupplier abandoned) {}
    private final Scheduler scheduler;
    private final BooleanSupplier held;
    private final ArrayDeque<Waiting> waiting = new ArrayDeque<>();
    private boolean checking;

    public CandidateHoldGate(Scheduler scheduler, BooleanSupplier held) {
        this.scheduler = scheduler;
        this.held = held;
    }

    /** 没按着、也没有更早的在等时当场执行；否则排在后面，每隔 {@link #RECHECK_MILLIS} 再看一次，松手后按排队的先后执行。{@code abandoned} 变成真之后不再执行。 */
    public void runWhenFree(Runnable task, BooleanSupplier abandoned) {
        if (abandoned.getAsBoolean()) return;
        if (waiting.isEmpty() && !held.getAsBoolean()) {
            task.run();
            return;
        }
        waiting.add(new Waiting(task, abandoned));
        recheckLater();
    }

    private void recheckLater() {
        if (checking) return;
        checking = true;
        scheduler.post(this::recheck, RECHECK_MILLIS);
    }

    private void recheck() {
        checking = false;
        while (!waiting.isEmpty() && !held.getAsBoolean()) {
            Waiting next = waiting.poll();
            if (!next.abandoned().getAsBoolean()) next.task().run();
        }
        if (!waiting.isEmpty()) recheckLater();
    }
}

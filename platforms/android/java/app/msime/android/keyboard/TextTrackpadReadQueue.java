package app.msime.android;

import java.util.concurrent.Executor;
import java.util.function.Consumer;
import java.util.function.Supplier;

/** 主线程持有的编辑器回读队列：一个在途、一个最新待处理，取消后不再发布旧结果。 */
public final class TextTrackpadReadQueue<T> {
    private final Executor worker;
    private final Executor main;
    private long generation;
    private boolean busy;
    private boolean closed;
    private Job<T> pending;

    private record Job<T>(long generation, Supplier<T> read, Consumer<T> publish) { }

    public TextTrackpadReadQueue(Executor worker, Executor main) {
        this.worker = worker;
        this.main = main;
    }

    /** 从主线程提交；`read` 在编辑器边界自行处理连接异常，`publish` 总在主线程执行。 */
    public void request(Supplier<T> read, Consumer<T> publish) {
        if (closed) return;
        pending = new Job<>(++generation, read, publish);
        startPending();
    }

    public void cancel() {
        generation++;
        pending = null;
    }

    public void close() {
        closed = true;
        cancel();
    }

    private void startPending() {
        if (closed || busy || pending == null) return;
        Job<T> job = pending;
        pending = null;
        busy = true;
        worker.execute(() -> {
            T value = job.read().get();
            main.execute(() -> {
                busy = false;
                if (!closed && job.generation() == generation) job.publish().accept(value);
                startPending();
            });
        });
    }
}

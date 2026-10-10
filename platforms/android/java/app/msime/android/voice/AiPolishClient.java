package app.msime.android;

import java.util.Objects;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.Future;
import java.util.concurrent.RejectedExecutionException;
import java.util.concurrent.ThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

/** One-at-a-time asynchronous AI request coordinator with cancellation and stale-result rejection. */
public final class AiPolishClient implements AutoCloseable {
    public enum Reason { INVALID, UNAVAILABLE, BUSY, CANCELLED }

    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        private final Reason reason;
        public Failure(Reason reason) { this.reason = Objects.requireNonNull(reason); }
        public Failure(Reason reason, Throwable cause) { super(cause); this.reason = Objects.requireNonNull(reason); }
        public Reason reason() { return reason; }
    }

    public static final class Cancellation {
        private final AtomicBoolean cancelled = new AtomicBoolean();
        private Runnable abort;

        public boolean cancelled() { return cancelled.get() || Thread.currentThread().isInterrupted(); }

        synchronized void attach(Runnable action) {
            if (cancelled()) action.run();
            else abort = action;
        }

        synchronized void detach() { abort = null; }

        synchronized void cancel() {
            cancelled.set(true);
            if (abort != null) abort.run();
            abort = null;
        }
    }

    @FunctionalInterface public interface Transport {
        String send(AiPolishConfiguration configuration, String text, Cancellation cancellation)
            throws Failure;
    }

    @FunctionalInterface public interface Callback {
        void complete(long generation, String result, Failure failure);
    }

    public final class Operation {
        private final long generation;
        private final Cancellation cancellation;
        private Future<?> future;

        private Operation(long generation, Cancellation cancellation) {
            this.generation = generation;
            this.cancellation = cancellation;
        }

        public long generation() { return generation; }
        public void cancel() { cancelOperation(this); }
    }

    private final Transport transport;
    private final ThreadPoolExecutor worker;
    private long generation;
    private Operation active;
    private boolean closed;

    public AiPolishClient(Transport transport) {
        this.transport = Objects.requireNonNull(transport);
        worker = new ThreadPoolExecutor(1, 1, 0, TimeUnit.MILLISECONDS,
            new ArrayBlockingQueue<>(1), ThreadPolicy.namedDaemonFactory("msime-ai-polish"),
            new ThreadPoolExecutor.AbortPolicy());
    }

    public synchronized Operation request(AiPolishConfiguration configuration, String text,
                                          Callback callback) throws Failure {
        Objects.requireNonNull(configuration);
        Objects.requireNonNull(callback);
        if (closed) throw new Failure(Reason.UNAVAILABLE);
        if (!AiPolishConfiguration.acceptableText(text)) throw new Failure(Reason.INVALID);
        if (active != null) active.cancel();
        Operation operation = new Operation(++generation, new Cancellation());
        active = operation;
        try {
            operation.future = worker.submit(() -> execute(operation, configuration, text, callback));
        } catch (RejectedExecutionException error) {
            active = null;
            throw new Failure(Reason.BUSY, error);
        }
        return operation;
    }

    private void execute(Operation operation, AiPolishConfiguration configuration, String text,
                         Callback callback) {
        String result = null;
        Failure failure = null;
        try {
            result = transport.send(configuration, text, operation.cancellation);
            if (operation.cancellation.cancelled()) throw new Failure(Reason.CANCELLED);
            if (!AiPolishConfiguration.acceptableText(result)) throw new Failure(Reason.INVALID);
        } catch (Failure error) {
            failure = error;
        } catch (RuntimeException error) {
            failure = new Failure(Reason.UNAVAILABLE, error);
        }
        synchronized (this) {
            if (closed || active != operation || operation.cancellation.cancelled()) return;
            active = null;
        }
        callback.complete(operation.generation, result, failure);
    }

    private synchronized void cancelOperation(Operation operation) {
        operation.cancellation.cancel();
        if (operation.future != null) operation.future.cancel(true);
        worker.purge();
        if (active == operation) {
            active = null;
            generation++;
        }
    }

    public synchronized boolean isCurrent(long value) {
        return !closed && active != null && active.generation == value;
    }

    @Override public synchronized void close() {
        if (closed) return;
        closed = true;
        if (active != null) active.cancel();
        worker.shutdownNow();
    }
}

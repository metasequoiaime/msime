package app.msime.android;

import android.content.Context;
import android.os.Handler;
import android.os.Looper;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * 首次启动时准备词库: the one place first-run resource preparation is triggered from.
 *
 * Preparation used to live behind a button on a development launcher screen, which meant a user who
 * never found that screen had a keyboard that could not reach the Engine. Both launchers — the
 * native host and the Tauri bundle — call this instead, and {@link Bootstrap#prepare} is itself
 * idempotent: an existing configuration is reported, never overwritten.
 */
public final class FirstRunPreparation {

    /** What the surfaces show. Preparation is not something the user asked for, so it is quiet unless it needs the screen. */
    public enum State { IDLE, RUNNING, READY, FAILED }

    public interface Listener {
        void onPreparationState(State state);
    }

    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor();
    private static final Handler MAIN = new Handler(Looper.getMainLooper());
    private static final AtomicBoolean RUNNING = new AtomicBoolean();
    private static volatile State state = State.IDLE;
    private static volatile Listener listener;

    private FirstRunPreparation() { }

    public static State state() { return state; }

    /** Observes the current and subsequent states. One surface at a time; passing null detaches. */
    public static void observe(Listener target) {
        listener = target;
        if (target != null) target.onPreparationState(state);
    }

    /**
     * Prepares the shipped dictionary unless a configuration already exists or a run is in flight.
     * Safe to call from every launcher on every start.
     */
    public static void startIfNeeded(Context context) {
        if (state == State.READY) return;
        if (!RUNNING.compareAndSet(false, true)) return;
        Context application = context.getApplicationContext();
        publish(State.RUNNING);
        WORKER.execute(() -> {
            State outcome;
            try {
                // Both true (prepared now) and false (a configuration was already there) leave the
                // keyboard able to reach the Engine, which is the only thing the surfaces report.
                Bootstrap.prepare(application);
                outcome = State.READY;
            } catch (Exception | LinkageError error) {
                // Bootstrap has no editor or session input; never use this logging for keystrokes.
                android.util.Log.e("MSIMEBootstrap", "First-run resource preparation failed", error);
                outcome = State.FAILED;
            }
            RUNNING.set(false);
            publish(outcome);
        });
    }

    /** Runs again after a failure. Does nothing while a run is in flight. */
    public static void retry(Context context) {
        if (state == State.RUNNING) return;
        state = State.IDLE;
        startIfNeeded(context);
    }

    private static void publish(State next) {
        state = next;
        MAIN.post(() -> {
            Listener target = listener;
            if (target != null) target.onPreparationState(next);
        });
    }
}

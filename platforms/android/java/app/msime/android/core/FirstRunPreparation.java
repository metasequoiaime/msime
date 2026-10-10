package app.msime.android;

import android.content.Context;
import android.os.Handler;
import java.util.concurrent.CopyOnWriteArraySet;
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

    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedFactory("msime-first-run"));
    private static final Handler MAIN = MainThreadPolicy.mainHandler();
    private static final AtomicBoolean RUNNING = new AtomicBoolean();
    private static volatile State state = State.IDLE;
    private static volatile String failure = "";
    private static final CopyOnWriteArraySet<Listener> LISTENERS = new CopyOnWriteArraySet<>();

    private FirstRunPreparation() { }

    public static State state() { return state; }

    /** 最近一次失败的原因（{@link PreparationFailure#describe}），没有失败时是空串。 */
    public static String failure() { return failure; }

    /** 观察当前及之后的状态，直到用同一个 listener 调用 {@link #stopObserving}。每个等待词库的界面各自观察：引导页叠在键盘页签之上打开，两者都需要知道准备已经完成。 */
    public static void observe(Listener target) {
        LISTENERS.add(target);
        target.onPreparationState(state);
    }

    public static void stopObserving(Listener target) {
        LISTENERS.remove(target);
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
                failure = "";
            } catch (Exception | LinkageError error) {
                // Bootstrap has no editor or session input; never use this logging for keystrokes.
                android.util.Log.e("MSIMEBootstrap", "First-run resource preparation failed", error);
                failure = PreparationFailure.describe(error);
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
            for (Listener target : LISTENERS) target.onPreparationState(next);
        });
    }
}

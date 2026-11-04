package app.msime.android;

import java.util.ArrayList;
import java.util.List;

public final class PauseRerankDiscardSmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    /** Timers the test runs by hand, in order. */
    static final class ManualClock implements PauseRerankDiscard.Clock {
        final List<Runnable> queued = new ArrayList<>();
        public Object schedule(Runnable task, long delayMillis) { queued.add(task); return task; }
        public void cancel(Object handle) { queued.remove(handle); }
        void runAll() { List<Runnable> now = new ArrayList<>(queued); queued.clear(); now.forEach(Runnable::run); }
    }

    public static void main(String[] args) {
        // A cancelled job is polled until the core says done, and blocks a new pause until then.
        List<String> replies = new ArrayList<>(List.of("pending", "pending", "done"));
        List<Long> polled = new ArrayList<>();
        long[] session = {7};
        ManualClock clock = new ManualClock();
        PauseRerankDiscard discard = new PauseRerankDiscard((s, t) -> { polled.add(t); return replies.remove(0); }, () -> session[0], clock);
        discard.discard(7, 3);
        check(discard.waiting(7), "waits while the job is pending");
        clock.runAll();
        check(discard.waiting(7), "still pending after the second poll");
        clock.runAll();
        check(!discard.waiting(7), "released once the core answers done");
        check(polled.equals(List.of(3L, 3L, 3L)), "polled the cancelled token each time: " + polled);
        check(clock.queued.isEmpty(), "no timer left behind");

        // The same token handed over twice is polled once; an unreadable reply keeps polling.
        polled.clear();
        String[] answer = {null};
        PauseRerankDiscard again = new PauseRerankDiscard((s, t) -> { polled.add(t); return answer[0]; }, () -> session[0], clock);
        again.discard(7, 5);
        again.discard(7, 5);
        check(polled.size() == 1, "a repeated token does not start a second poll");
        clock.runAll();
        check(again.waiting(7), "an unreadable reply is not a release");

        // A destroyed or replaced session drops the job without polling it again.
        session[0] = 9;
        clock.runAll();
        check(!again.waiting(9), "a different session has nothing pending");
        check(clock.queued.isEmpty(), "polling stops once the session is gone");

        // A newer cancelled job replaces the older one and its timer.
        session[0] = 7;
        answer[0] = "pending";
        again.discard(7, 11);
        again.discard(7, 12);
        check(clock.queued.size() == 1, "only the newest job keeps a timer");
        answer[0] = "done";
        clock.runAll();
        check(!again.waiting(7), "the newest job is released");

        // Zero handles and tokens are ignored.
        again.discard(0, 4);
        again.discard(7, 0);
        check(!again.waiting(7) && clock.queued.isEmpty(), "zero session or token starts nothing");
        System.out.println("pause rerank discard: release on done, same-token no-op, session change, replacement");
    }
}

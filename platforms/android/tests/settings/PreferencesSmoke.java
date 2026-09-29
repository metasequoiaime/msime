import app.msime.android.PreferencesReloader;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

public final class PreferencesSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    public static void main(String[] args) {
        ArrayDeque<Runnable> main = new ArrayDeque<>();
        ArrayDeque<Runnable> worker = new ArrayDeque<>();
        List<String> received = new ArrayList<>();
        List<String> reads = new ArrayList<>();
        PreferencesReloader reloader = new PreferencesReloader((task, delay) -> main.add(task), worker::add, directory -> {
            reads.add(directory);
            if (directory.equals("bad")) throw new IllegalStateException("synthetic read failure");
            return directory;
        });
        reloader.start("old", received::add);
        check(reads.isEmpty()); // No file IO on the caller/session thread.
        reloader.stop();
        reloader.start("new", received::add);
        check(worker.size() == 1); // The old blocked read still owns the worker slot.
        worker.remove().run();
        main.remove().run(); // New session must wait for the old completion.
        check(worker.isEmpty());
        main.remove().run(); // Old completion is discarded, releases slot.
        check(received.isEmpty());
        main.remove().run();
        worker.remove().run();
        main.remove().run();
        check(received.equals(List.of("new")));
        reloader.stop();
        main.remove().run();
        check(worker.isEmpty());
        reloader.start("bad", received::add);
        worker.remove().run();
        main.remove().run();
        check(received.size() == 2 && received.get(1) == null);
        main.remove().run(); // Failed reads continue polling.
        check(worker.size() == 1);
        reloader.stop();
        worker.remove().run();
        main.remove().run();
        check(main.isEmpty() && received.size() == 2);
        ArrayDeque<Runnable> retryMain = new ArrayDeque<>();
        PreferencesReloader rejected = new PreferencesReloader(
            (task, delay) -> retryMain.add(task), task -> { throw new IllegalStateException("full"); },
            directory -> directory);
        rejected.start("retry", received::add);
        check(retryMain.size() == 1); // Rejected work must clear reading and schedule a retry.
        retryMain.remove().run();
        check(retryMain.size() == 1);
        rejected.stop();
        System.out.println("Android preferences scheduling: background IO, stale completion, stop/restart, non-overlap and error retry passed");
    }
}

import app.msime.android.PreferencesReloader;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.List;

public final class PreferencesSmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }
    /** 键盘每次显示会连着 start 两三次（onWindowShown、32 ms 后的输入视图刷新、建会话），同一目录只读一次；只有每轮第一次读取是 fresh。 */
    static void checkSingleReadPerShow() {
        ArrayDeque<Runnable> main = new ArrayDeque<>();
        ArrayDeque<Runnable> worker = new ArrayDeque<>();
        List<Long> delays = new ArrayList<>();
        List<String> reads = new ArrayList<>();
        List<String> first = new ArrayList<>();
        List<String> second = new ArrayList<>();
        PreferencesReloader reloader = new PreferencesReloader((task, delay) -> { main.add(task); delays.add(delay); },
            worker::add, (directory, fresh) -> {
                reads.add(directory + (fresh ? ":fresh" : ":poll"));
                return directory;
            });
        reloader.start("prefs", first::add);
        reloader.start("prefs", second::add); // Joins the pending first read instead of starting over.
        check(worker.size() == 1 && main.isEmpty());
        worker.remove().run();
        main.remove().run();
        check(reads.equals(List.of("prefs:fresh")));
        check(first.isEmpty() && second.equals(List.of("prefs"))); // The latest receiver gets the joined read.
        check(main.size() == 1 && delays.get(delays.size() - 1) == 1000);
        main.remove().run();
        worker.remove().run();
        main.remove().run();
        check(reads.equals(List.of("prefs:fresh", "prefs:poll"))); // Later polls may use the reader's cache.
        // After the first read was delivered, a new appearance boundary reads fresh again at once.
        reloader.start("prefs", second::add);
        check(worker.size() == 1);
        worker.remove().run();
        main.remove().run(); // Previous generation's scheduled poll, now stale.
        main.remove().run();
        check(reads.get(reads.size() - 1).equals("prefs:fresh") && second.size() == 3);
        // A different directory never joins a pending read.
        reloader.start("other", second::add);
        reloader.start("next", second::add);
        check(worker.size() == 1);
        reloader.stop();
        main.clear();
        worker.clear();
        // A caller that has just read the live preferences delays the first poll, which is not fresh.
        PreferencesReloader delayed = new PreferencesReloader((task, delay) -> { main.add(task); delays.add(delay); },
            worker::add, (directory, fresh) -> {
                reads.add(directory + (fresh ? ":fresh" : ":poll"));
                return directory;
            });
        delayed.startAfter("live", second::add, 1000);
        check(worker.isEmpty() && main.size() == 1 && delays.get(delays.size() - 1) == 1000);
        main.remove().run();
        worker.remove().run();
        main.remove().run();
        check(reads.get(reads.size() - 1).equals("live:poll") && second.get(second.size() - 1).equals("live"));
        // A delayed start does not count as a pending fresh read: the next boundary still reads at once.
        delayed.start("live", second::add);
        check(worker.size() == 1);
        worker.remove().run();
        check(reads.get(reads.size() - 1).equals("live:fresh"));
        delayed.stop();
    }

    public static void main(String[] args) {
        ArrayDeque<Runnable> main = new ArrayDeque<>();
        ArrayDeque<Runnable> worker = new ArrayDeque<>();
        List<String> received = new ArrayList<>();
        List<String> reads = new ArrayList<>();
        PreferencesReloader reloader = new PreferencesReloader((task, delay) -> main.add(task), worker::add, (directory, fresh) -> {
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
            (directory, fresh) -> directory);
        rejected.start("retry", received::add);
        check(retryMain.size() == 1); // Rejected work must clear reading and schedule a retry.
        retryMain.remove().run();
        check(retryMain.size() == 1);
        rejected.stop();
        checkSingleReadPerShow();
        System.out.println("Android preferences scheduling: background IO, stale completion, stop/restart, non-overlap, error retry, one read per show and fresh first read passed");
    }
}

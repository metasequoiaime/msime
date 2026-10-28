package app.msime.android;

import java.util.concurrent.Executor;
import java.util.function.Consumer;

/** Platform scheduling only; decoding, revisions and composition deferral belong to Rust.
 * start/stop and scheduled completions must run on the session thread. No session
 * handle or editor is ever accessed by the reader worker.
 */
public final class PreferencesReloader {
    public interface Scheduler { void post(Runnable task, long delayMillis); }
    /** {@code fresh} is true for the first read after {@link #start}, which must really read; the later once-a-second polls may hand back the previous result when nothing on disk changed. */
    public interface Reader { String read(String directory, boolean fresh) throws Exception; }
    private final Scheduler main;
    private final Executor worker;
    private final Reader reader;
    private long generation;
    private boolean reading;
    /** The directory being polled; null after stop. */
    private String directory;
    private Consumer<String> receive;
    /** The first read of the current generation has not been delivered yet. */
    private boolean firstPending;

    public PreferencesReloader(Scheduler main, Executor worker, Reader reader) {
        this.main = main;
        this.worker = worker;
        this.reader = reader;
    }

    /** Read now, then poll once a second. While the first read of a running generation for the same directory is still pending, a second start joins it instead of starting over: every keyboard show calls this from onWindowShown and again from the input-view refresh 32 ms later, and the second call used to either throw away the first read or read and apply the whole snapshot again right after it. */
    public void start(String directory, Consumer<String> receive) {
        if (firstPending && directory.equals(this.directory)) {
            this.receive = receive;
            return;
        }
        long token = begin(directory, receive);
        firstPending = true;
        poll(token, true);
    }

    /** For a caller that has just read the live preferences itself: polling starts after {@code delayMillis}, and that first poll is not a fresh read. */
    public void startAfter(String directory, Consumer<String> receive, long delayMillis) {
        long token = begin(directory, receive);
        firstPending = false;
        main.post(() -> poll(token, false), delayMillis);
    }

    public void stop() {
        generation++;
        directory = null;
        receive = null;
        firstPending = false;
    }

    private long begin(String directory, Consumer<String> receive) {
        long token = ++generation;
        this.directory = directory;
        this.receive = receive;
        return token;
    }

    private void poll(long token, boolean fresh) {
        if (token != generation) return;
        if (reading) {
            main.post(() -> poll(token, fresh), 1000);
            return;
        }
        reading = true;
        String target = directory;
        Runnable task = () -> {
            String result;
            try { result = reader.read(target, fresh); }
            catch (Exception | LinkageError error) { result = null; }
            final String response = result;
            main.post(() -> {
                reading = false;
                if (token != generation) return;
                if (fresh) firstPending = false;
                try { receive.accept(response); }
                finally { main.post(() -> poll(token, false), 1000); }
            }, 0);
        };
        try {
            worker.execute(task);
        } catch (RuntimeException error) {
            reading = false;
            if (token == generation)
                main.post(() -> poll(token, fresh), 1000);
        }
    }
}

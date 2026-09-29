package app.msime.android;

import java.util.concurrent.Executor;
import java.util.function.Consumer;

/** Platform scheduling only; decoding, revisions and composition deferral belong to Rust.
 * start/stop and scheduled completions must run on the session thread. No session
 * handle or editor is ever accessed by the reader worker.
 */
public final class PreferencesReloader {
    public interface Scheduler { void post(Runnable task, long delayMillis); }
    public interface Reader { String read(String directory) throws Exception; }
    private final Scheduler main;
    private final Executor worker;
    private final Reader reader;
    private long generation;
    private boolean reading;

    public PreferencesReloader(Scheduler main, Executor worker, Reader reader) {
        this.main = main;
        this.worker = worker;
        this.reader = reader;
    }
    public void start(String directory, Consumer<String> receive) {
        long token = ++generation;
        poll(token, directory, receive);
    }
    public void stop() { generation++; }
    private void poll(long token, String directory, Consumer<String> receive) {
        if (token != generation) return;
        if (reading) {
            main.post(() -> poll(token, directory, receive), 1000);
            return;
        }
        reading = true;
        Runnable task = () -> {
            String result;
            try { result = reader.read(directory); }
            catch (Exception | LinkageError error) { result = null; }
            final String response = result;
            main.post(() -> {
                reading = false;
                if (token != generation) return;
                try { receive.accept(response); }
                finally { main.post(() -> poll(token, directory, receive), 1000); }
            }, 0);
        };
        try {
            worker.execute(task);
        } catch (RuntimeException error) {
            reading = false;
            if (token == generation)
                main.post(() -> poll(token, directory, receive), 1000);
        }
    }
}

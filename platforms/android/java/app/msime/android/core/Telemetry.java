package app.msime.android.core;

import app.msime.android.AppVersionPolicy;
import app.msime.android.TextPolicy;
import app.msime.android.JsonPolicy;
import android.content.Context;
import android.util.Log;
import app.msime.android.NativeClient;
import app.msime.android.ThreadPolicy;
import app.msime.android.policy.HostOptionsPolicy;
import java.io.File;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;

/**
 * Android 应用和键盘的匿名使用情况上报，建立在 client-services 的共享上报器之上（msime_client.h 中的 msime_client_telemetry_*）。
 *
 * <p>队列、随机安装 id、每日 `active`、会话标记和网络都归共享存储负责：两个进程把它指向同一个目录，由它用文件锁把两者串行化。这个类只决定何时调用它，以及写崩溃记录，这是唯一一件必须在即将崩溃的进程内完成的事。
 *
 * <p>会话属于键盘进程（`:ime`）：每个进程生命周期一个，在 {@link #beginInputSession} 中开始，在 {@link #endInputSession} 中结束。应用进程只负责刷出队列和记录崩溃。键盘里的崩溃写进该会话自己的记录，所以下次启动时报告为 `crash` 加 `session_crash`；应用里的崩溃写成一条独立记录，在键盘下次启动时报告为 `crash`。只留下标记的会话（进程被系统杀掉）不算崩溃。
 *
 * <p>是否同意以共享的 `usage_reporting` 偏好为准（默认开启）。关闭时共享存储会清空队列、标记和崩溃记录，不发送任何东西。
 */
public final class Telemetry {
    private static final String TAG = "MSIMETelemetry";
    private static final String DIRECTORY = "telemetry";
    /** 必须与 crates/client-services/src/telemetry.rs 中的 CRASH_DIRECTORY 一致：那里的每个 `*.crash` 文件都会在下次会话开始时变成一条崩溃事件。 */
    static final String CRASH_DIRECTORY = "telemetry-crashes";
    static final String CRASH_EXTENSION = ".crash";
    /** The server's limits (message 1000 Unicode scalars, stack 16000) and the stack's byte cap that keeps an event under 32 KiB. The shared store applies them again when it reads the record; clipping here keeps the dying process's write small. */
    static final int MAX_MESSAGE_CODE_POINTS = 1000;
    static final int MAX_STACK_CODE_POINTS = 16_000;
    static final int MAX_STACK_BYTES = 12 * 1024;
    private static final int MAX_CAUSES = 4;
    /** The keyboard process can live for days; a flush when the keyboard is shown, at most this often, sends what is queued and counts a new day's `active`. */
    private static final long INPUT_FLUSH_INTERVAL_MILLIS = 6L * 60 * 60 * 1000;
    private static final long END_WAIT_MILLIS = 1500;
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-telemetry"));
    private static boolean crashHandlerInstalled;
    /** Where a crash in this process is written: the session's own record in the keyboard process, null elsewhere. */
    private static volatile File sessionCrashRecord;
    private static volatile File crashDirectory;
    /** The last consent the shared store reported; a crash while reporting is off writes nothing. */
    private static volatile boolean enabled = true;
    private static long lastInputFlush;
    private static volatile boolean sessionBegun;

    private Telemetry() {}

    /** The app process (HomeActivity): crash handler, and a flush that also counts today's `active`. */
    public static void start(Context context) {
        Context app = context.getApplicationContext();
        crashDirectory = new File(directory(app), CRASH_DIRECTORY);
        installCrashHandler(app);
        WORKER.execute(() -> flush(app));
    }

    /** The keyboard process (MSIMEInputService.onCreate): crash handler, a new session, then a flush. */
    public static void beginInputSession(Context context) {
        Context app = context.getApplicationContext();
        crashDirectory = new File(directory(app), CRASH_DIRECTORY);
        installCrashHandler(app);
        lastInputFlush = System.currentTimeMillis();
        WORKER.execute(() -> {
            JSONObject value = call(() -> NativeClient.telemetryBegin(request(app)));
            if (value != null) {
                enabled = JsonPolicy.strictBoolean(value.opt("enabled"), false);
                String path = value.optString("crash_record_path", "");
                File candidate = path.isEmpty() ? null : new File(path);
                sessionCrashRecord = enabled && isSafeSessionCrashRecord(candidate)
                    ? candidate : null;
            }
            sessionBegun = true;
            flush(app);
        });
    }

    /** The keyboard was shown. Flushes at most every few hours; call on the main thread. */
    public static void inputViewShown(Context context) {
        long now = System.currentTimeMillis();
        if (now - lastInputFlush < INPUT_FLUSH_INTERVAL_MILLIS) return;
        lastInputFlush = now;
        Context app = context.getApplicationContext();
        WORKER.execute(() -> flush(app));
    }

    /** The keyboard process is shutting down normally (MSIMEInputService.onDestroy): queue its `session`. No network, only a small file write under the shared lock, so it runs right here once the session has begun; a begin still queued behind a flush is waited for briefly instead. */
    public static void endInputSession(Context context) {
        Context app = context.getApplicationContext();
        sessionCrashRecord = null;
        Runnable task = () -> call(() -> NativeClient.telemetryEnd(directoryRequest(app)));
        if (sessionBegun) {
            task.run();
            return;
        }
        Future<?> end = WORKER.submit(task);
        try {
            end.get(END_WAIT_MILLIS, TimeUnit.MILLISECONDS);
        } catch (Exception error) {
            // The session then reads as one the system ended, which is not counted as a crash.
            Log.i(TAG, "Session end did not finish before shutdown", error);
        }
    }

    /** The usage_reporting switch was saved. Turning it off clears everything queued at once rather than on the next start. */
    public static void setEnabled(Context context, boolean value) {
        enabled = value;
        Context app = context.getApplicationContext();
        if (!value) {
            sessionCrashRecord = null;
            WORKER.execute(() -> call(() -> NativeClient.telemetryClear(directoryRequest(app))));
        } else {
            WORKER.execute(() -> flush(app));
        }
    }

    static synchronized boolean installCrashHandler(Context app) {
        if (crashHandlerInstalled) return false;
        Thread.UncaughtExceptionHandler previous = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((thread, error) -> {
            try {
                if (enabled) writeCrashRecord(error);
            } catch (Throwable ignored) {
                // A failing report must not stand between the crash and the platform's own handling.
            }
            if (previous != null) previous.uncaughtException(thread, error);
        });
        crashHandlerInstalled = true;
        return true;
    }

    /** Synchronous: the process is about to be killed, so the record is on disk (and forced) before the previous handler runs. */
    private static void writeCrashRecord(Throwable error) throws Exception {
        File target = sessionCrashRecord;
        if (target != null && !isSafeSessionCrashRecord(target)) return;
        if (target == null) {
            File directory = crashDirectory;
            if (directory == null) return;
            if (!prepareCrashDirectory(directory)) return;
            target = new File(directory, UUID.randomUUID() + CRASH_EXTENSION);
        }
        byte[] record = TextPolicy.utf8Bytes(crashRecord(error));
        // CREATE_NEW: the first record of a session is kept, as the shared store's own writer does.
        try (FileChannel channel = FileChannel.open(target.toPath(),
                StandardOpenOption.WRITE, StandardOpenOption.CREATE_NEW,
                LinkOption.NOFOLLOW_LINKS)) {
            ByteBuffer buffer = ByteBuffer.wrap(record);
            while (buffer.hasRemaining()) channel.write(buffer);
            channel.force(true);
        }
    }

    /** The native begin response names the reserved record below our crash directory. Recheck it
     * before a crash write so a malformed response or a replaced parent cannot redirect the file. */
    private static boolean isSafeSessionCrashRecord(File target) {
        File directory = crashDirectory;
        if (target == null || directory == null) return false;
        try {
            Path root = directory.toPath().toAbsolutePath().normalize();
            Path raw = target.toPath();
            if (!raw.isAbsolute()) return false;
            Path path = raw.normalize();
            if (path.equals(root) || !path.startsWith(root)
                    || !Files.isDirectory(root, LinkOption.NOFOLLOW_LINKS)) return false;
            app.msime.android.SafePaths.rejectSymlinkComponents(path);
            return true;
        } catch (java.io.IOException | RuntimeException error) {
            return false;
        }
    }

    private static boolean prepareCrashDirectory(File directory) {
        try {
            Path path = directory.toPath().toAbsolutePath().normalize();
            app.msime.android.SafePaths.rejectSymlinkComponents(path);
            if (!Files.exists(path, LinkOption.NOFOLLOW_LINKS) && !directory.mkdirs()) return false;
            app.msime.android.SafePaths.rejectSymlinkComponents(path);
            return Files.isDirectory(path, LinkOption.NOFOLLOW_LINKS);
        } catch (java.io.IOException | RuntimeException error) {
            return false;
        }
    }

    /** The record format the shared store reads: the summary line, '\n', then the frames. Java frames name classes and source files, never a path. */
    static String crashRecord(Throwable error) {
        String summary = TextPolicy.clipCodePoints(firstLine(String.valueOf(error)), MAX_MESSAGE_CODE_POINTS);
        StringBuilder stack = new StringBuilder(MAX_STACK_CODE_POINTS);
        Throwable current = error;
        for (int depth = 0; current != null && depth <= MAX_CAUSES; depth++) {
            if (depth > 0) stack.append("Caused by: ").append(firstLine(String.valueOf(current))).append('\n');
            for (StackTraceElement frame : current.getStackTrace()) stack.append("at ").append(frame).append('\n');
            Throwable cause = current.getCause();
            current = cause == current ? null : cause;
        }
        return summary + "\n" + clipStack(stack.toString());
    }

    static String firstLine(String value) {
        if (value == null) return "";
        int end = value.indexOf('\n');
        String line = end < 0 ? value : value.substring(0, end);
        return line.endsWith("\r") ? line.substring(0, line.length() - 1) : line;
    }

    /** The stack within both the code-point limit and the byte cap, cut at the end of a line. */
    static String clipStack(String stack) {
        String clipped = TextPolicy.clipCodePoints(stack, MAX_STACK_CODE_POINTS);
        if (TextPolicy.utf8Length(clipped) <= MAX_STACK_BYTES
                && clipped.length() == stack.length()) return stack;
        int bytes = 0;
        int end = 0;
        int lineEnd = 0;
        for (int index = 0; index < clipped.length();) {
            int codePoint = clipped.codePointAt(index);
            int size = codePoint < 0x80 ? 1 : codePoint < 0x800 ? 2 : codePoint < 0x10000 ? 3 : 4;
            if (bytes + size > MAX_STACK_BYTES) break;
            bytes += size;
            index += Character.charCount(codePoint);
            end = index;
            if (codePoint == '\n') lineEnd = index;
        }
        return clipped.substring(0, lineEnd > 0 ? lineEnd : end);
    }

    private static void flush(Context app) {
        JSONObject value = call(() -> NativeClient.telemetryFlush(request(app)));
        if (value != null) enabled = JsonPolicy.strictBoolean(value.opt("enabled"), enabled);
    }

    private static File directory(Context app) {
        return new File(app.getFilesDir(), DIRECTORY);
    }

    private static String directoryRequest(Context app) throws Exception {
        File directory = directory(app);
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IllegalStateException("telemetry directory");
        return new JSONObject().put("directory", directory.getAbsolutePath()).toString();
    }

    /** The begin/flush request. Before first-run preparation there is no shared preferences file yet, and so no switch the user could have turned off: reporting is on, as its default says. */
    private static String request(Context app) throws Exception {
        JSONObject request = new JSONObject(directoryRequest(app))
            .put("platform", "android")
            .put("version", version(app));
        String preferences = HostOptionsPolicy.readOption(app.getFilesDir(), "preferences_directory");
        if (preferences.isEmpty()) request.put("enabled", true);
        else request.put("preferences_directory", preferences);
        return request.toString();
    }

    /** The real versionName (the release version build-apk.sh stamps), not a constant: crash groups and the per-version crash-free rate are keyed by it. */
    static String version(Context app) {
        try {
            String name = TextPolicy.trimmed(AppVersionPolicy.versionName(app));
            if (!name.isEmpty() && name.length() <= 64) return name;
        } catch (Exception error) {
            Log.i(TAG, "Package version unavailable", error);
        }
        return "unknown";
    }

    private interface Call {
        String run() throws Exception;
    }

    /** Reporting is best effort: a reporter that cannot load or answer is logged and skipped, never a failure of the keyboard. */
    private static JSONObject call(Call call) {
        try {
            JSONObject root = new JSONObject(call.run());
            if (JsonPolicy.strictTrue(root.opt("ok"))) {
                JSONObject value = root.optJSONObject("value");
                return value == null ? new JSONObject() : value;
            }
            Log.i(TAG, "Reporter refused: " + root.optString("error", ""));
        } catch (Exception | LinkageError error) {
            Log.i(TAG, "Reporter unavailable", error);
        }
        return null;
    }

}

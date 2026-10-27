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
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
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
    private static volatile String crashRecordHeader = "";
    private static volatile String sessionId = "";
    private static boolean inputProcess;
    private static final java.util.concurrent.atomic.AtomicLong consentGeneration = new java.util.concurrent.atomic.AtomicLong();
    private static volatile File crashDirectory;
    /** The last consent the shared store reported; a crash while reporting is off writes nothing. */
    private static volatile boolean enabled;
    private static long lastInputFlush;
    private static volatile boolean sessionBegun;

    private Telemetry() {}

    /** The app process (HomeActivity): crash handler, and a flush that also counts today's `active`. */
    public static void start(Context context) {
        Context app = context.getApplicationContext();
        crashDirectory = new File(directory(app), CRASH_DIRECTORY);
        installCrashHandler(app);
        installNativeCrashHandler();
        long generation = consentGeneration.get();
        WORKER.execute(() -> { prepareCapture(app, true, generation); collectSystemExits(app); flush(app); });
    }

    /** The keyboard process (MSIMEInputService.onCreate): crash handler, a new session, then a flush. */
    public static void beginInputSession(Context context) {
        inputProcess = true;
        Context app = context.getApplicationContext();
        crashDirectory = new File(directory(app), CRASH_DIRECTORY);
        installCrashHandler(app);
        installNativeCrashHandler();
        lastInputFlush = System.currentTimeMillis();
        long generation = consentGeneration.get();
        WORKER.execute(() -> {
            prepareCapture(app, false, generation);
            collectSystemExits(app);
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
        synchronized (Telemetry.class) {
            consentGeneration.incrementAndGet();
            sessionCrashRecord = null;
            crashRecordHeader = "";
            inputProcess = false;
            disarmNativeCrashHandler();
        }
        Runnable task = () -> call(() -> NativeClient.telemetryEnd(
            new JSONObject(directoryRequest(app)).put("session", sessionId).toString()));
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

    /** usage_reporting 已保存：关闭时同步撤销历史诊断身份并清空队列，重开只在准备完成后启用。 */
    public static synchronized void setEnabled(Context context, boolean value) {
        long generation = consentGeneration.incrementAndGet();
        enabled = false;
        Context app = context.getApplicationContext();
        if (!value) {
            sessionCrashRecord = null;
            crashRecordHeader = "";
            disarmNativeCrashHandler();
            setSystemSummary(app, null);
            try {
                DiagnosticConsent.locked(app.getFilesDir(), consent -> {
                    consent.observe(false, System.currentTimeMillis());
                    call(() -> NativeClient.telemetryClear(directoryRequest(app)));
                });
            } catch (Exception error) {
                // 同意文件损坏也必须履行清空承诺；该损坏同时阻止历史补录。
                call(() -> NativeClient.telemetryClear(directoryRequest(app)));
                Log.i(TAG, "Diagnostic consent unavailable");
            }
        } else {
            WORKER.execute(() -> { prepareCapture(app, !inputProcess, generation); flush(app); });
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

    /** 进程即将被系统结束：经共享层同步落盘并原子发布，再交给之前的异常处理器。 */
    private static void writeCrashRecord(Throwable error) throws Exception {
        File target;
        String header;
        synchronized (Telemetry.class) {
            target = sessionCrashRecord;
            header = crashRecordHeader;
        }
        if (target != null && !isSafeSessionCrashRecord(target)) return;
        if (target == null) {
            File directory = crashDirectory;
            if (directory == null) return;
            if (!prepareCrashDirectory(directory)) return;
            target = new File(directory, UUID.randomUUID() + CRASH_EXTENSION);
        }
        String record = crashRecord(error);
        int newline = record.indexOf('\n');
        String name = target.getName();
        String id = UUID.fromString(name.substring(0, name.length() - CRASH_EXTENSION.length())).toString();
        JSONObject request = new JSONObject()
            .put("directory", target.getParentFile().getParentFile().getAbsolutePath())
            .put("session", id)
            .put("message", record.substring(0, newline))
            .put("stack", record.substring(newline + 1));
        if (!header.isEmpty()) request.put("crash_record_header", header);
        // 带预先分配的 id，不按当前会话标记认领；该分支不取共享队列锁，也不联网。
        NativeClient.telemetryRecordCrash(request.toString());
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
            if (!root.equals(path.getParent())
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

    /** 共享层读取的摘要只含异常类型，不上传可能含输入或凭据的异常消息；栈只含类名与源文件名。 */
    static String crashRecord(Throwable error) {
        String summary = TextPolicy.clipCodePoints(error.getClass().getName(), MAX_MESSAGE_CODE_POINTS);
        StringBuilder stack = new StringBuilder(MAX_STACK_CODE_POINTS);
        Throwable current = error;
        for (int depth = 0; current != null && depth <= MAX_CAUSES; depth++) {
            if (depth > 0) stack.append("Caused by: ").append(current.getClass().getName()).append('\n');
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
        if (value != null && !JsonPolicy.strictBoolean(value.opt("enabled"), enabled)) {
            enabled = false;
            disarmNativeCrashHandler();
            setSystemSummary(app, null);
            try { DiagnosticConsent.locked(app.getFilesDir(), consent -> {
                if (!sharedEnabled(app)) consent.observe(false, System.currentTimeMillis());
            }); }
            catch (Exception error) { Log.i(TAG, "Diagnostic consent unavailable"); }
        }
    }

    /** 重新读取共享偏好，不能用另一个进程尚未刷新的 enabled 认领历史记录。 */
    private static boolean sharedEnabled(Context app) throws Exception {
        String preferences = HostOptionsPolicy.readOption(app.getFilesDir(), "preferences_directory");
        if (preferences.isEmpty()) {
            if (Files.exists(new File(app.getFilesDir(), "runtime-options.json").toPath(), LinkOption.NOFOLLOW_LINKS))
                throw new IllegalStateException("diagnostic options unavailable");
            return true;
        }
        JSONObject value = call(() -> NativeClient.loadPreferences(preferences));
        if (value == null || value.optJSONObject("preferences") == null) throw new IllegalStateException("diagnostic consent");
        JSONObject settings = value.getJSONObject("preferences");
        return !settings.has("usage_reporting") || JsonPolicy.strictTrue(settings.opt("usage_reporting"));
    }

    private static void prepareCapture(Context app, boolean captureOnly, long generation) {
        synchronized (Telemetry.class) {
            if (generation != consentGeneration.get()) return;
            try {
                DiagnosticConsent.locked(app.getFilesDir(), consent -> {
                    JSONObject value = call(() -> NativeClient.telemetryBegin(
                        new JSONObject(request(app)).put("capture_only", captureOnly)
                            .put("component", captureOnly ? "application" : "keyboard").toString()));
                    if (value == null) throw new IllegalStateException("diagnostic begin");
                    enabled = JsonPolicy.strictBoolean(value.opt("enabled"), false) && sharedEnabled(app);
                    DiagnosticConsent.State state = consent.observe(enabled, System.currentTimeMillis());
                    String path = value.optString("crash_record_path", "");
                    File candidate = path.isEmpty() ? null : new File(path);
                    sessionCrashRecord = enabled && isSafeSessionCrashRecord(candidate) ? candidate : null;
                    crashRecordHeader = value.optString("crash_record_header", "");
                    sessionId = captureOnly ? "" : value.optString("session_id", "");
                    sessionBegun = !captureOnly && !sessionId.isEmpty();
                    setSystemSummary(app, state == null ? null : state.summary(value.optString("session_id", ""),
                        version(app), captureOnly ? "application" : "keyboard"));
                    try {
                        if (sessionCrashRecord != null && !crashRecordHeader.isEmpty())
                            NativeClient.telemetryCrashCapture(path, crashRecordHeader);
                        else NativeClient.telemetryCrashDisarm();
                    } catch (LinkageError error) { Log.i(TAG, "Native crash recorder unavailable"); }
                });
            } catch (Exception error) {
                enabled = false;
                sessionCrashRecord = null;
                crashRecordHeader = "";
                disarmNativeCrashHandler();
                setSystemSummary(app, null);
                Log.i(TAG, "Diagnostic consent unavailable");
            }
        }
    }

    private static void setSystemSummary(Context app, String summary) {
        if (android.os.Build.VERSION.SDK_INT < 30) return;
        try { SystemExits.summary(app, summary); }
        catch (RuntimeException error) { Log.i(TAG, "Exit summary unavailable"); }
    }

    private static void collectSystemExits(Context app) {
        if (!enabled || android.os.Build.VERSION.SDK_INT < 30) return;
        try { SystemExits.collect(app); }
        catch (Exception error) { Log.i(TAG, "Exit diagnostics unavailable"); }
    }

    /** API 30 起系统提供退出原因；只接收本进程曾明确启用采集时留下的短身份，不读描述、日志或 tombstone。 */
    private static final class SystemExits {
        static void summary(Context app, String summary) {
            if (android.os.Build.VERSION.SDK_INT < 30) return;
            android.app.ActivityManager manager = (android.app.ActivityManager) app.getSystemService(Context.ACTIVITY_SERVICE);
            if (manager == null) return;
            byte[] bytes = summary == null ? null : summary.getBytes(java.nio.charset.StandardCharsets.US_ASCII);
            if (bytes == null || bytes.length <= 128) manager.setProcessStateSummary(bytes);
        }

        static void collect(Context app) throws Exception {
            if (android.os.Build.VERSION.SDK_INT < 30) return;
            android.app.ActivityManager manager = (android.app.ActivityManager) app.getSystemService(Context.ACTIVITY_SERVICE);
            if (manager == null) return;
            DiagnosticConsent.locked(app.getFilesDir(), consent -> {
                if (!enabled || !sharedEnabled(app)) return;
                DiagnosticConsent.State state = consent.current();
                if (state == null) return;
                for (android.app.ApplicationExitInfo exit : manager.getHistoricalProcessExitReasons(app.getPackageName(), 0, 16)) {
                    if (!enabled) return;
                    DiagnosticConsent.Identity original = state.accept(exit.getProcessStateSummary(),
                        exit.getTimestamp(), System.currentTimeMillis());
                    if (original == null) continue;
                    String captureId = original.capture;
                    int reason = exit.getReason();
                    boolean fatal = reason == android.app.ApplicationExitInfo.REASON_CRASH
                        || reason == android.app.ApplicationExitInfo.REASON_CRASH_NATIVE;
                    String message;
                    if (fatal) message = "Crash Android system reason " + reason + " signal " + exit.getStatus();
                    else if (reason == android.app.ApplicationExitInfo.REASON_ANR) message = "Hang Android ANR";
                    else if (reason == android.app.ApplicationExitInfo.REASON_LOW_MEMORY) message = "Killed Android low memory";
                    else continue;
                    // 真崩溃与现场记录共用 id，保留已有的精确栈；卡死、内存回收另分组，不污染输入会话崩溃率。
                    String id = fatal ? captureId : UUID.nameUUIDFromBytes(("msime-exit:" + captureId + ":" + exit.getTimestamp() + ":" + reason)
                        .getBytes(java.nio.charset.StandardCharsets.US_ASCII)).toString();
                    JSONObject identity = new JSONObject().put("platform", "android").put("version", original.version)
                        .put("revision", "unknown").put("run", "").put("attempt", "").put("arch", "unknown")
                        .put("component", original.component);
                    String header = "MSIME-CRASH/1 " + identity + "\n";
                    call(() -> NativeClient.telemetryRecordCrash(new JSONObject(directoryRequest(app))
                        .put("session", id).put("message", message).put("stack", "")
                        .put("crash_record_header", header).toString()));
                }
            });
        }
    }

    private static void installNativeCrashHandler() {
        try { NativeClient.telemetryCrashInstall(); }
        catch (LinkageError error) { Log.i(TAG, "Native crash recorder unavailable"); }
    }

    private static void disarmNativeCrashHandler() {
        try { NativeClient.telemetryCrashDisarm(); }
        catch (LinkageError error) { Log.i(TAG, "Native crash recorder unavailable"); }
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

package app.msime.android.core;

import app.msime.android.AppVersionPolicy;
import app.msime.android.AppEdition;
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
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
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
    private static final long CONSENT_POLL_MILLIS = 500;
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor(
        ThreadPolicy.namedDaemonFactory("msime-telemetry"));
    private static final ScheduledExecutorService OBSERVER = Executors.newSingleThreadScheduledExecutor(
        ThreadPolicy.namedDaemonFactory("msime-telemetry-consent"));
    private static ScheduledFuture<?> consentWatch;
    private static long watchGeneration;
    private static boolean captureActive;
    private static String captureEpoch = "";
    private static String capturePreferences = "";
    private static String captureSummary;
    private static Boolean observedConsent;
    private static volatile boolean optionsObserved;
    private static volatile Capture armedCapture;
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

    /** 应用进程独立采集，不创建输入会话；重复打开窗口仍沿用当前捕获身份。 */
    public static void start(Context context) {
        activate(context.getApplicationContext(), false);
    }

    /** 键盘进程开始一次生命周期，同时观察隐藏期间发生的同意变更。 */
    public static void beginInputSession(Context context) {
        activate(context.getApplicationContext(), true);
    }

    private static synchronized void activate(Context app, boolean input) {
        if (captureActive) {
            // 设置窗口再次打开仍刷新队列，已准备的捕获和输入会话保持不变。
            if (!input) WORKER.execute(() -> flush(app));
            return;
        }
        inputProcess = input;
        captureActive = true;
        crashDirectory = new File(directory(app), CRASH_DIRECTORY);
        installCrashHandler(app);
        installNativeCrashHandler();
        lastInputFlush = System.currentTimeMillis();
        long generation = consentGeneration.incrementAndGet();
        long watch = ++watchGeneration;
        consentWatch = OBSERVER.scheduleWithFixedDelay(() -> refreshConsent(app, watch),
            CONSENT_POLL_MILLIS, CONSENT_POLL_MILLIS, TimeUnit.MILLISECONDS);
        WORKER.execute(() -> { prepareCapture(app, !input, generation); collectSystemExits(app); flush(app); });
    }

    /** The keyboard was shown. Flushes at most every few hours; call on the main thread. */
    public static void inputViewShown(Context context) {
        long now = System.currentTimeMillis();
        if (now - lastInputFlush < INPUT_FLUSH_INTERVAL_MILLIS) return;
        lastInputFlush = now;
        Context app = context.getApplicationContext();
        WORKER.execute(() -> flush(app));
    }

    /** 正常结束只结束本次会话；先停止观察，排队的准备和过期回调不能在结束后重新武装。 */
    public static void endInputSession(Context context) {
        Context app = context.getApplicationContext();
        String endingSession;
        synchronized (Telemetry.class) {
            captureActive = false;
            watchGeneration++;
            if (consentWatch != null) consentWatch.cancel(false);
            consentWatch = null;
            consentGeneration.incrementAndGet();
            endingSession = sessionBegun ? sessionId : "";
            stopCapture(app, true);
            inputProcess = false;
        }
        if (!endingSession.isEmpty()) call(() -> NativeClient.telemetryEnd(
            new JSONObject(directoryRequest(app)).put("session", endingSession).toString()));
    }

    /** 偏好已保存；回调只触发重新核对，迟到的关闭通知不能清空重新授权后的队列。设置页在主线程上调用，而核对要取跨进程文件锁、经 JNI 读写偏好并落盘，所以交给串行的观察线程，与轮询按序执行；撤回前晚写的旧记录由共享代次拒收。 */
    public static void setEnabled(Context context, boolean value) {
        Context app = context.getApplicationContext();
        consentGeneration.incrementAndGet();
        OBSERVER.execute(() -> applyPreference(app, value));
    }

    private static synchronized void applyPreference(Context app, boolean value) {
        if (!captureActive && value) activate(app, false);
        if (captureActive) refreshConsent(app, watchGeneration);
        else {
            try {
                Consent current = sharedConsent(app);
                if (current.state == null) {
                    stopCapture(app, true);
                    DiagnosticConsent.locked(app.getFilesDir(), consent -> consent.observe(null));
                    clearDisabled(app, current.preferences);
                }
            } catch (Exception | LinkageError error) {
                stopCapture(app, false);
                Log.i(TAG, "Diagnostic consent unavailable");
            }
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
        // 不等待 Telemetry 的生命周期锁：持锁线程可能正是故障来源。
        Capture capture = armedCapture;
        if (!enabled || capture == null || !isSafeSessionCrashRecord(capture.target)) return;
        Context app = capture.app;
        DiagnosticConsent.tryLocked(app.getFilesDir(), consent -> {
            DiagnosticConsent.State state = consent.current();
            if (armedCapture != capture || state == null || !capture.epoch.equals(state.epoch)) return;
            String record = crashRecord(error);
            int newline = record.indexOf('\n');
            File target = capture.target;
            String name = target.getName();
            String id = UUID.fromString(name.substring(0, name.length() - CRASH_EXTENSION.length())).toString();
            // 只核对预备镜像，不等偏好或队列锁；旧现场晚写由记录头中的共享代次拒收。
            NativeClient.telemetryRecordCrash(new JSONObject()
                .put("directory", target.getParentFile().getParentFile().getAbsolutePath())
                .put("session", id).put("message", record.substring(0, newline))
                .put("stack", record.substring(newline + 1)).put("crash_record_header", capture.header).toString());
        });
    }

    /** 一次原子发布的捕获快照，故障线程不读取正在更新的一组生命周期字段。 */
    private static final class Capture {
        final Context app;
        final File target;
        final String header;
        final String epoch;
        final String preferences;

        Capture(Context app) {
            this.app = app;
            target = sessionCrashRecord;
            header = crashRecordHeader;
            epoch = captureEpoch;
            preferences = capturePreferences;
        }
    }

    /** 共享层分配的记录必须直接位于崩溃目录内；写入前再次拒绝被替换的符号链接。 */
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
        Capture capture = armedCapture;
        if (capture != null) call(() -> NativeClient.telemetryFlush(request(app, capture.preferences, capture.epoch)));
        long watch;
        synchronized (Telemetry.class) { watch = watchGeneration; }
        // 网络返回的 enabled 可能已经过期，重新读取此刻的共享同意再决定是否停用。
        refreshConsent(app, watch);
    }

    private static String reportingPreferences(Context app) throws Exception {
        boolean exists = Files.exists(new File(app.getFilesDir(), "runtime-options.json").toPath(), LinkOption.NOFOLLOW_LINKS);
        if (exists) optionsObserved = true;
        String preferences = HostOptionsPolicy.readOption(app.getFilesDir(), "preferences_directory");
        // 首次准备也使用 Bootstrap 的实际状态目录；配置曾存在后消失则停采。
        if (preferences.isEmpty()) {
            if (exists || optionsObserved) throw new IllegalStateException("diagnostic options unavailable");
            preferences = HostOptionsPolicy.bootstrapStateDirectory(app.getFilesDir());
        }
        if (!new File(preferences).isAbsolute()) throw new IllegalStateException("diagnostic preferences path");
        return preferences;
    }

    private static final class Consent {
        final String preferences;
        final DiagnosticConsent.State state;
        Consent(String preferences, DiagnosticConsent.State state) { this.preferences = preferences; this.state = state; }
        boolean same(Consent other) {
            return other.state != null && state != null && preferences.equals(other.preferences)
                && state.epoch.equals(other.state.epoch) && state.since == other.state.since;
        }
    }

    /** 不可读保持错误；共享层在真实偏好锁下确保代次，不能由布尔缓存绕过。 */
    private static Consent sharedConsent(Context app) throws Exception {
        String preferences = reportingPreferences(app);
        JSONObject root = new JSONObject(NativeClient.telemetryConsent(new JSONObject().put("directory", preferences).put("edition", AppEdition.current().id()).toString()));
        JSONObject value = JsonPolicy.objectValue(root);
        Boolean enabled = JsonPolicy.strictBoolean(value == null ? null : value.opt("enabled"));
        if (enabled == null) throw new IllegalStateException("diagnostic consent");
        if (!enabled) return new Consent(preferences, null);
        Object since = value.opt("since_unix_ms");
        if (!(since instanceof Long) && !(since instanceof Integer)) throw new IllegalStateException("diagnostic consent time");
        return new Consent(preferences, new DiagnosticConsent.State(value.getString("epoch"), ((Number) since).longValue()));
    }

    private static void clearDisabled(Context app, String preferences) {
        call(() -> NativeClient.telemetryClear(new JSONObject(directoryRequest(app))
            .put("preferences_directory", preferences).toString()));
    }

    /** 停采与遗忘分开：暂时读不到同意只解除武装，明确关闭才丢弃旧身份。 */
    private static void stopCapture(Context app, boolean forget) {
        enabled = false;
        armedCapture = null;
        disarmNativeCrashHandler();
        setSystemSummary(app, null);
        if (forget) {
            sessionCrashRecord = null;
            crashRecordHeader = "";
            captureSummary = null;
            captureEpoch = "";
            capturePreferences = "";
            sessionId = "";
            sessionBegun = false;
        }
    }

    private static void armCapture(Context app) {
        try { NativeClient.telemetryCrashCapture(sessionCrashRecord.getAbsolutePath(), crashRecordHeader); }
        catch (LinkageError error) { Log.i(TAG, "Native crash recorder unavailable"); }
        setSystemSummary(app, captureSummary);
        armedCapture = new Capture(app);
        enabled = true;
    }

    /** 独立于网络工作线程，每轮只读同意；身份未变时不 begin、不写队列。 */
    private static synchronized void refreshConsent(Context app, long watch) {
        if (!captureActive || watch != watchGeneration) return;
        boolean[] prepare = {false};
        try {
            DiagnosticConsent.locked(app.getFilesDir(), consent -> {
                Consent current = sharedConsent(app);
                if (current.state == null) {
                    boolean changed = !Boolean.FALSE.equals(observedConsent) || enabled || !captureEpoch.isEmpty();
                    if (changed) consentGeneration.incrementAndGet();
                    if (changed) stopCapture(app, true);
                    observedConsent = false;
                    consent.observe(null);
                    if (changed) clearDisabled(app, current.preferences);
                    return;
                }
                DiagnosticConsent.State state = consent.observe(current.state);
                if (state.epoch.equals(captureEpoch) && current.preferences.equals(capturePreferences)
                        && sessionCrashRecord != null && !crashRecordHeader.isEmpty()
                        && isSafeSessionCrashRecord(sessionCrashRecord)) {
                    if (!enabled) armCapture(app);
                    observedConsent = true;
                } else prepare[0] = true;
            });
            if (prepare[0]) {
                long generation = consentGeneration.incrementAndGet();
                prepareCapture(app, !inputProcess, generation);
            }
        } catch (Exception | LinkageError error) {
            if (enabled || observedConsent != null) consentGeneration.incrementAndGet();
            stopCapture(app, false);
            observedConsent = null;
        } catch (Error error) {
            stopCapture(app, false);
            captureActive = false;
            throw error;
        }
    }

    private static void prepareCapture(Context app, boolean captureOnly, long generation) {
        synchronized (Telemetry.class) {
            if (!captureActive || generation != consentGeneration.get()) return;
            try {
                DiagnosticConsent.locked(app.getFilesDir(), consent -> {
                    // 新 epoch 或偏好目录需要新会话；旧会话若已被撤回清理，end 只会返回未找到。
                    if (sessionBegun && !sessionId.isEmpty()) {
                        String previous = sessionId;
                        call(() -> NativeClient.telemetryEnd(new JSONObject(directoryRequest(app)).put("session", previous).toString()));
                    }
                    stopCapture(app, true);
                    Consent current = sharedConsent(app);
                    if (current.state == null) {
                        observedConsent = false;
                        consent.observe(null);
                        clearDisabled(app, current.preferences);
                        return;
                    }
                    JSONObject value = call(() -> NativeClient.telemetryBegin(
                        new JSONObject(request(app, current.preferences, current.state.epoch)).put("capture_only", captureOnly)
                            .put("component", captureOnly ? "application" : "keyboard").toString()));
                    if (value == null) throw new IllegalStateException("diagnostic begin");
                    sessionId = captureOnly ? "" : value.optString("session_id", "");
                    sessionBegun = !captureOnly && !sessionId.isEmpty();
                    if (!Boolean.TRUE.equals(JsonPolicy.strictBoolean(value.opt("enabled"))) || !current.same(sharedConsent(app)))
                        throw new IllegalStateException("diagnostic consent changed");
                    DiagnosticConsent.State state = consent.observe(current.state);
                    String path = value.optString("crash_record_path", "");
                    File candidate = path.isEmpty() ? null : new File(path);
                    if (!isSafeSessionCrashRecord(candidate)) throw new IllegalStateException("diagnostic identity");
                    sessionCrashRecord = candidate;
                    crashRecordHeader = value.optString("crash_record_header", "");
                    if (crashRecordHeader.isEmpty()) throw new IllegalStateException("diagnostic header");
                    captureEpoch = state.epoch;
                    capturePreferences = current.preferences;
                    captureSummary = state.summary(value.optString("session_id", ""), version(app),
                        captureOnly ? "application" : "keyboard");
                    observedConsent = true;
                    armCapture(app);
                });
            } catch (Exception | LinkageError error) {
                stopCapture(app, false);
                observedConsent = null;
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
                if (!enabled) return;
                Consent current = sharedConsent(app);
                DiagnosticConsent.State state = current.state;
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
                        .put("outside_session", true).put("diagnostic_capture", id)
                        .put("consent_epoch", state.epoch).put("preferences_directory", current.preferences)
                        .put("diagnostic_begin_unix_ms", exit.getTimestamp()).put("diagnostic_end_unix_ms", exit.getTimestamp())
                        .put("message", message).put("stack", "").put("crash_record_header", header).toString()));
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

    /** begin 和 flush 固定共享授权代次，由 Rust 在本地操作及每条发送前重新核对。 */
    private static String request(Context app, String preferences, String epoch) throws Exception {
        return new JSONObject(directoryRequest(app)).put("platform", "android").put("version", version(app))
            .put("preferences_directory", preferences).put("consent_epoch", epoch).toString();
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

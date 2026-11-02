package app.msime.android;

import android.graphics.Color;
import android.os.SystemClock;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.ByteBuffer;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 候选区上方那行提示与调试信息：提示的显示、自动消失与清除；`developer_options.debug_overlay` 打开时，没有提示的空档里显示引擎耗时（按键到候选更新）与候选数；`diagnostic_log.mobile` 打开且隐私判断允许时，把输入事件与耗时写进 `<filesDir>/diagnostics/` 下的两个 jsonl。
 *
 * <p>写入的每一行只能由 {@link DiagnosticEventKind} 枚举、时间戳和耗时构造（P19），写入方法不接受任何字符串参数，所以输入内容、拼音、候选和按键字符无从进入文件。
 */
final class ImeDebugOverlay {
    /**
     * 两个本地日志文件：`input-events.jsonl`（`{t_ms,kind}`）与 `perf.jsonl`（`{t_ms,kind,duration_ms}`），各自不超过 1 MiB；写满时丢掉最旧的行，只留到上限的四分之三再续写。
     *
     * <p>写在单独的后台线程上，写不进去时静默放弃：这是开发者自己打开的诊断记录，不能影响打字。
     */
    static final class EventLog {
        static final int MAX_BYTES = 1024 * 1024;
        static final String EVENTS_FILE = "input-events.jsonl";
        static final String PERF_FILE = "perf.jsonl";

        private final File directory;
        private final ExecutorService worker = Executors.newSingleThreadExecutor(
            ThreadPolicy.namedDaemonFactory("msime-input-events"));

        EventLog(File directory) {
            this.directory = directory;
        }

        /** 一行输入事件：只有时间戳与种类。 */
        static String line(DiagnosticEventKind kind, long timeMillis) {
            if (kind == null) throw new IllegalArgumentException("event kind");
            return "{\"t_ms\":" + BoundsPolicy.nonNegative(timeMillis) + ",\"kind\":\"" + kind.wire() + "\"}\n";
        }

        /** 一行耗时记录：时间戳、种类与耗时（毫秒）。 */
        static String line(DiagnosticEventKind kind, long timeMillis, long durationMillis) {
            if (kind == null) throw new IllegalArgumentException("event kind");
            return "{\"t_ms\":" + BoundsPolicy.nonNegative(timeMillis) + ",\"kind\":\"" + kind.wire()
                + "\",\"duration_ms\":" + BoundsPolicy.nonNegative(durationMillis) + "}\n";
        }

        /**
         * 环形上限：已有内容加新行超过 {@code cap} 时，从最旧的行开始丢，直到剩下的不超过上限的四分之三（且能放下新行）。
         *
         * @return 应保留的已有内容（可能就是原数组）
         */
        static byte[] trimmed(byte[] existing, int incoming, long cap) {
            if (existing.length + (long) incoming <= cap) return existing;
            long keep = BoundsPolicy.bounded(cap * 3 / 4, 0L, cap - incoming);
            int start = (int) BoundsPolicy.nonNegative(existing.length - keep);
            while (start < existing.length && start > 0 && existing[start - 1] != '\n') start++;
            byte[] kept = new byte[existing.length - start];
            System.arraycopy(existing, start, kept, 0, kept.length);
            return kept;
        }

        void event(DiagnosticEventKind kind) {
            submit(EVENTS_FILE, line(kind, System.currentTimeMillis()));
        }

        void perf(DiagnosticEventKind kind, long durationMillis) {
            submit(PERF_FILE, line(kind, System.currentTimeMillis(), durationMillis));
        }

        private void submit(String name, String line) {
            try {
                worker.execute(() -> append(name, TextPolicy.utf8Bytes(line)));
            } catch (RejectedExecutionException ignored) {
                // 进程正在退出，这一行不再需要。
            }
        }

        /** 不再接新行；已排队的行照常写完（线程是 daemon，不会拖住进程退出）。 */
        void shutdown() {
            worker.shutdown();
        }

        private void append(String name, byte[] line) {
            try {
                SafePaths.ensureDirectory(directory.toPath());
                java.nio.file.Path file = new File(directory, name).toPath();
                if (Files.isSymbolicLink(file)) Files.delete(file);
                if (appendInPlace(file, line)) return;
                byte[] existing = Files.exists(file, LinkOption.NOFOLLOW_LINKS)
                    ? read(file) : new byte[0];
                byte[] kept = existing.length + (long) line.length <= MAX_BYTES
                    ? existing : trimmed(existing, line.length, MAX_BYTES);
                byte[] next = new byte[kept.length + line.length];
                System.arraycopy(kept, 0, next, 0, kept.length);
                System.arraycopy(line, 0, next, kept.length, line.length);
                // 认不准是单链接普通文件或要截掉旧行时，必须先写新文件再替换；直接打开日志目标会跟随硬链接并改写目录外 inode。
                java.nio.file.Path staging = Files.createTempFile(directory.toPath(),
                    "." + name + ".", ".staging");
                try {
                    Files.write(staging, next, StandardOpenOption.TRUNCATE_EXISTING,
                        StandardOpenOption.WRITE, LinkOption.NOFOLLOW_LINKS);
                    Files.move(staging, file, StandardCopyOption.REPLACE_EXISTING,
                        StandardCopyOption.ATOMIC_MOVE);
                } finally {
                    Files.deleteIfExists(staging);
                }
            } catch (IOException | RuntimeException ignored) {
                // 诊断记录写不进去时放弃这一行，不影响键盘。
            }
        }

        /**
         * 平常的一行只追加到文件末尾。每行都读一遍整个文件再整份重写，日志接近 1 MiB 时每次按键要读写约 2 MiB，打开「记录输入日志」后一直在写闪存、发热，也扭曲了要量的卡顿。
         *
         * <p>只有文件已存在、是普通文件、只有这一个链接、追加后不超上限时才走这里，否则返回 false，交给先写新文件再替换的路径：它负责丢掉最旧的行，也负责不跟随硬链接去改诊断目录之外的 inode。
         */
        private static boolean appendInPlace(java.nio.file.Path file, byte[] line) throws IOException {
            Object identity = soleLink(file);
            if (identity == null) return false;
            try (FileChannel channel = FileChannel.open(file, StandardOpenOption.WRITE,
                    StandardOpenOption.APPEND, LinkOption.NOFOLLOW_LINKS)) {
                // 打开后再认一次：打开的必须还是刚才那个只有一个链接的 inode。
                if (!identity.equals(soleLink(file))) return false;
                long start = channel.size();
                if (start + line.length > MAX_BYTES) return false;
                try {
                    ByteBuffer buffer = ByteBuffer.wrap(line);
                    while (buffer.hasRemaining()) channel.write(buffer);
                } catch (IOException failure) {
                    // 写了半行（比如盘满）就退回原长度，不留残行。
                    channel.truncate(start);
                    throw failure;
                }
                return true;
            }
        }

        /** 普通文件且只有一个链接时返回它的 fileKey（设备号与 inode）；不存在、不是普通文件、另有硬链接或平台给不出链接数时返回 null。 */
        private static Object soleLink(java.nio.file.Path file) {
            try {
                Map<String, Object> attributes = Files.readAttributes(file, "unix:nlink,fileKey,isRegularFile",
                    LinkOption.NOFOLLOW_LINKS);
                Object links = attributes.get("nlink");
                Object key = attributes.get("fileKey");
                return Boolean.TRUE.equals(attributes.get("isRegularFile")) && links instanceof Integer count
                    && count == 1 && key != null ? key : null;
            } catch (IOException | UnsupportedOperationException | IllegalArgumentException unavailable) {
                return null;
            }
        }

        private static byte[] read(java.nio.file.Path file) throws IOException {
            try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS)) {
                byte[] bytes = HttpBodyPolicy.readBounded(input, Math.toIntExact(2 * MAX_BYTES));
                return bytes == null ? new byte[0] : bytes;
            }
        }
    }

    private final MSIMEInputService s;
    private EventLog eventLog;
    private AndroidLocalSettings.Snapshot localSeen;
    private boolean overlayEnabled;
    private boolean mobileLogEnabled;
    /** 最近一次按键的时刻（uptime 毫秒），下一次引擎结果到达时据此算按键到候选更新的耗时；没有待测的按键时为 -1。 */
    private long pendingKeyAt = -1;
    /** 引擎结果到达后、render() 还没改完视图时，那次按键的时刻（uptime 毫秒）；{@link #onRendered} 据此记 candidate_rendered。没有待记的按键时为 -1。 */
    private long renderPendingAt = -1;
    private long lastEngineMillis = -1;
    /** `onCreate` 量到的服务创建耗时（毫秒），还没写进日志时为 -1。那时还没有输入框，`allowLearning` 是默认的 false，隐私判断按「不做个性化学习的输入框」挡掉一切记录，只能等进了允许记录的输入框再补写（#9916）。 */
    private long pendingServiceCreateMillis = -1;

    ImeDebugOverlay(MSIMEInputService s) {
        this.s = s;
    }

    /** 按当前偏好刷新调试开关与日志级别；偏好文档没变时什么都不做。 */
    void refreshPreferences() {
        AndroidLocalSettings.Snapshot settings = s.localSettings;
        if (settings == localSeen) return;
        localSeen = settings;
        boolean loggingEnabled = settings.bool(AndroidLocalSettings.DEVELOPER_LOGGING_ENABLED);
        overlayEnabled = loggingEnabled && settings.bool(AndroidLocalSettings.DEVELOPER_DEBUG_OVERLAY);
        ImeLog.applyLevel(settings.choice(AndroidLocalSettings.DEVELOPER_LOG_LEVEL));
        mobileLogEnabled = loggingEnabled && settings.bool(AndroidLocalSettings.DEVELOPER_INPUT_LOG);
    }

    /** 开发者选项里的「显示调试信息」开着（日志总开关也开着）：双拼键位几何的调试导出（{@link ImeTouchPoints}）跟着它。 */
    boolean overlay() {
        refreshPreferences();
        return overlayEnabled;
    }

    /** 本地输入日志是否在写：用户打开了「记录输入日志」，而且当前输入框与隐私模式允许。 */
    boolean logging() {
        refreshPreferences();
        return mobileLogEnabled && s.imePrivacyGate != null && s.imePrivacyGate.recordsInputEvents()
            && s.imePrivacyGate.writesDiagnosticLog();
    }

    /** 服务销毁时调用：停掉输入日志的写线程。 */
    void shutdown() {
        if (eventLog != null) eventLog.shutdown();
    }

    private EventLog log() {
        if (eventLog == null) {
            File files = s.getFilesDir();
            if (files == null) return null;
            eventLog = new EventLog(new File(files, "diagnostics"));
        }
        return eventLog;
    }

    /** 记一个输入事件（只记种类与时间）。 */
    void recordEvent(DiagnosticEventKind kind) {
        if (!logging()) return;
        EventLog target = log();
        if (target != null) target.event(kind);
    }

    /** 记一条耗时（只记种类、时间与毫秒数）。 */
    void recordPerf(DiagnosticEventKind kind, long durationMillis) {
        if (!logging()) return;
        EventLog target = log();
        if (target != null) target.perf(kind, durationMillis);
    }

    /** 记下 `onCreate` 的耗时，等 {@link #flushServiceCreate} 在允许记录的输入框里写出；同一个服务只写一次。 */
    void holdServiceCreate(long durationMillis) {
        pendingServiceCreateMillis = durationMillis;
    }

    /** 进了输入框、隐私判断有了依据之后调用：有待写的服务创建耗时且此刻允许记录时写出。不允许时继续留着，等下一个允许记录的输入框。 */
    void flushServiceCreate() {
        if (pendingServiceCreateMillis < 0 || !logging()) return;
        long durationMillis = pendingServiceCreateMillis;
        pendingServiceCreateMillis = -1;
        recordPerf(DiagnosticEventKind.IME_SERVICE_CREATE, durationMillis);
    }

    /** 一次按键按下：开始计时，并记 key_down（删除键记 backspace）。 */
    void onKeyPressed(boolean backspace) {
        pendingKeyAt = SystemClock.uptimeMillis();
        renderPendingAt = -1;
        recordEvent(backspace ? DiagnosticEventKind.BACKSPACE : DiagnosticEventKind.KEY_DOWN);
    }

    /** 同步调引擎之前取的时刻；不在量一次按键时为 -1，{@link #onEngineCall} 见到 -1 什么也不记。 */
    long engineCallStart() {
        return pendingKeyAt >= 0 ? SystemClock.uptimeMillis() : -1;
    }

    /** 一次按键的引擎调用返回了：记 engine_call。只记引擎结果到达（{@link #showDiagnostic}）之前的那一次调用，所以和 candidate_shown 一一对应。 */
    void onEngineCall(long startedAt) {
        if (startedAt < 0 || pendingKeyAt < 0) return;
        recordPerf(DiagnosticEventKind.ENGINE_CALL, SystemClock.uptimeMillis() - startedAt);
    }

    /** render() 改完了视图：有待记的按键时记 candidate_rendered，即按键到这一刻的耗时。之后这一帧的测量、布局和绘制不在里面。 */
    void onRendered() {
        if (renderPendingAt < 0) return;
        long startedAt = renderPendingAt;
        renderPendingAt = -1;
        recordPerf(DiagnosticEventKind.CANDIDATE_RENDERED, SystemClock.uptimeMillis() - startedAt);
    }

    void showDiagnostic(String value) {
        if (pendingKeyAt >= 0) {
            lastEngineMillis = SystemClock.uptimeMillis() - pendingKeyAt;
            renderPendingAt = pendingKeyAt;
            pendingKeyAt = -1;
            recordPerf(DiagnosticEventKind.CANDIDATE_SHOWN, lastEngineMillis);
        }
        showMessage(value);
    }

    /** 在诊断行显示一句话，{@link InputDiagnosticPolicy#DISMISS_DELAY_MILLIS} 后消失；空串清掉当前那句。和引擎诊断共用一行，但不计入按键耗时。 */
    void showMessage(String value) {
        s.diagnosticGeneration++;
        if (s.diagnosticDismissTask != null) {
            s.main.removeCallbacks(s.diagnosticDismissTask);
            s.diagnosticDismissTask = null;
        }
        s.diagnosticMessage = InputDiagnosticPolicy.normalize(value);
        if (s.diagnosticMessage.isEmpty()) return;
        long generation = s.diagnosticGeneration;
        s.diagnosticDismissTask = () -> {
            if (generation != s.diagnosticGeneration) return;
            s.diagnosticMessage = "";
            s.diagnosticDismissTask = null;
            s.render();
        };
        s.main.postDelayed(s.diagnosticDismissTask, InputDiagnosticPolicy.DISMISS_DELAY_MILLIS);
    }

    void clearDiagnostic() {
        s.diagnosticGeneration++;
        if (s.diagnosticDismissTask != null) {
            s.main.removeCallbacks(s.diagnosticDismissTask);
            s.diagnosticDismissTask = null;
        }
        s.diagnosticMessage = "";
        pendingKeyAt = -1;
        renderPendingAt = -1;
    }

    /** 调试行的文字：引擎耗时与候选数；首选候选带权重或来源字段时一并显示。 */
    static String debugLine(long engineMillis, int candidateCount, String firstWeight) {
        StringBuilder line = new StringBuilder("调试 · 引擎 ");
        line.append(engineMillis < 0 ? "—" : engineMillis + " ms");
        line.append(" · 候选 ").append(BoundsPolicy.nonNegative(candidateCount));
        if (firstWeight != null && !firstWeight.isEmpty()) line.append(" · 首选词频 ").append(firstWeight);
        return line.toString();
    }

    private String debugText() {
        JSONArray candidates = s.view == null ? null : s.view.optJSONArray("candidates");
        int count = candidates == null ? 0 : candidates.length();
        String weight = "";
        JSONObject first = candidates == null || count == 0 ? null : candidates.optJSONObject(0);
        if (first != null) {
            for (String key : new String[] {"frequency", "weight", "score"}) {
                Object value = first.opt(key);
                if (value instanceof Number number) {
                    weight = String.valueOf(number.longValue());
                    break;
                }
            }
        }
        return debugLine(lastEngineMillis, count, weight);
    }

    void updateDiagnosticView(boolean hasDiagnostic) {
        if (s.diagnosticView == null) return;
        refreshPreferences();
        boolean debug = !hasDiagnostic && overlayEnabled;
        String text = debug ? debugText() : s.diagnosticMessage;
        s.diagnosticView.setText(text);
        s.diagnosticView.setContentDescription("提示：" + text);
        ViewPolicy.setTextColor(s.diagnosticView, hasDiagnostic ? Color.WHITE
            : Color.parseColor(s.skin.accent()));
        ViewPolicy.setBackground(s.diagnosticView, hasDiagnostic
            ? DrawablePolicy.pill(Color.BLACK) : null);
        s.diagnosticView.setPadding(s.pixels(hasDiagnostic ? 10 : 0),
            s.pixels(hasDiagnostic ? 3 : 0), s.pixels(hasDiagnostic ? 10 : 0),
            s.pixels(hasDiagnostic ? 3 : 0));
        if (hasDiagnostic || debug) ViewPolicy.show(s.diagnosticView);
        else ViewPolicy.hide(s.diagnosticView);
    }
}

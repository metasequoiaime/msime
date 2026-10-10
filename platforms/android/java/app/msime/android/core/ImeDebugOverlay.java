package app.msime.android;

import android.graphics.Color;
import android.os.SystemClock;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.RejectedExecutionException;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 候选区上方那行提示与调试信息：提示的显示、自动消失与清除；`developer_options.debug_overlay` 打开时，没有提示的空档里显示引擎耗时（按键到候选更新）与候选数；`diagnostic_log.mobile` 打开且隐私判断允许时，把输入事件与耗时写进 `<filesDir>/diagnostics/` 下的两个 jsonl。
 *
 * <p>写入的每一行只能由 {@link Event} 枚举、时间戳和耗时构造（P19），写入方法不接受任何字符串参数，所以输入内容、拼音、候选和按键字符无从进入文件。
 */
final class ImeDebugOverlay {
    /** P19 规定的事件种类；输入事件与耗时记录共用，与 client-core 诊断包的白名单一致。 */
    enum Event {
        KEY_DOWN("key_down"),
        KEY_UP("key_up"),
        CANDIDATE_SHOWN("candidate_shown"),
        CANDIDATE_SELECTED("candidate_selected"),
        COMMIT("commit"),
        BACKSPACE("backspace"),
        PANEL_OPEN("panel_open"),
        PANEL_CLOSE("panel_close"),
        IME_START("ime_start"),
        IME_FINISH("ime_finish");

        private final String wire;

        Event(String wire) { this.wire = wire; }

        String wire() { return wire; }
    }

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
        static String line(Event kind, long timeMillis) {
            if (kind == null) throw new IllegalArgumentException("event kind");
            return "{\"t_ms\":" + BoundsPolicy.nonNegative(timeMillis) + ",\"kind\":\"" + kind.wire() + "\"}\n";
        }

        /** 一行耗时记录：时间戳、种类与耗时（毫秒）。 */
        static String line(Event kind, long timeMillis, long durationMillis) {
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

        void event(Event kind) {
            submit(EVENTS_FILE, line(kind, System.currentTimeMillis()));
        }

        void perf(Event kind, long durationMillis) {
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
                byte[] existing = Files.exists(file, LinkOption.NOFOLLOW_LINKS)
                    ? read(file) : new byte[0];
                byte[] kept = existing.length + (long) line.length <= MAX_BYTES
                    ? existing : trimmed(existing, line.length, MAX_BYTES);
                byte[] next = new byte[kept.length + line.length];
                System.arraycopy(kept, 0, next, 0, kept.length);
                System.arraycopy(line, 0, next, kept.length, line.length);
                // 追加路径也必须先写新文件再替换；直接打开日志目标会跟随硬链接并改写目录外 inode。
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
    private long lastEngineMillis = -1;

    ImeDebugOverlay(MSIMEInputService s) {
        this.s = s;
    }

    /** 按当前偏好刷新调试开关与日志级别；偏好文档没变时什么都不做。 */
    void refreshPreferences() {
        AndroidLocalSettings.Snapshot settings = s.localSettings;
        if (settings == localSeen) return;
        localSeen = settings;
        overlayEnabled = settings.bool(AndroidLocalSettings.DEVELOPER_DEBUG_OVERLAY);
        ImeLog.applyLevel(settings.choice(AndroidLocalSettings.DEVELOPER_LOG_LEVEL));
        mobileLogEnabled = settings.bool(AndroidLocalSettings.DEVELOPER_INPUT_LOG);
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
    void recordEvent(Event kind) {
        if (!logging()) return;
        EventLog target = log();
        if (target != null) target.event(kind);
    }

    /** 记一条耗时（只记种类、时间与毫秒数）。 */
    void recordPerf(Event kind, long durationMillis) {
        if (!logging()) return;
        EventLog target = log();
        if (target != null) target.perf(kind, durationMillis);
    }

    /** 一次按键按下：开始计时，并记 key_down（删除键记 backspace）。 */
    void onKeyPressed(boolean backspace) {
        pendingKeyAt = SystemClock.uptimeMillis();
        recordEvent(backspace ? Event.BACKSPACE : Event.KEY_DOWN);
    }

    void showDiagnostic(String value) {
        if (pendingKeyAt >= 0) {
            lastEngineMillis = SystemClock.uptimeMillis() - pendingKeyAt;
            pendingKeyAt = -1;
            recordPerf(Event.CANDIDATE_SHOWN, lastEngineMillis);
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
        // 只在输入结束（stop）与服务销毁时调用，所以同时记一次 ime_finish。
        recordEvent(Event.IME_FINISH);
        pendingKeyAt = -1;
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
        ViewPolicy.setTextColor(s.diagnosticView, Color.parseColor(s.skin.accent()));
        if (hasDiagnostic || debug) ViewPolicy.show(s.diagnosticView);
        else ViewPolicy.hide(s.diagnosticView);
    }
}

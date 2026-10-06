package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「MCP 开发者访问」的云端接口：上传一份日志快照、读回快照与最近访问、重新生成访问令牌、删除快照（`/v1/users/me/diagnostics`）。
 *
 * <p>上传的内容只来自 Rust 写出的诊断包（`NativeClient.diagnosticBundle`）：配置快照是诊断包里已经脱敏的 `config_snapshot`，这里不自己脱敏；输入事件和性能数据从诊断包里按 P19 白名单校验过的 jsonl 读出，再在这里按 {@link EventKind} 枚举逐条重建，只留 `t_ms`、`kind`、`duration_ms`，不认识的种类整行丢弃。请求体里不会出现任何文本、拼音、候选或按键字面值。
 *
 * <p>匿名账号也可以上传（`ACCOUNT_OR_ANONYMOUS`）。每个请求都阻塞在网络上，不要在主线程调用。
 */
public final class DiagnosticsApi {
    static final String PATH = "/v1/users/me/diagnostics";
    /** 后端整份请求体的上限。 */
    static final int MAX_BODY_BYTES = 2 * 1024 * 1024;
    /** 崩溃摘要与堆栈的上限（UTF-8 字节），与后端校验一致。 */
    static final int MAX_MESSAGE_BYTES = 2 * 1024;
    static final int MAX_STACK_BYTES = 16 * 1024;
    /** 每类事件最多带多少条，超出时保留最新的。 */
    static final int MAX_EVENTS = 8000;
    static final int MAX_CRASH_LOGS = 50;

    /** 快照在云端保留多久，对应偏好 `developer_options.mcp_upload.retention` 和请求里的 `ttl`。 */
    public enum Retention {
        ONE_HOUR("one_hour", "1 小时"),
        ONE_DAY("one_day", "24 小时"),
        SEVEN_DAYS("seven_days", "7 天");

        private final String wire;
        private final String label;

        Retention(String wire, String label) {
            this.wire = wire;
            this.label = label;
        }

        public String wire() { return wire; }

        public String label() { return label; }

        /** 不认识的值按默认的 24 小时处理。 */
        public static Retention fromWire(String value) {
            for (Retention retention : values()) {
                if (retention.wire.equals(value)) return retention;
            }
            return ONE_DAY;
        }
    }

    /** P19 允许的事件种类；输入事件和性能记录的 `kind` 只能取这些值。 */
    public enum EventKind {
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

        EventKind(String wire) { this.wire = wire; }

        public String wire() { return wire; }

        /** 不在枚举里的种类返回 null，调用方丢弃这一条。 */
        public static EventKind fromWire(String value) {
            for (EventKind kind : values()) {
                if (kind.wire.equals(value)) return kind;
            }
            return null;
        }
    }

    /** 一条输入事件或性能记录；`durationMs` 小于 0 表示没有耗时。 */
    public record Event(long tMs, EventKind kind, long durationMs) {
        /** 按枚举重建一条记录；种类不认识、时间为负时返回 null。 */
        public static Event of(long tMs, String kind, long durationMs) {
            EventKind known = EventKind.fromWire(kind);
            if (known == null || tMs < 0) return null;
            return new Event(tMs, known, durationMs < 0 ? -1 : durationMs);
        }
    }

    /** 一条崩溃记录；摘要和堆栈按后端上限截断。 */
    public record CrashLog(String at, String message, String stack) {
        public static CrashLog of(String at, String message, String stack) {
            return new CrashLog(at == null ? "" : at, clipUtf8(message, MAX_MESSAGE_BYTES),
                clipUtf8(stack, MAX_STACK_BYTES));
        }
    }

    /** 上传哪几类日志，对应偏好 `developer_options.mcp_upload.*`。 */
    public record Include(boolean crashLogs, boolean performanceLogs, boolean inputEvents,
            boolean configSnapshot) {
        public boolean any() { return crashLogs || performanceLogs || inputEvents || configSnapshot; }
    }

    /** 请求体的 `sections`；没选的类别为 null，不出现在请求里。`configSnapshot` 是诊断包里脱敏后的 JSON 对象原文。 */
    public record Sections(List<CrashLog> crashLogs, List<Event> perfTrace, String configSnapshot,
            List<Event> inputEvents) {}

    /** 上传成功的回答；`token` 只在这里出现一次。 */
    public record Created(String id, String mcpUrl, String token, String expiresAt) {}

    /** 云端现有的快照。 */
    public record Snapshot(String id, String createdAt, String expiresAt, long bytes, List<String> sections,
            String tokenHint) {
        /** 远程地址；GET 不回 `mcp_url`，按 id 拼出与上传回答相同的地址。 */
        public String mcpUrl() { return DiagnosticsApi.mcpUrl(id); }
    }

    /** 开发者的一次读取。`arguments` 是调用参数的 JSON 原文（≤1 KiB）。 */
    public record Access(String at, String tool, String arguments, long resultCount, long bytes) {}

    /** `GET` 的回答；`snapshot` 为 null 表示云端没有快照。 */
    public record State(Snapshot snapshot, List<Access> accesses) {
        public static final State EMPTY = new State(null, Collections.emptyList());
    }

    private final CloudApi api;

    public DiagnosticsApi(CloudApi api) {
        this.api = api;
    }

    /** 上传一份快照，替换云端已有的那份。 */
    public Created upload(String platform, String appVersion, Sections sections, Retention ttl)
            throws CloudApi.Failure {
        byte[] body = requestBody(platform, appVersion, sections, ttl).getBytes(StandardCharsets.UTF_8);
        if (body.length > MAX_BODY_BYTES) {
            throw new CloudApi.Failure(413, "payload_too_large", "diagnostics snapshot exceeds 2 MiB", 0);
        }
        CloudApi.Response response = api.send("POST", PATH, new CloudApi.Body("application/json", body),
            CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        try {
            JSONObject root = response.json();
            String id = root.optString("id", "");
            String token = root.optString("token", "");
            if (id.isEmpty() || token.isEmpty()) {
                throw new CloudApi.Failure(response.status(), "invalid_response", "snapshot id or token missing", 0);
            }
            return new Created(id, root.optString("mcp_url", mcpUrl(id)), token, root.optString("expires_at", ""));
        } catch (JSONException malformed) {
            throw new CloudApi.Failure(response.status(), "invalid_response", "malformed JSON response", 0);
        }
    }

    /** 云端快照与最近访问。 */
    public State state() throws CloudApi.Failure {
        JSONObject root = api.json("GET", PATH, null, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        return parseState(root);
    }

    /** 换一枚访问令牌，旧令牌立即作废；返回新令牌（只出现这一次）。 */
    public String regenerateToken() throws CloudApi.Failure {
        JSONObject root = api.json("POST", PATH + "/token", new JSONObject(), CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        String token = root.optString("token", "");
        if (token.isEmpty()) throw new CloudApi.Failure(200, "invalid_response", "token missing", 0);
        return token;
    }

    /** 删除云端快照和令牌。 */
    public void delete() throws CloudApi.Failure {
        api.send("DELETE", PATH, null, CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
    }

    public static String mcpUrl(String id) {
        return CloudApi.ORIGIN + "/mcp/s/" + id;
    }

    /**
     * 编码请求体。只写固定的键：事件只有 `t_ms`、`kind`（枚举值）和可选的 `duration_ms`，性能记录的耗时缺失时记 0；配置快照原样嵌入诊断包里已脱敏的对象。
     */
    public static String requestBody(String platform, String appVersion, Sections sections, Retention ttl) {
        StringBuilder out = new StringBuilder(4096);
        out.append("{\"platform\":");
        quote(out, platform);
        out.append(",\"app_version\":");
        quote(out, appVersion);
        out.append(",\"sections\":{");
        boolean first = true;
        if (sections.crashLogs() != null) {
            first = false;
            out.append("\"crash_logs\":[");
            for (int i = 0; i < sections.crashLogs().size(); i++) {
                CrashLog log = sections.crashLogs().get(i);
                if (i > 0) out.append(',');
                out.append("{\"at\":");
                quote(out, log.at());
                out.append(",\"message\":");
                quote(out, log.message());
                out.append(",\"stack\":");
                quote(out, log.stack());
                out.append('}');
            }
            out.append(']');
        }
        if (sections.perfTrace() != null) {
            if (!first) out.append(',');
            first = false;
            out.append("\"perf_trace\":");
            events(out, sections.perfTrace(), true);
        }
        if (sections.configSnapshot() != null) {
            if (!first) out.append(',');
            first = false;
            out.append("\"config_snapshot\":").append(sections.configSnapshot());
        }
        if (sections.inputEvents() != null) {
            if (!first) out.append(',');
            out.append("\"input_events\":");
            events(out, sections.inputEvents(), false);
        }
        out.append("},\"ttl\":");
        quote(out, ttl.wire());
        out.append('}');
        return out.toString();
    }

    private static void events(StringBuilder out, List<Event> events, boolean durationRequired) {
        out.append('[');
        for (int i = 0; i < events.size(); i++) {
            Event event = events.get(i);
            if (i > 0) out.append(',');
            out.append("{\"t_ms\":").append(event.tMs()).append(",\"kind\":\"").append(event.kind().wire()).append('"');
            if (event.durationMs() >= 0 || durationRequired) {
                out.append(",\"duration_ms\":").append(Math.max(0L, event.durationMs()));
            }
            out.append('}');
        }
        out.append(']');
    }

    /**
     * 从诊断包 zip 里取出要上传的各节。按条目文件名认：`config_snapshot.json`、`input_events.jsonl`（或 `input-events.jsonl`）、`perf.jsonl`（或 `perf_trace.jsonl`、`performance_logs.jsonl`）和 `*.crash`；没选的类别不读。
     */
    public static Sections readBundle(File zip, Include include) throws IOException {
        List<CrashLog> crashes = include.crashLogs() ? new ArrayList<>() : null;
        List<Event> perf = include.performanceLogs() ? new ArrayList<>() : null;
        List<Event> input = include.inputEvents() ? new ArrayList<>() : null;
        String config = null;
        try (ZipInputStream stream = new ZipInputStream(new FileInputStream(zip), StandardCharsets.UTF_8)) {
            for (ZipEntry entry = stream.getNextEntry(); entry != null; entry = stream.getNextEntry()) {
                if (entry.isDirectory()) continue;
                String name = baseName(entry.getName());
                if (include.configSnapshot() && name.equals("config_snapshot.json")) {
                    config = configSnapshot(readEntry(stream));
                } else if (input != null && (name.equals("input_events.jsonl") || name.equals("input-events.jsonl"))) {
                    input.addAll(eventLines(readEntry(stream), false));
                } else if (perf != null && (name.equals("perf.jsonl") || name.equals("perf_trace.jsonl")
                        || name.equals("performance_logs.jsonl"))) {
                    perf.addAll(eventLines(readEntry(stream), true));
                } else if (crashes != null && name.endsWith(".crash") && crashes.size() < MAX_CRASH_LOGS) {
                    crashes.add(crashRecord(readEntry(stream), entry.getTime()));
                }
            }
        }
        return new Sections(crashes, latest(perf), include.configSnapshot() ? (config == null ? "{}" : config) : null,
            latest(input));
    }

    /** 每条 jsonl 都按枚举重建，只取三个数值/枚举字段；坏行和不认识的种类丢弃。 */
    static List<Event> eventLines(String text, boolean durationRequired) {
        List<Event> events = new ArrayList<>();
        for (String line : text.split("\n")) {
            String trimmed = line.trim();
            if (trimmed.isEmpty()) continue;
            try {
                JSONObject row = new JSONObject(trimmed);
                Object kind = row.opt("kind");
                if (!(kind instanceof String) || !row.has("t_ms")) continue;
                if (durationRequired && !row.has("duration_ms")) continue;
                Event event = Event.of(row.getLong("t_ms"), (String) kind, row.optLong("duration_ms", -1));
                if (event != null) events.add(event);
            } catch (JSONException malformed) {
                // 不合规的行丢弃，与 Rust 诊断包和后端的口径一致。
            }
        }
        return events;
    }

    private static String configSnapshot(String text) {
        try {
            return new JSONObject(text).toString();
        } catch (JSONException malformed) {
            return "{}";
        }
    }

    /** `Telemetry` 的崩溃记录：第一行是摘要，其余是堆栈。 */
    static CrashLog crashRecord(String text, long modifiedMillis) {
        int end = text.indexOf('\n');
        String message = end < 0 ? text : text.substring(0, end);
        String stack = end < 0 ? "" : text.substring(end + 1);
        String at = modifiedMillis > 0 ? Instant.ofEpochMilli(modifiedMillis).toString() : "";
        return CrashLog.of(at, message, stack);
    }

    private static List<Event> latest(List<Event> events) {
        if (events == null || events.size() <= MAX_EVENTS) return events;
        return new ArrayList<>(events.subList(events.size() - MAX_EVENTS, events.size()));
    }

    static State parseState(JSONObject root) {
        Snapshot snapshot = null;
        JSONObject raw = root.optJSONObject("snapshot");
        if (raw != null && !raw.optString("id", "").isEmpty()) {
            List<String> sections = new ArrayList<>();
            JSONArray names = raw.optJSONArray("sections");
            if (names != null) {
                for (int i = 0; i < names.length(); i++) sections.add(names.optString(i, ""));
            }
            snapshot = new Snapshot(raw.optString("id", ""), raw.optString("created_at", ""),
                raw.optString("expires_at", ""), raw.optLong("bytes", 0), Collections.unmodifiableList(sections),
                raw.optString("token_hint", ""));
        }
        List<Access> accesses = new ArrayList<>();
        JSONArray list = root.optJSONArray("accesses");
        if (list != null) {
            for (int i = 0; i < list.length(); i++) {
                JSONObject item = list.optJSONObject(i);
                if (item == null) continue;
                Object arguments = item.opt("arguments");
                accesses.add(new Access(item.optString("at", ""), item.optString("tool", ""),
                    arguments == null || arguments == JSONObject.NULL ? "" : String.valueOf(arguments),
                    item.optLong("result_count", 0), item.optLong("bytes", 0)));
            }
        }
        return new State(snapshot, Collections.unmodifiableList(accesses));
    }

    private static String readEntry(InputStream stream) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int total = 0;
        for (int read = stream.read(buffer); read >= 0; read = stream.read(buffer)) {
            total += read;
            if (total > MAX_BODY_BYTES * 4) throw new IOException("diagnostics bundle entry too large");
            out.write(buffer, 0, read);
        }
        return out.toString(StandardCharsets.UTF_8);
    }

    private static String baseName(String path) {
        int slash = path.lastIndexOf('/');
        return (slash < 0 ? path : path.substring(slash + 1)).toLowerCase(Locale.ROOT);
    }

    /** 按 UTF-8 字节截断，不切开多字节字符和代理对。 */
    static String clipUtf8(String value, int maxBytes) {
        if (value == null) return "";
        if (value.getBytes(StandardCharsets.UTF_8).length <= maxBytes) return value;
        int bytes = 0;
        int index = 0;
        while (index < value.length()) {
            int codePoint = value.codePointAt(index);
            int size = codePoint < 0x80 ? 1 : codePoint < 0x800 ? 2 : codePoint < 0x10000 ? 3 : 4;
            if (bytes + size > maxBytes) break;
            bytes += size;
            index += Character.charCount(codePoint);
        }
        return value.substring(0, index);
    }

    /** JSON 字符串转义（RFC 8259）。 */
    static void quote(StringBuilder out, String value) {
        out.append('"');
        String text = value == null ? "" : value;
        for (int i = 0; i < text.length(); i++) {
            char c = text.charAt(i);
            switch (c) {
                case '"': out.append("\\\""); break;
                case '\\': out.append("\\\\"); break;
                case '\n': out.append("\\n"); break;
                case '\r': out.append("\\r"); break;
                case '\t': out.append("\\t"); break;
                case '\b': out.append("\\b"); break;
                case '\f': out.append("\\f"); break;
                default:
                    if (c < 0x20 || c == ' ' || c == ' ') {
                        out.append(String.format(Locale.ROOT, "\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
            }
        }
        out.append('"');
    }
}

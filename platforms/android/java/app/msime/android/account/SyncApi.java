package app.msime.android;

import android.content.Context;
import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.math.BigInteger;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 云同步用到的接口：偏好字段表与偏好文档、无编码常用语、个人词库快照。
 *
 * <p>偏好和常用语都是整文档 CAS：PUT 带上读到的 `revision`，别人先写了就回 409 `revision_conflict`（{@link #conflict}），调用方重新拉取、按 {@link SyncMergePolicy} 合并后再写。词库快照是 NDJSON，可能远大于 {@link CloudApi} 的 4 MiB 响应上限，所以下载直接流式写文件、上传直接从文件流式读，不经过内存。
 *
 * <p>全部用真实账号的会话；匿名账号不同步。每个方法都阻塞在网络上，不要在主线程调用。
 */
public final class SyncApi {
    public static final String PREFERENCES = "/v1/users/me/preferences";
    public static final String PREFERENCES_SCHEMA = "/v1/users/me/preferences/schema";
    public static final String PHRASES = "/v1/users/me/phrases";
    public static final String SNAPSHOT = "/v1/users/me/dictionary/snapshot";
    public static final String REVISION_CONFLICT = "revision_conflict";
    static final String NDJSON = "application/x-ndjson";
    /** 快照文件上限，与 client-core 的 `MAX_DICTIONARY_SNAPSHOT_BYTES` 一致。 */
    public static final long MAX_SNAPSHOT_BYTES = 512L * 1024L * 1024L;
    /** 与 NativeClient 的快照行上限一致，避免合并路径为一行异常数据分配整份快照。 */
    static final int MAX_SNAPSHOT_LINE_CHARS = 65_535;
    /** 与原生快照检查一致，避免合并路径把超量记录全部留在 Java 集合中。 */
    static final int MAX_SNAPSHOT_RECORDS = 500_000;

    /** 偏好文档：`settings` 里的值只有字符串、布尔、整数和浮点数。 */
    public record Preferences(long revision, Map<String, Object> settings) {}

    /** 常用语文档。 */
    public record Phrases(long revision, List<SyncMergePolicy.Phrase> phrases) {}

    /** 流式传输那一层；冒烟测试换成内存实现。下载只在 2xx 时把响应体写进 `out`；上传从 `file` 读请求体。 */
    public interface Streams {
        Exchange download(String path, String token, OutputStream out) throws IOException;

        Exchange upload(String path, String token, Path file, String contentType) throws IOException;
    }

    /** 一次流式往返：状态码、`Retry-After`，非 2xx 或上传时的响应体（有上限）。 */
    public record Exchange(int status, String retryAfter, byte[] body) {}

    private final CloudApi cloud;
    private final CloudApi.Tokens account;
    private final Streams streams;

    public SyncApi(Context context) {
        Context application = context.getApplicationContext();
        this.cloud = new CloudApi(application);
        this.account = rejected -> new BackendAccount(application).currentAccessToken(rejected);
        this.streams = new HttpStreams();
    }

    /** Every token lookup belongs to the same account binding, including a 401 retry. */
    public SyncApi(Context context, long bindingGeneration) {
        Context application = context.getApplicationContext();
        this.account = rejected -> {
            if (SyncSwitch.bindingGeneration(application) != bindingGeneration) return "";
            String token = new BackendAccount(application).currentAccessToken(rejected);
            return SyncSwitch.bindingGeneration(application) == bindingGeneration ? token : "";
        };
        this.cloud = new CloudApi(application, this.account);
        this.streams = new HttpStreams();
    }

    public SyncApi(CloudApi cloud, CloudApi.Tokens account, Streams streams) {
        this.cloud = cloud;
        this.account = account;
        this.streams = streams;
    }

    /** 这次失败是不是 CAS 冲突：别人在我们读之后先写了。 */
    public static boolean conflict(CloudApi.Failure failure) {
        return failure != null && failure.status == 409 && REVISION_CONFLICT.equals(failure.code);
    }

    // ---- 偏好 ----

    /** 服务端字段表 `{fields:{key:{type,…}}}`，原样交给 client-core 的 apply 用。 */
    public JSONObject preferencesSchema() throws CloudApi.Failure {
        return cloud.json("GET", PREFERENCES_SCHEMA, null, CloudApi.Auth.ACCOUNT);
    }

    public Preferences preferences() throws CloudApi.Failure {
        return parsePreferences(cloud.json("GET", PREFERENCES, null, CloudApi.Auth.ACCOUNT));
    }

    /** 整份替换；`revision` 是读到的那一版，冲突时抛出 {@link #conflict} 为真的失败。 */
    public Preferences putPreferences(long revision, Map<String, Object> settings) throws CloudApi.Failure {
        JSONObject body;
        try {
            JSONObject values = new JSONObject();
            for (Map.Entry<String, Object> entry : settings.entrySet()) values.put(entry.getKey(), entry.getValue());
            body = new JSONObject().put("revision", revision).put("settings", values);
        } catch (JSONException invalid) {
            throw new IllegalArgumentException("invalid settings value", invalid);
        }
        return parsePreferences(cloud.json("PUT", PREFERENCES, body, CloudApi.Auth.ACCOUNT));
    }

    static Preferences parsePreferences(JSONObject root) throws CloudApi.Failure {
        long revision = preferenceRevision(root.opt("revision"));
        JSONObject raw = requiredSettings(root.opt("settings"));
        LinkedHashMap<String, Object> settings = new LinkedHashMap<>(raw.length());
        Iterator<String> keys = raw.keys();
        while (keys.hasNext()) {
            String key = keys.next();
            Object value = raw.opt(key);
            if (value instanceof String || value instanceof Boolean || value instanceof Number) {
                settings.put(key, value);
            } else {
                throw invalid("preferences setting must be scalar");
            }
        }
        return new Preferences(revision, Collections.unmodifiableMap(settings));
    }

    /** Successful preference responses always carry an object, including an empty one. */
    static JSONObject requiredSettings(Object value) throws CloudApi.Failure {
        if (!(value instanceof JSONObject)) throw invalid("preferences settings missing");
        return (JSONObject) value;
    }

    /** Account preference revisions are JSON integers in the non-negative long range. */
    static long preferenceRevision(Object value) throws CloudApi.Failure {
        if (!(value instanceof Number)
            || value instanceof Float || value instanceof Double
            || value instanceof java.math.BigDecimal
            || value instanceof BigInteger) {
            throw invalid("preferences revision must be an integer");
        }
        long revision = ((Number) value).longValue();
        if (revision < 0) throw invalid("preferences revision must be non-negative");
        return revision;
    }

    // ---- 常用语 ----

    public Phrases phrases() throws CloudApi.Failure {
        return parsePhrases(cloud.json("GET", PHRASES, null, CloudApi.Auth.ACCOUNT));
    }

    /** 整份替换（先经 {@link SyncMergePolicy#normalized} 去重截断）；冲突时抛出 {@link #conflict} 为真的失败。 */
    public Phrases putPhrases(long revision, List<SyncMergePolicy.Phrase> phrases) throws CloudApi.Failure {
        JSONObject body;
        try {
            JSONArray values = new JSONArray();
            for (SyncMergePolicy.Phrase phrase : SyncMergePolicy.normalized(phrases)) {
                values.put(new JSONObject().put("id", phrase.id()).put("text", phrase.text())
                    .put("group", phrase.group()).put("position", phrase.position()));
            }
            body = new JSONObject().put("revision", revision).put("phrases", values);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        return parsePhrases(cloud.json("PUT", PHRASES, body, CloudApi.Auth.ACCOUNT));
    }

    static Phrases parsePhrases(JSONObject root) throws CloudApi.Failure {
        long revision = phraseRevision(root.opt("revision"));
        JSONArray raw = requiredPhrases(root.opt("phrases"));
        if (raw.length() > SyncMergePolicy.MAX_PHRASES)
            throw invalid("too many phrases");
        List<SyncMergePolicy.Phrase> phrases = new ArrayList<>(raw.length());
        for (int index = 0; index < raw.length(); index++) {
            JSONObject value = raw.optJSONObject(index);
            if (value == null) throw invalid("phrase row must be an object");
            Object id = value.opt("id");
            Object text = value.opt("text");
            if (!(id instanceof String) || !(text instanceof String))
                throw invalid("phrase row is malformed");
            Object group = value.opt("group");
            if (group != null && !(group instanceof String)) throw invalid("phrase group is malformed");
            Object position = value.opt("position");
            phrases.add(new SyncMergePolicy.Phrase((String) id, (String) text,
                group instanceof String ? (String) group : "",
                strictPhrasePosition(position, index)));
        }
        phrases.sort((left, right) -> Integer.compare(left.position(), right.position()));
        return new Phrases(revision, Collections.unmodifiableList(phrases));
    }

    /** Successful phrase responses always carry an array, including an empty one. */
    static JSONArray requiredPhrases(Object value) throws CloudApi.Failure {
        if (!(value instanceof JSONArray)) throw invalid("phrases missing");
        return (JSONArray) value;
    }

    /** Common phrase positions are bounded JSON integers; malformed values keep response order. */
    public static int strictPhrasePosition(Object value, int fallback) {
        if (value instanceof Integer integer
                && integer >= 0 && integer < SyncMergePolicy.MAX_PHRASES) return integer;
        if (value instanceof Long longValue
                && longValue >= 0L && longValue < SyncMergePolicy.MAX_PHRASES) {
            return longValue.intValue();
        }
        return fallback;
    }

    /** Common phrase revisions use the same non-negative integer CAS contract as preferences. */
    static long phraseRevision(Object value) throws CloudApi.Failure {
        return preferenceRevision(value);
    }

    // ---- 词库快照 ----

    /** 词库改动探测：自 `after` 以来有没有改动，以及服务端给的下一页游标（没有改动时就是当前云端版本）。 */
    public record DictionaryProbe(boolean changed, long revision) {}

    /** 云端自 `after` 以来有没有词库改动（只取一条）。 */
    public DictionaryProbe dictionaryChangedSince(long after) throws CloudApi.Failure {
        JSONObject page = cloud.json("GET", changesPath(after), null, CloudApi.Auth.ACCOUNT);
        JSONArray changes = page.optJSONArray("changes");
        long minimum = BoundsPolicy.nonNegative(after);
        long revision = changesRevision(page.opt("next"), minimum);
        return new DictionaryProbe(changes != null && changes.length() > 0, revision);
    }

    /** Dictionary change cursors are non-negative integer revisions and cannot move backwards. */
    static long changesRevision(Object value, long minimum) throws CloudApi.Failure {
        long revision = preferenceRevision(value);
        long floor = BoundsPolicy.nonNegative(minimum);
        if (revision < floor) throw invalid("dictionary revision moved backwards");
        return revision;
    }

    static String changesPath(long after) {
        return "/v1/users/me/dictionary/changes?after=" + BoundsPolicy.nonNegative(after) + "&limit=1";
    }

    static String restorePath(long revision) {
        if (revision < 0) throw new IllegalArgumentException("negative snapshot revision");
        return SNAPSHOT + "?revision=" + revision;
    }

    /**
     * 把云端快照下载到 `destination`（覆盖），返回快照头里的 `revision`。
     *
     * <p>文件先写到同目录的临时名，读完头一行确认是快照再改名，失败时不留半个文件。
     */
    public long downloadSnapshot(Path destination) throws CloudApi.Failure {
        Path partial = destination.resolveSibling(destination.getFileName() + ".partial");
        try {
            try (OutputStream out = Files.newOutputStream(partial, StandardOpenOption.CREATE,
                    StandardOpenOption.TRUNCATE_EXISTING, StandardOpenOption.WRITE,
                    LinkOption.NOFOLLOW_LINKS)) {
                streamed(token -> streams.download(SNAPSHOT, token, new BoundedStream(out, MAX_SNAPSHOT_BYTES)));
            }
            long revision = snapshotRevision(partial);
            Files.move(partial, destination, java.nio.file.StandardCopyOption.REPLACE_EXISTING,
                java.nio.file.StandardCopyOption.ATOMIC_MOVE);
            return revision;
        } catch (IOException disk) {
            throw new CloudApi.Failure(0, "storage", disk.getMessage(), 0);
        } finally {
            try { Files.deleteIfExists(partial); } catch (IOException ignored) { /* 下一轮会覆盖它。 */ }
        }
    }

    /** 用本机导出的快照文件整份替换云端词库，`revision` 是上次同步时的云端版本；返回服务端的新版本。 */
    public long uploadSnapshot(Path file, long revision) throws CloudApi.Failure {
        String path = restorePath(revision);
        Exchange exchange = streamed(token -> streams.upload(path, token, file, NDJSON));
        try {
            JSONObject root = new JSONObject(new String(exchange.body(), StandardCharsets.UTF_8));
            Object next = root.opt("revision");
            long nextRevision = snapshotRevisionValue(next);
            if (nextRevision <= revision) throw invalid("snapshot revision");
            return nextRevision;
        } catch (JSONException malformed) {
            throw invalid("malformed snapshot response");
        }
    }

    /** 快照第一行是 `{"type":"header",…,"revision":N}`。 */
    static long snapshotRevision(Path file) throws IOException, CloudApi.Failure {
        try (BufferedReader reader = Files.newBufferedReader(file, StandardCharsets.UTF_8)) {
            String first = readSnapshotLine(reader);
            if (first == null) throw invalid("empty snapshot");
            JSONObject header = new JSONObject(first);
            Object revision = header.opt("revision");
            if (!"header".equals(header.opt("type"))) throw invalid("snapshot header");
            return snapshotRevisionValue(revision);
        } catch (JSONException malformed) {
            throw invalid("snapshot header");
        }
    }

    /** Snapshot headers and restore responses carry the same integer revision contract. */
    static long snapshotRevisionValue(Object value) throws CloudApi.Failure {
        return preferenceRevision(value);
    }

    /** Snapshot entry weights are positive JSON integers in the host-api range. */
    public static Long strictSnapshotWeight(Object value) {
        if (!(value instanceof Integer) && !(value instanceof Long)) return null;
        long weight = ((Number) value).longValue();
        return weight >= 1L && weight <= 100_000_000L ? weight : null;
    }

    /**
     * 快照里用户自己的词（`type:"entry"`），供「合并」时导入本机个人词库队列。被 `overlay` 标成已删除的词不算。词条数不设上限之外的限制，调用方按 {@link SyncMergePolicy#batches} 分批。
     */
    public static List<SyncMergePolicy.Word> snapshotWords(Path file) throws IOException {
        LinkedHashMap<String, SyncMergePolicy.Word> words = new LinkedHashMap<>();
        java.util.HashSet<String> deleted = new java.util.HashSet<>();
        try (BufferedReader reader = Files.newBufferedReader(file, StandardCharsets.UTF_8)) {
            SnapshotRecordReader records = new SnapshotRecordReader(reader);
            int countedRecords = 0;
            String line;
            while ((line = records.next()) != null) {
                if (line.isEmpty()) continue;
                JSONObject record;
                try {
                    record = new JSONObject(line);
                } catch (JSONException malformed) {
                    throw new IOException("malformed snapshot line", malformed);
                }
                Object type = record.opt("type");
                if (!"footer".equals(type) && ++countedRecords > MAX_SNAPSHOT_RECORDS)
                    throw new IOException("snapshot has too many records");
                JSONObject data = record.optJSONObject("data");
                if (data == null || !(data.opt("id") instanceof String)) continue;
                String id = (String) data.opt("id");
                if ("overlay".equals(type)) {
                    if (JsonPolicy.strictTrue(record.opt("deleted"))) deleted.add(id);
                    continue;
                }
                if (!"entry".equals(type)) continue;
                Object kind = data.opt("kind");
                Object code = data.opt("code");
                Object word = data.opt("word");
                Object weight = data.opt("weight");
                if (!(kind instanceof String) || !(code instanceof String) || !(word instanceof String)) continue;
                Long strictWeight = strictSnapshotWeight(weight);
                if (strictWeight == null) throw new IOException("invalid snapshot weight");
                words.put(id, new SyncMergePolicy.Word((String) kind, (String) code, (String) word,
                    strictWeight));
            }
        }
        for (String id : deleted) words.remove(id);
        return new ArrayList<>(words.values());
    }

    /** 逐字符读一行并在超过快照契约前失败，不能先调用无界的 {@link BufferedReader#readLine}。 */
    static String readSnapshotLine(BufferedReader reader) throws IOException {
        StringBuilder line = new StringBuilder(256);
        int value;
        while ((value = reader.read()) != -1) {
            if (value == '\n') {
                int length = line.length();
                if (length > 0 && line.charAt(length - 1) == '\r') line.setLength(length - 1);
                return line.toString();
            }
            if (line.length() >= MAX_SNAPSHOT_LINE_CHARS)
                throw new IOException("snapshot line too large");
            line.append((char) value);
        }
        return line.length() == 0 ? null : line.toString();
    }

    /** 带总记录数上限的快照逐行读取器，避免调用方先把所有记录放进集合。 */
    static final class SnapshotRecordReader {
        private final BufferedReader reader;
        private int records;

        SnapshotRecordReader(BufferedReader reader) {
            this.reader = reader;
        }

        String next() throws IOException {
            String line = readSnapshotLine(reader);
            if (line == null) return null;
            // 原生 records 不包含 footer；预留这一行后，再由 snapshotWords 按 type 计数。
            if (++records > MAX_SNAPSHOT_RECORDS + 1)
                throw new IOException("snapshot has too many records");
            return line;
        }
    }

    interface Call {
        Exchange run(String token) throws IOException;
    }

    /** 带令牌发一次流式请求，401 时换一枚新令牌重试一次；非 2xx 读成 {@link CloudApi.Failure}。 */
    Exchange streamed(Call call) throws CloudApi.Failure {
        String rejected = null;
        for (int attempt = 0; ; attempt++) {
            String token;
            try {
                token = account.token(rejected);
            } catch (Exception unavailable) {
                throw new CloudApi.Failure(0, "session_unavailable", unavailable.getMessage(), 0);
            }
            if (token == null || token.isEmpty()) throw new CloudApi.Failure(401, "signed_out", "not signed in", 0);
            Exchange exchange;
            try {
                exchange = call.run(token);
            } catch (IOException offline) {
                throw new CloudApi.Failure(0, "network", offline.getMessage(), 0);
            }
            if (exchange.status() / 100 == 2) return exchange;
            if (exchange.status() == 401 && attempt == 0) {
                rejected = token;
                continue;
            }
            throw CloudApi.failure(new CloudApi.Exchange(exchange.status(), "application/json",
                exchange.retryAfter(), exchange.body()));
        }
    }

    private static CloudApi.Failure invalid(String message) {
        return new CloudApi.Failure(500, "invalid_response", message, 0);
    }

    /** 超过上限就中断下载，免得一个异常的响应写满磁盘。 */
    static final class BoundedStream extends OutputStream {
        private final OutputStream target;
        private final long limit;
        private long written;

        BoundedStream(OutputStream target, long limit) {
            this.target = target;
            this.limit = limit;
        }

        @Override public void write(int value) throws IOException {
            if (++written > limit) throw new IOException("snapshot too large");
            target.write(value);
        }

        @Override public void write(byte[] bytes, int offset, int length) throws IOException {
            written += length;
            if (written > limit) throw new IOException("snapshot too large");
            target.write(bytes, offset, length);
        }

        @Override public void flush() throws IOException { target.flush(); }
    }

    private static final class HttpStreams implements Streams {
        @Override public Exchange download(String path, String token, OutputStream out) throws IOException {
            HttpsURLConnection connection = open(path, "GET", token, NDJSON);
            try {
                int status = connection.getResponseCode();
                if (status / 100 == 2) {
                    try (InputStream input = connection.getInputStream()) {
                        HttpBodyPolicy.copy(input, out);
                    }
                    out.flush();
                    return new Exchange(status, null, new byte[0]);
                }
                return new Exchange(status, connection.getHeaderField("Retry-After"), errorBody(connection));
            } finally {
                connection.disconnect();
            }
        }

        @Override public Exchange upload(String path, String token, Path file, String contentType) throws IOException {
            long length = Files.size(file);
            if (length <= 0 || length > MAX_SNAPSHOT_BYTES) throw new IOException("snapshot size out of range");
            HttpsURLConnection connection = open(path, "PUT", token, "application/json");
            try {
                connection.setRequestProperty("Content-Type", contentType);
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(length);
                try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS); OutputStream output = connection.getOutputStream()) {
                    HttpBodyPolicy.copy(input, output);
                }
                int status = connection.getResponseCode();
                if (status / 100 == 2) {
                    try (InputStream input = connection.getInputStream()) {
                        return new Exchange(status, null,
                            HttpBodyPolicy.readRequired(input, CloudApi.MAX_RESPONSE_BYTES));
                    }
                }
                return new Exchange(status, connection.getHeaderField("Retry-After"), errorBody(connection));
            } finally {
                connection.disconnect();
            }
        }

        private static HttpsURLConnection open(String path, String method, String token, String accept)
                throws IOException {
            HttpsURLConnection connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + path).openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod(method);
            connection.setConnectTimeout(CloudApi.CONNECT_TIMEOUT_MILLIS);
            // 快照在服务端整份导出或原子恢复，读超时和 client-core 一样放宽到两分钟以上。
            connection.setReadTimeout(130_000);
            connection.setRequestProperty("Accept", accept);
            connection.setRequestProperty("User-Agent", CloudApi.USER_AGENT);
            connection.setRequestProperty("Authorization", "Bearer " + token);
            return connection;
        }

        private static byte[] errorBody(HttpsURLConnection connection) throws IOException {
            InputStream error = connection.getErrorStream();
            if (error == null) return new byte[0];
            try (InputStream input = error) {
                return HttpBodyPolicy.readRequired(input, CloudApi.MAX_RESPONSE_BYTES);
            }
        }

    }
}

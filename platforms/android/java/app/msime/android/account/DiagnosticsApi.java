package app.msime.android;

import android.content.Context;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「MCP 开发者访问」的云端操作：上传一份日志快照、读回快照与最近访问、重新生成访问令牌、删除快照，都经 Rust 的 {@code msime_client_account}（diagnostics_upload、diagnostics_state、diagnostics_regenerate_token、diagnostics_delete，#8201 迁移计划 5c）。
 *
 * <p>上传的内容由 Rust 从本机记录现读现建：请求里给的是与 `NativeClient.diagnosticBundle` 相同的 bundle（状态根、勾选的类别和各类记录的源文件），Rust 按服务端的规则重建（事件只留认识的种类和 `t_ms`、`kind`、`duration_ms`，崩溃记录最多 200 条，配置快照已脱敏），整份超过 2 MiB 时先截掉最旧的事件。请求体里不会出现任何文本、拼音、候选或按键字面值。
 *
 * <p>登录账号优先、没有登录时用匿名账号上传；之后的查看、换令牌和删除绑定到上传时 Rust 交回的 `user_id`，登录会话和匿名会话都不是它时是 account_identity_changed，不会去动另一个账号的快照。每个请求都阻塞在网络上，不要在主线程调用。
 */
public final class DiagnosticsApi {
    /** 读回的访问记录最多保留多少条，超出时保留最新的。 */
    static final int MAX_EVENTS = 8000;

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

    /** 上传哪几类日志，对应偏好 `developer_options.mcp_upload.*`。 */
    public record Include(boolean crashLogs, boolean performanceLogs, boolean inputEvents,
            boolean configSnapshot) {
        public boolean any() { return crashLogs || performanceLogs || inputEvents || configSnapshot; }
    }

    /** 上传成功的回答；`token` 只在这里出现一次。`userId` 是上传所用的账号，之后的三个操作要带上它。 */
    public record Created(String id, String mcpUrl, String token, String expiresAt, String userId) {}

    /** 云端现有的快照。 */
    public record Snapshot(String id, String createdAt, String expiresAt, long bytes, List<String> sections,
            String tokenHint) {
        /** 远程地址；读状态时没有 `mcp_url`，按 id 拼出与上传回答相同的地址。 */
        public String mcpUrl() { return DiagnosticsApi.mcpUrl(id); }
    }

    /** 开发者的一次读取。`arguments` 是调用参数的 JSON 原文（≤1 KiB）。 */
    public record Access(String at, String tool, String arguments, long resultCount, long bytes) {}

    /** 读状态的回答；`snapshot` 为 null 表示云端没有快照。 */
    public record State(Snapshot snapshot, List<Access> accesses) {
        public static final State EMPTY = new State(null, ListPolicy.copyOrEmpty(null));
    }

    private DiagnosticsApi() {}

    /**
     * 这台设备现在会以哪个账号上传：登录账号的编号，没有登录时是匿名账号的编号；两种会话都没有时为空串。页面打开时按它读快照状态。可能刷新令牌、阻塞网络。
     */
    public static String uploaderId(Context context) throws AccountStore.Failure {
        try {
            return JsonPolicy.strictStringOrEmpty(AccountStore.accessToken(context, null).opt("user_id"));
        } catch (AccountStore.Failure failure) {
            if ("account_unauthorized".equals(failure.code)) return "";
            throw failure;
        }
    }

    /**
     * 上传一份快照，替换云端已有的那份。`bundle` 是 `{state_root, include, sources}`，与 `NativeClient.diagnosticBundle` 的请求相同、只是没有 destination。没有任何会话时先注册匿名账号再试一次。
     */
    public static Created upload(Context context, String platform, String appVersion, JSONObject bundle,
            Retention ttl) throws AccountStore.Failure {
        final JSONObject action;
        try {
            action = new JSONObject().put("operation", "diagnostics_upload").put("platform", platform)
                .put("app_version", appVersion).put("ttl", ttl.wire()).put("bundle", bundle);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
        Object value = RustAccount.preferredAccount(context, action);
        if (!(value instanceof JSONObject root)) throw new AccountStore.Failure("account_unavailable", 0);
        String id = JsonPolicy.strictStringOrEmpty(root.opt("id"));
        String token = JsonPolicy.strictStringOrEmpty(root.opt("token"));
        String userId = JsonPolicy.strictStringOrEmpty(root.opt("user_id"));
        if (id.isEmpty() || token.isEmpty() || userId.isEmpty()) throw new AccountStore.Failure("account_unavailable", 0);
        String url = JsonPolicy.strictStringOrEmpty(root.opt("mcp_url"));
        return new Created(id, url.isEmpty() ? mcpUrl(id) : url, token,
            JsonPolicy.strictStringOrEmpty(root.opt("expires_at")), userId);
    }

    /** `userId` 名下的云端快照与最近访问；`userId` 为空（这台设备还没有任何会话）时是空状态，不联网。 */
    public static State state(Context context, String userId) throws AccountStore.Failure {
        if (userId == null || userId.isEmpty()) return State.EMPTY;
        Object value = RustAccount.account(context, bound("diagnostics_state", userId));
        if (!(value instanceof JSONObject root)) throw new AccountStore.Failure("account_unavailable", 0);
        return parseState(root);
    }

    /** 换一枚访问令牌，旧令牌立即作废；返回新令牌（只出现这一次）。没有有效快照时是 account_not_found。 */
    public static String regenerateToken(Context context, String userId) throws AccountStore.Failure {
        Object value = RustAccount.account(context, bound("diagnostics_regenerate_token", userId));
        String token = value instanceof JSONObject root ? JsonPolicy.strictStringOrEmpty(root.opt("token")) : "";
        if (token.isEmpty()) throw new AccountStore.Failure("account_unavailable", 0);
        return token;
    }

    /** 删除云端快照和令牌；没有快照也算成功。 */
    public static void delete(Context context, String userId) throws AccountStore.Failure {
        RustAccount.account(context, bound("diagnostics_delete", userId));
    }

    public static String mcpUrl(String id) {
        return CloudApi.ORIGIN + "/mcp/s/" + id;
    }

    private static JSONObject bound(String operation, String userId) throws AccountStore.Failure {
        try {
            return new JSONObject().put("operation", operation).put("user_id", userId == null ? "" : userId);
        } catch (JSONException impossible) {
            throw new AccountStore.Failure("account_invalid", 0);
        }
    }

    static State parseState(JSONObject root) {
        Snapshot snapshot = null;
        JSONObject raw = root.optJSONObject("snapshot");
        String snapshotId = raw == null ? null : optionalString(raw, "id");
        Long snapshotBytes = raw == null ? null : optionalInteger(raw, "bytes");
        String snapshotCreated = raw == null ? null : optionalString(raw, "created_at");
        String snapshotExpires = raw == null ? null : optionalString(raw, "expires_at");
        String snapshotHint = raw == null ? null : optionalString(raw, "token_hint");
        if (raw != null && snapshotId != null && !snapshotId.isEmpty()
                && snapshotBytes != null && snapshotBytes >= 0
                && snapshotCreated != null && snapshotExpires != null && snapshotHint != null) {
            JSONArray names = raw.optJSONArray("sections");
            List<String> sections = new ArrayList<>(names == null ? 0 : names.length());
            if (names != null) {
                for (int i = 0; i < names.length(); i++) {
                    String section = JsonPolicy.strictString(names.opt(i));
                    if (section != null) sections.add(section);
                }
            }
            snapshot = new Snapshot(snapshotId, snapshotCreated, snapshotExpires, snapshotBytes,
                ListPolicy.copyOrEmpty(sections), snapshotHint);
        }
        JSONArray list = root.optJSONArray("accesses");
        int accessCount = list == null ? 0 : list.length();
        int firstAccess = BoundsPolicy.nonNegative(accessCount - MAX_EVENTS);
        List<Access> accesses = new ArrayList<>(BoundsPolicy.atMost(accessCount, MAX_EVENTS));
        if (list != null) {
            for (int i = firstAccess; i < list.length(); i++) {
                JSONObject item = list.optJSONObject(i);
                if (item == null) continue;
                Object arguments = item.opt("arguments");
                String at = optionalString(item, "at");
                String tool = optionalString(item, "tool");
                // Rust 把服务端记下的参数原样交回：通常是对象，也可能是别的 JSON 值。
                String rawArguments = arguments == null || arguments == JSONObject.NULL ? ""
                    : arguments instanceof JSONObject || arguments instanceof JSONArray ? arguments.toString()
                    : JsonPolicy.strictString(arguments);
                Long resultCount = optionalInteger(item, "result_count");
                Long bytes = optionalInteger(item, "bytes");
                if (at == null || tool == null || rawArguments == null || resultCount == null || bytes == null
                        || resultCount < 0 || bytes < 0) continue;
                accesses.add(new Access(at, tool, rawArguments, resultCount, bytes));
            }
        }
        return new State(snapshot, ListPolicy.copyOrEmpty(accesses));
    }

    /** Optional response strings: absent/null means empty, every other JSON type is malformed. */
    static String optionalString(JSONObject object, String key) {
        if (object == null || !object.has(key) || object.isNull(key)) return "";
        return JsonPolicy.strictString(object.opt(key));
    }

    /** Optional response integers: absent/null means zero, every other non-integer is malformed. */
    static Long optionalInteger(JSONObject object, String key) {
        if (object == null || !object.has(key) || object.isNull(key)) return 0L;
        return JsonPolicy.strictLong(object.opt(key));
    }
}

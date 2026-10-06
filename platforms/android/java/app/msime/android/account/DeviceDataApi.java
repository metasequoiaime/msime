package app.msime.android;

import android.content.Context;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.time.Instant;
import java.time.LocalDate;
import java.time.format.DateTimeParseException;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 「我的」里个人资料、我的设备与云端数据用到的接口：`/v1/users/me`（读、改昵称、注销）、头像、会话列表与撤销、云端数据汇总 / 导出 / 删除。
 *
 * <p>全部建在 {@link CloudApi} 上，用真实账号的会话（匿名账号不算「我的」）。注销账号和删除云端数据要求最近登录，服务端回 403 `recent_login_required` 时这里抛 {@link RecentLoginRequired}，界面据此让用户重新登录一次再重试。导出是一个可能很大的 zip，不走 CloudApi 的 4 MiB 响应上限，由 {@link #exportData} 直接流式写进调用方给的输出流。
 *
 * <p>每个方法都阻塞在网络上，不要在主线程调用。
 */
public final class DeviceDataApi {
    /** 昵称上限，与服务端 `utf8.RuneCountInString(name) > 64` 一致，按码点计。 */
    public static final int MAX_DISPLAY_NAME = 64;
    /** 头像上传上限，与服务端 `maxAvatarUploadBytes` 一致。 */
    public static final int MAX_AVATAR_BYTES = 1024 * 1024;
    /** 账号响应里允许展开的会话数。 */
    public static final int MAX_SESSIONS = 100;
    /** 账号响应里允许展开的数据分类数。 */
    public static final int MAX_DATA_SECTIONS = 16;
    /** 账号响应里允许展开的关联身份数，与共享账号校验保持一致。 */
    public static final int MAX_IDENTITIES = 16;
    public static final String RECENT_LOGIN_REQUIRED = "recent_login_required";
    /** 云端数据里可以单独删除的分类，顺序即确认框里的顺序。 */
    public static final List<String> DELETABLE_SECTIONS = List.of("preferences", "dictionary", "phrases", "clipboard");

    /** 这个操作要求最近登录过的会话；重新登录后再试。 */
    public static final class RecentLoginRequired extends Exception {
        private static final long serialVersionUID = 1L;

        public RecentLoginRequired(CloudApi.Failure cause) {
            super("recent login required", cause);
        }
    }

    /** `GET /v1/users/me` 的用户与已关联的登录方式（只取 provider 名，subject 不给界面看）。 */
    public record Profile(String id, String displayName, String email, String avatarUrl, List<String> providers) {
        public boolean linked(String provider) { return providers.contains(provider); }

        /** 真实账号的登录方式里排在最前的一种（google / apple / email），一种都没有时为空字符串。 */
        public String loginKind() {
            for (String kind : SyncSwitch.LOGIN_KINDS) if (providers.contains(kind)) return kind;
            return "";
        }
    }

    /** 一个未撤销、未过期的登录会话；时间是 Unix 毫秒，读不出时为 0。 */
    public record Session(String id, String platform, String name, String appVersion, long createdAt,
            long lastActive, boolean current) {}

    /** 云端数据的一个分类。 */
    public record DataSection(String id, long bytes, long items) {}

    /** `GET /v1/users/me/data`：总字节数（统计口径，不是精确的磁盘占用）与各分类。 */
    public record DataSummary(long bytes, List<DataSection> sections) {}

    /** 流式下载的那一层；冒烟测试换成内存实现。只在 2xx 时把响应体写进 `out`，否则把错误体放进返回值。 */
    public interface Downloader {
        Download download(String path, String token, OutputStream out) throws IOException;
    }

    /** 一次流式下载的结果：状态码，非 2xx 时的错误体与 `Retry-After`。 */
    public record Download(int status, String retryAfter, byte[] errorBody) {}

    private final CloudApi cloud;
    private final CloudApi.Tokens account;
    private final Downloader downloader;

    public DeviceDataApi(Context context) {
        Context application = context.getApplicationContext();
        this.cloud = new CloudApi(application);
        this.account = rejected -> new BackendAccount(application).currentAccessToken(rejected);
        this.downloader = DeviceDataApi::httpDownload;
    }

    public DeviceDataApi(CloudApi cloud, CloudApi.Tokens account, Downloader downloader) {
        this.cloud = cloud;
        this.account = account;
        this.downloader = downloader;
    }

    // ---- 个人资料 ----

    public Profile profile() throws CloudApi.Failure {
        return parseProfile(cloud.json("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT));
    }

    /** 改昵称；去掉首尾空白后为空表示恢复服务端的默认昵称。 */
    public void rename(String displayName) throws CloudApi.Failure {
        String name = displayName == null ? "" : displayName.trim();
        if (!validDisplayName(name)) throw new IllegalArgumentException("invalid display name");
        JSONObject body;
        try {
            body = new JSONObject().put("display_name", name);
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        cloud.send("PATCH", "/v1/users/me", CloudApi.Body.json(body), CloudApi.Auth.ACCOUNT);
    }

    /** 上传头像（PNG 或 JPEG，≤1 MiB，服务端重编码为 256 px），返回带新 `avatar_url` 的资料。 */
    public Profile uploadAvatar(byte[] image) throws CloudApi.Failure {
        String type = avatarType(image);
        if (type == null) throw new IllegalArgumentException("avatar must be PNG or JPEG");
        if (image.length > MAX_AVATAR_BYTES) throw new IllegalArgumentException("avatar too large");
        CloudApi.Response response = cloud.send("PUT", "/v1/users/me/avatar", new CloudApi.Body(type, image),
            CloudApi.Auth.ACCOUNT);
        try {
            return parseProfile(response.json());
        } catch (JSONException malformed) {
            throw new CloudApi.Failure(response.status(), "invalid_response", "malformed profile", 0);
        }
    }

    public void deleteAvatar() throws CloudApi.Failure {
        cloud.send("DELETE", "/v1/users/me/avatar", null, CloudApi.Auth.ACCOUNT);
    }

    /** 注销账号：服务端立即删除账号与它的全部云端数据、社区作品和会话。要求最近登录。 */
    public void deleteAccount() throws CloudApi.Failure, RecentLoginRequired {
        try {
            cloud.send("DELETE", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
        } catch (CloudApi.Failure failure) {
            throw recentLogin(failure);
        }
    }

    // ---- 我的设备 ----

    public List<Session> sessions() throws CloudApi.Failure {
        JSONObject root = cloud.json("GET", "/v1/users/me/sessions", null, CloudApi.Auth.ACCOUNT);
        JSONArray rows = root.optJSONArray("sessions");
        if (rows == null) throw new CloudApi.Failure(500, "invalid_response", "sessions missing", 0);
        if (!validResponseArrayLength(rows.length(), MAX_SESSIONS)) {
            throw new CloudApi.Failure(500, "invalid_response", "too many sessions", 0);
        }
        List<Session> sessions = new ArrayList<>(rows.length());
        for (int index = 0; index < rows.length(); index++) {
            JSONObject row = rows.optJSONObject(index);
            if (row == null) continue;
            String id = string(row, "id");
            if (!validSessionId(id)) continue;
            sessions.add(new Session(id, string(row, "platform"), string(row, "name"), string(row, "app_version"),
                instant(string(row, "created_at")), instant(string(row, "last_active")),
                Boolean.TRUE.equals(row.opt("current"))));
        }
        return Collections.unmodifiableList(sessions);
    }

    /** 撤销一个会话；撤销当前会话等于在服务端退出登录，调用方随后要清掉本机会话。 */
    public void revokeSession(String id) throws CloudApi.Failure {
        if (!validSessionId(id)) throw new IllegalArgumentException("invalid session id");
        cloud.send("DELETE", "/v1/users/me/sessions/" + id, null, CloudApi.Auth.ACCOUNT);
    }

    // ---- 云端数据 ----

    public DataSummary dataSummary() throws CloudApi.Failure {
        JSONObject root = cloud.json("GET", "/v1/users/me/data", null, CloudApi.Auth.ACCOUNT);
        JSONArray rows = root.optJSONArray("sections");
        if (rows != null && !validResponseArrayLength(rows.length(), MAX_DATA_SECTIONS)) {
            throw new CloudApi.Failure(500, "invalid_response", "too many data sections", 0);
        }
        List<DataSection> sections = new ArrayList<>(rows == null ? 0 : rows.length());
        if (rows != null) {
            for (int index = 0; index < rows.length(); index++) {
                JSONObject row = rows.optJSONObject(index);
                if (row == null || string(row, "id").isEmpty()) continue;
                sections.add(new DataSection(string(row, "id"), strictCount(row.opt("bytes")),
                    strictCount(row.opt("items"))));
            }
        }
        return new DataSummary(strictCount(root.opt("bytes")), Collections.unmodifiableList(sections));
    }

    /**
     * 把一次性导出的 zip 流式写进 `out`（每用户每天 3 次，超出时是 429）。令牌被拒时换一枚新的只重试一次，与 CloudApi 相同。
     *
     * @return 写入的字节数
     */
    public long exportData(OutputStream out) throws CloudApi.Failure {
        String rejected = null;
        for (int attempt = 0; ; attempt++) {
            String token;
            try {
                token = account.token(rejected);
            } catch (Exception unavailable) {
                throw new CloudApi.Failure(0, "session_unavailable", unavailable.getMessage(), 0);
            }
            if (token == null || token.isEmpty()) throw new CloudApi.Failure(401, "signed_out", "not signed in", 0);
            CountingStream counted = new CountingStream(out);
            Download result;
            try {
                result = downloader.download("/v1/users/me/data/export", token, counted);
            } catch (IOException offline) {
                throw new CloudApi.Failure(0, "network", offline.getMessage(), 0);
            }
            if (result.status() / 100 == 2) return counted.count;
            if (result.status() == 401 && attempt == 0) {
                rejected = token;
                continue;
            }
            throw CloudApi.failure(new CloudApi.Exchange(result.status(), "application/json", result.retryAfter(),
                result.errorBody()));
        }
    }

    /** 只删数据、保留账号：删掉 `sections`（{@link #DELETABLE_SECTIONS} 的子集）。要求最近登录。 */
    public void deleteData(List<String> sections) throws CloudApi.Failure, RecentLoginRequired {
        if (sections == null || sections.isEmpty() || !DELETABLE_SECTIONS.containsAll(sections)) {
            throw new IllegalArgumentException("invalid data sections");
        }
        JSONObject body;
        try {
            body = new JSONObject().put("sections", new JSONArray(sections));
        } catch (JSONException impossible) {
            throw new IllegalStateException(impossible);
        }
        try {
            cloud.send("DELETE", "/v1/users/me/data", CloudApi.Body.json(body), CloudApi.Auth.ACCOUNT);
        } catch (CloudApi.Failure failure) {
            throw recentLogin(failure);
        }
    }

    // ---- 纯逻辑（冒烟测试覆盖） ----

    /** 403 `recent_login_required`：要求重新登录一次。 */
    public static boolean recentLoginRequired(CloudApi.Failure failure) {
        return failure != null && failure.status == 403 && RECENT_LOGIN_REQUIRED.equals(failure.code);
    }

    private static CloudApi.Failure recentLogin(CloudApi.Failure failure) throws RecentLoginRequired {
        if (recentLoginRequired(failure)) throw new RecentLoginRequired(failure);
        return failure;
    }

    /** 昵称：去掉首尾空白后不超过 64 个码点，不含控制字符。空字符串合法（恢复默认昵称）。 */
    public static boolean validDisplayName(String name) {
        if (name == null) return false;
        if (name.codePointCount(0, name.length()) > MAX_DISPLAY_NAME) return false;
        for (int index = 0; index < name.length(); index++) {
            char c = name.charAt(index);
            if (c < 0x20 || c == 0x7F) return false;
        }
        return true;
    }

    /** 会话 id 只拼进路径，所以只认 1–128 个字母、数字、`-`、`_`。 */
    public static boolean validSessionId(String id) {
        if (id == null || id.isEmpty() || id.length() > 128) return false;
        for (int index = 0; index < id.length(); index++) {
            char c = id.charAt(index);
            boolean allowed = c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '-' || c == '_';
            if (!allowed) return false;
        }
        return true;
    }

    /** 数组长度检查独立出来供无 `org.json` 的宿主冒烟测试覆盖。 */
    public static boolean validResponseArrayLength(int length, int maximum) {
        return length >= 0 && maximum >= 0 && length <= maximum;
    }

    /** 按文件头认头像格式：PNG 签名或 JPEG 的 SOI 标记；都不是时为 null。 */
    public static String avatarType(byte[] image) {
        if (image == null || image.length < 4) return null;
        if ((image[0] & 0xFF) == 0x89 && image[1] == 'P' && image[2] == 'N' && image[3] == 'G') return "image/png";
        if ((image[0] & 0xFF) == 0xFF && (image[1] & 0xFF) == 0xD8 && (image[2] & 0xFF) == 0xFF) return "image/jpeg";
        return null;
    }

    /** 给人看的大小：`512 B`、`3.2 KB`、`12.4 MB`、`1.1 GB`（1024 进制，一位小数）。 */
    public static String formatBytes(long bytes) {
        long value = BoundsPolicy.nonNegative(bytes);
        if (value < 1024) return value + " B";
        String[] units = {"KB", "MB", "GB", "TB"};
        double scaled = value;
        int unit = -1;
        while (scaled >= 1024 && unit < units.length - 1) {
            scaled /= 1024;
            unit++;
        }
        return String.format(Locale.ROOT, "%.1f %s", scaled, units[unit]);
    }

    /** 给人看的相对时间：一分钟内「刚刚」，然后「N 分钟前」「N 小时前」「N 天前」；时间未知（0）时为空字符串。 */
    public static String relativeTime(long nowMillis, long thenMillis) {
        if (thenMillis <= 0) return "";
        long minutes = BoundsPolicy.nonNegative(nowMillis - thenMillis) / 60_000L;
        if (minutes < 1) return "刚刚";
        if (minutes < 60) return minutes + " 分钟前";
        long hours = minutes / 60;
        if (hours < 24) return hours + " 小时前";
        return (hours / 24) + " 天前";
    }

    /** 导出文件名，与服务端 `Content-Disposition` 的写法一致。 */
    public static String exportFileName(LocalDate date) {
        return "msime-data-" + date + ".zip";
    }

    /** RFC 3339 时间转 Unix 毫秒；读不出时为 0。 */
    static long instant(String value) {
        if (value == null || value.isEmpty()) return 0L;
        try {
            return Instant.parse(value).toEpochMilli();
        } catch (DateTimeParseException malformed) {
            return 0L;
        }
    }

    private static Profile parseProfile(JSONObject root) throws CloudApi.Failure {
        JSONObject user = root.optJSONObject("user");
        if (user == null || string(user, "id").isEmpty()) {
            throw new CloudApi.Failure(500, "invalid_response", "user missing", 0);
        }
        JSONArray identities = root.optJSONArray("identities");
        if (identities != null && !validResponseArrayLength(identities.length(), MAX_IDENTITIES)) {
            throw new CloudApi.Failure(500, "invalid_response", "too many identities", 0);
        }
        List<String> providers = new ArrayList<>(identities == null ? 0 : identities.length());
        if (identities != null) {
            for (int index = 0; index < identities.length(); index++) {
                JSONObject identity = identities.optJSONObject(index);
                String provider = identity == null ? "" : string(identity, "provider");
                if (!provider.isEmpty() && !providers.contains(provider)) providers.add(provider);
            }
        }
        return new Profile(string(user, "id"), string(user, "display_name"), string(user, "email"),
            string(user, "avatar_url"), Collections.unmodifiableList(providers));
    }

    private static String string(JSONObject object, String key) {
        Object value = object.opt(key);
        return value instanceof String text ? text : "";
    }

    /** Data summary counters are JSON integers; reject coercion and negative values. */
    public static long strictCount(Object value) throws CloudApi.Failure {
        if (!(value instanceof Integer) && !(value instanceof Long))
            throw new CloudApi.Failure(500, "invalid_response", "invalid data count", 0);
        long count = ((Number) value).longValue();
        if (count < 0) throw new CloudApi.Failure(500, "invalid_response", "invalid data count", 0);
        return count;
    }

    /** 数一数写了多少字节，原样转给下游。 */
    private static final class CountingStream extends OutputStream {
        private final OutputStream target;
        long count;

        CountingStream(OutputStream target) { this.target = target; }

        @Override public void write(int value) throws IOException {
            target.write(value);
            count++;
        }

        @Override public void write(byte[] buffer, int offset, int length) throws IOException {
            target.write(buffer, offset, length);
            count += length;
        }

        @Override public void flush() throws IOException { target.flush(); }
    }

    private static Download httpDownload(String path, String token, OutputStream out) throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + path).openConnection();
        try {
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("GET");
            connection.setConnectTimeout(CloudApi.CONNECT_TIMEOUT_MILLIS);
            // 导出在服务端现拼，给它比普通请求长的读超时。
            connection.setReadTimeout(CloudApi.READ_TIMEOUT_MILLIS * 4);
            connection.setRequestProperty("Accept", "application/zip");
            connection.setRequestProperty("User-Agent", CloudApi.USER_AGENT);
            connection.setRequestProperty("Authorization", "Bearer " + token);
            int status = connection.getResponseCode();
            if (status / 100 == 2) {
                try (InputStream input = connection.getInputStream()) {
                    HttpBodyPolicy.copy(input, out);
                }
                out.flush();
                return new Download(status, null, new byte[0]);
            }
            InputStream error = connection.getErrorStream();
            byte[] body = new byte[0];
            if (error != null) {
                try (InputStream input = error) {
                    body = HttpBodyPolicy.readRequired(input, CloudApi.MAX_RESPONSE_BYTES);
                }
            }
            return new Download(status, connection.getHeaderField("Retry-After"), body);
        } finally {
            connection.disconnect();
        }
    }
}

import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;
import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.time.LocalDate;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public final class DeviceDataApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // 403 recent_login_required 是要求重新登录，其他 403 不是。
        check(DeviceDataApi.recentLoginRequired(new CloudApi.Failure(403, "recent_login_required", "", 0)),
            "recent_login_required is recognised");
        check(!DeviceDataApi.recentLoginRequired(new CloudApi.Failure(403, "forbidden", "", 0)), "other 403");
        check(!DeviceDataApi.recentLoginRequired(new CloudApi.Failure(401, "recent_login_required", "", 0)),
            "only 403 counts");

        // 昵称按码点计 64 个，空字符串是恢复默认。
        check(DeviceDataApi.validDisplayName(""), "an empty name restores the default");
        check(DeviceDataApi.validDisplayName("林".repeat(64)), "64 code points");
        check(!DeviceDataApi.validDisplayName("林".repeat(65)), "65 code points");
        check(DeviceDataApi.validDisplayName("🌲".repeat(64)), "astral characters count once");
        check(!DeviceDataApi.validDisplayName("a\nb"), "control characters are refused");
        check(!DeviceDataApi.validDisplayName(null), "null is refused");

        // 会话 id 只拼进路径。
        check(DeviceDataApi.validSessionId("a1B2-c_3"), "plain session id");
        check(!DeviceDataApi.validSessionId("../me"), "path traversal is refused");
        check(!DeviceDataApi.validSessionId(""), "empty id is refused");
        check(!DeviceDataApi.validSessionId("a".repeat(129)), "overlong id is refused");

        // 头像按文件头认格式。
        check("image/png".equals(DeviceDataApi.avatarType(new byte[] {(byte) 0x89, 'P', 'N', 'G', 13, 10})), "png");
        check("image/jpeg".equals(DeviceDataApi.avatarType(new byte[] {(byte) 0xFF, (byte) 0xD8, (byte) 0xFF, 0})),
            "jpeg");
        check(DeviceDataApi.avatarType(new byte[] {'G', 'I', 'F', '8'}) == null, "gif is refused");
        check(DeviceDataApi.avatarType(null) == null, "missing image");

        check("512 B".equals(DeviceDataApi.formatBytes(512)), "bytes");
        check("12.4 MB".equals(DeviceDataApi.formatBytes(13_002_342)), "megabytes with one decimal");
        check("1.0 KB".equals(DeviceDataApi.formatBytes(1024)), "kilobytes");
        check("0 B".equals(DeviceDataApi.formatBytes(-5)), "negative is zero");

        long now = 1_800_000_000_000L;
        check("".equals(DeviceDataApi.relativeTime(now, 0)), "unknown time is blank");
        check("刚刚".equals(DeviceDataApi.relativeTime(now, now - 30_000)), "within a minute");
        check("2 分钟前".equals(DeviceDataApi.relativeTime(now, now - 150_000)), "minutes");
        check("3 小时前".equals(DeviceDataApi.relativeTime(now, now - 3 * 3_600_000L)), "hours");
        check("2 天前".equals(DeviceDataApi.relativeTime(now, now - 50 * 3_600_000L)), "days");
        check("msime-data-2026-10-05.zip".equals(DeviceDataApi.exportFileName(LocalDate.of(2026, 10, 5))),
            "export file name");
        String token = "t".repeat(64);

        // 外部数组必须在展开前受限，避免恶意账号响应让移动端按数组长度分配内存。
        check(DeviceDataApi.validResponseArrayLength(100, DeviceDataApi.MAX_SESSIONS),
            "the session limit accepts its boundary");
        check(!DeviceDataApi.validResponseArrayLength(101, DeviceDataApi.MAX_SESSIONS),
            "an oversized sessions response is refused");
        check(!DeviceDataApi.validResponseArrayLength(17, DeviceDataApi.MAX_IDENTITIES),
            "an oversized identities response is refused");
        check(!DeviceDataApi.validResponseArrayLength(17, DeviceDataApi.MAX_DATA_SECTIONS),
            "an oversized data summary is refused");
        check(DeviceDataApi.strictCount(42L) == 42L, "integer data count");
        for (Number invalid : new Number[] {1.5d, -1L}) {
            try {
                DeviceDataApi.strictCount(invalid);
                throw new AssertionError("invalid data count must be refused: " + invalid);
            } catch (CloudApi.Failure expected) {
                check(expected.status == 500 && "invalid_response".equals(expected.code),
                    "invalid data count failure");
            }
        }

        // 撤销会话走 DELETE /v1/users/me/sessions/{id}，带真实账号的令牌；不合法的 id 不发请求。
        List<String> requests = new ArrayList<>();
        List<Map<String, String>> headers = new ArrayList<>();
        CloudApi cloud = new CloudApi((method, path, sent, body) -> {
            requests.add(method + " " + path);
            headers.add(Map.copyOf(sent));
            return new CloudApi.Exchange(204, null, null, new byte[0]);
        }, rejected -> token, rejected -> "");
        DeviceDataApi api = new DeviceDataApi(cloud, rejected -> token, (path, bearer, out) -> {
            throw new AssertionError("no download expected");
        });
        api.revokeSession("abc123");
        check(requests.equals(List.of("DELETE /v1/users/me/sessions/abc123")), "revoke path");
        check(("Bearer " + token).equals(headers.get(0).get("Authorization")), "account bearer");
        try {
            api.revokeSession("../../admin");
            throw new AssertionError("an unsafe id must be refused");
        } catch (IllegalArgumentException expected) {
            check(requests.size() == 1, "no request for an unsafe id");
        }
        api.deleteAvatar();
        check("DELETE /v1/users/me/avatar".equals(requests.get(1)), "avatar delete path");
        api.deleteAccount();
        check("DELETE /v1/users/me".equals(requests.get(2)), "account delete path");

        // 导出：401 换新令牌重试一次，成功时字节原样写进输出流。
        String stale = "s".repeat(64);
        String fresh = "f".repeat(64);
        byte[] zip = "PK\u0003\u0004zip".getBytes(StandardCharsets.ISO_8859_1);
        List<String> bearers = new ArrayList<>();
        DeviceDataApi exporter = new DeviceDataApi(cloud, rejected -> rejected == null ? stale : fresh,
            (path, bearer, out) -> {
                check("/v1/users/me/data/export".equals(path), "export path");
                bearers.add(bearer);
                if (!fresh.equals(bearer)) return new DeviceDataApi.Download(401, null, new byte[0]);
                out.write(zip);
                return new DeviceDataApi.Download(200, null, new byte[0]);
            });
        ByteArrayOutputStream exported = new ByteArrayOutputStream();
        long written = exporter.exportData(exported);
        check(bearers.equals(List.of(stale, fresh)), "export retries once with a fresh token");
        check(written == zip.length && java.util.Arrays.equals(exported.toByteArray(), zip), "export bytes");

        DeviceDataApi limited = new DeviceDataApi(cloud, rejected -> token,
            (path, bearer, out) -> new DeviceDataApi.Download(429, "3600", new byte[0]));
        try {
            limited.exportData(new ByteArrayOutputStream());
            throw new AssertionError("a rate-limited export must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.status == 429 && failure.retryAfterSeconds == 3600, "rate limit and Retry-After");
        }

        DeviceDataApi signedOut = new DeviceDataApi(cloud, rejected -> "", (path, bearer, out) -> {
            throw new AssertionError("no download without a session");
        });
        try {
            signedOut.exportData(new ByteArrayOutputStream());
            throw new AssertionError("export needs a session");
        } catch (CloudApi.Failure failure) {
            check(failure.signedOut(), "signed out");
        }
        System.out.println("Android device data API passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

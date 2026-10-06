package app.msime.android;

import android.content.Context;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 水杉云端接口的公共传输层：基础地址、会话 Bearer、固定 User-Agent、JSON 与 multipart 请求体、错误解析和超时。
 *
 * <p>各功能自己的接口文件（同步、设备、反馈……）都建在这一层上，只管路径和字段。这里统一发 `MSIME/Android`；带设备型号的详细 User-Agent 只出现在 {@link BackendAccount} 的真实账号登录请求上，后端只在登录时记录它。
 *
 * <p>错误响应是 `{"error":{"code","message"}}`，读成 {@link Failure}。503 且错误码是 `provider_disabled` / `service_disabled` 表示这个部署没有开这项功能，{@link Failure#unavailable()} 为真，界面应当隐藏入口而不是提示重试。
 *
 * <p>每个请求都阻塞在网络上，不要在主线程调用。
 */
public final class CloudApi {
    public static final String ORIGIN = "https://api.msime.app";
    /** 除真实账号登录外，所有请求都只说这一句。 */
    public static final String USER_AGENT = "MSIME/Android";
    static final int CONNECT_TIMEOUT_MILLIS = 15_000;
    static final int READ_TIMEOUT_MILLIS = 30_000;
    /** 响应体上限；导出之类更大的下载由各自的接口文件流式读取，不走这里。 */
    static final int MAX_RESPONSE_BYTES = 4 * 1024 * 1024;
    private static final SecureRandom RANDOM = new SecureRandom();

    /** 请求用哪个身份：不带令牌、真实账号、设备的匿名账号，或有真实账号用真实账号、否则用匿名账号。 */
    public enum Auth { NONE, ACCOUNT, ANONYMOUS, ACCOUNT_OR_ANONYMOUS }

    /** 一次失败：HTTP 状态（网络失败为 0）、服务端错误码与原文。 */
    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        public final int status;
        public final String code;
        /** 服务端给的 `Retry-After`（秒）；没有时为 0。 */
        public final long retryAfterSeconds;

        public Failure(int status, String code, String message, long retryAfterSeconds) {
            super(message == null || message.isEmpty() ? "HTTP " + status : message);
            this.status = status;
            this.code = code == null ? "" : code;
            this.retryAfterSeconds = Math.max(0L, retryAfterSeconds);
        }

        /** 这个部署没有开这项功能。 */
        public boolean unavailable() { return featureUnavailable(status, code); }

        /** 没有可用的登录会话，或会话已被服务端拒绝。 */
        public boolean signedOut() { return status == 401; }

        /** 连不上服务器（DNS、TLS、超时等），没有拿到任何 HTTP 响应。 */
        public boolean network() { return status == 0; }
    }

    /** 一次成功的响应。 */
    public record Response(int status, String contentType, byte[] body) {
        /** 响应体按 JSON 对象读；空响应（204 之类）读成空对象。 */
        public JSONObject json() throws JSONException {
            if (body == null || body.length == 0) return new JSONObject();
            return new JSONObject(new String(body, StandardCharsets.UTF_8));
        }

        public String text() {
            return body == null ? "" : new String(body, StandardCharsets.UTF_8);
        }
    }

    /** multipart/form-data 的一段：字段名、可选的文件名、类型与内容。 */
    public record Part(String name, String filename, String contentType, byte[] content) {
        /** 一个 JSON 字段，例如反馈和语音贡献的 `payload`。 */
        public static Part json(String name, String json) {
            return new Part(name, null, "application/json", json.getBytes(StandardCharsets.UTF_8));
        }

        /** 一个纯文本字段。 */
        public static Part text(String name, String value) {
            return new Part(name, null, "text/plain; charset=utf-8", value.getBytes(StandardCharsets.UTF_8));
        }

        /** 一个文件，例如截图或录音。 */
        public static Part file(String name, String filename, String contentType, byte[] content) {
            return new Part(name, filename, contentType, content);
        }
    }

    /** 已编码好的请求体与它的 `Content-Type`。 */
    public record Body(String contentType, byte[] bytes) {
        public static Body json(JSONObject value) {
            return new Body("application/json", value.toString().getBytes(StandardCharsets.UTF_8));
        }

        public static Body multipart(List<Part> parts) {
            String boundary = newBoundary();
            return new Body(multipartContentType(boundary), encodeMultipart(boundary, parts));
        }
    }

    /** 服务端在 `GET /v1/auth/providers` 里说它接受哪些登录方式。 */
    public record Providers(boolean google, boolean apple, boolean appleWeb, boolean email) {
        public static final Providers NONE = new Providers(false, false, false, false);
    }

    /** 真正发请求的那一层；冒烟测试换成内存实现。 */
    public interface Transport {
        Exchange exchange(String method, String path, Map<String, String> headers, byte[] body) throws IOException;
    }

    /** 一次往返的原始结果，不论状态码。 */
    public record Exchange(int status, String contentType, String retryAfter, byte[] body) {}

    /** 取令牌：参数是刚被服务端拒绝的令牌（首次为 null），返回空字符串表示没有登录。 */
    public interface Tokens {
        String token(String rejected) throws Exception;
    }

    private final Transport transport;
    private final Tokens account;
    private final Tokens anonymous;

    public CloudApi(Context context) {
        Context application = context.getApplicationContext();
        this.transport = CloudApi::httpExchange;
        this.account = rejected -> new BackendAccount(application).currentAccessToken(rejected);
        this.anonymous = rejected -> new BackendAnonymousAccount(application).accessToken(rejected);
    }

    public CloudApi(Transport transport, Tokens account, Tokens anonymous) {
        this.transport = transport;
        this.account = account;
        this.anonymous = anonymous;
    }

    /** 当前部署接受的登录方式。读不到时抛出，调用方按「都不提供」处理。 */
    public Providers providers() throws Failure {
        JSONObject root = json("GET", "/v1/auth/providers", null, Auth.NONE);
        JSONObject map = root.optJSONObject("providers");
        if (map == null) throw new Failure(500, "invalid_response", "providers missing", 0);
        return new Providers(strictTrue(map.opt("google")), strictTrue(map.opt("apple")),
            strictTrue(map.opt("apple_web")), strictTrue(map.opt("email")));
    }

    /** 发一个 JSON 请求（`body` 可为 null）并把响应读成 JSON 对象。 */
    public JSONObject json(String method, String path, JSONObject body, Auth auth) throws Failure {
        Response response = send(method, path, body == null ? null : Body.json(body), auth);
        try {
            return response.json();
        } catch (JSONException malformed) {
            throw new Failure(response.status(), "invalid_response", "malformed JSON response", 0);
        }
    }

    /** 发一个 multipart/form-data 请求并把响应读成 JSON 对象。 */
    public JSONObject multipart(String path, List<Part> parts, Auth auth) throws Failure {
        Response response = send("POST", path, Body.multipart(parts), auth);
        try {
            return response.json();
        } catch (JSONException malformed) {
            throw new Failure(response.status(), "invalid_response", "malformed JSON response", 0);
        }
    }

    /**
     * 发一个请求，2xx 返回响应，其余抛 {@link Failure}。
     *
     * <p>带令牌的请求被 401 拒绝时，向令牌来源要一枚新的（同一个被拒的令牌不会再给回来）并只重试一次。
     */
    public Response send(String method, String path, Body body, Auth auth) throws Failure {
        if (path == null || !path.startsWith("/")) throw new IllegalArgumentException("path must be absolute");
        Credential credential = credential(auth, null);
        for (int attempt = 0; ; attempt++) {
            Map<String, String> headers = new LinkedHashMap<>(4);
            headers.put("Accept", "application/json");
            headers.put("User-Agent", USER_AGENT);
            if (credential.token() != null) headers.put("Authorization", "Bearer " + credential.token());
            if (body != null) headers.put("Content-Type", body.contentType());
            Exchange exchange;
            try {
                exchange = transport.exchange(method, path, headers, body == null ? null : body.bytes());
            } catch (IOException offline) {
                throw new Failure(0, "network", offline.getMessage(), 0);
            }
            if (exchange.status() / 100 == 2) {
                return new Response(exchange.status(), exchange.contentType(), exchange.body());
            }
            if (exchange.status() == 401 && credential.token() != null && attempt == 0) {
                Credential fresh = credential(credential.auth(), credential.token());
                if (fresh.token() != null && !fresh.token().equals(credential.token())) {
                    credential = fresh;
                    continue;
                }
            }
            throw failure(exchange);
        }
    }

    private record Credential(Auth auth, String token) {}

    /** 按身份取令牌；`ACCOUNT_OR_ANONYMOUS` 在没有真实账号时落到匿名账号，重试时沿用第一次选中的那一种。 */
    private Credential credential(Auth auth, String rejected) throws Failure {
        try {
            switch (auth) {
                case NONE:
                    return new Credential(Auth.NONE, null);
                case ACCOUNT: {
                    String token = account.token(rejected);
                    if (token == null || token.isEmpty()) throw new Failure(401, "signed_out", "not signed in", 0);
                    return new Credential(Auth.ACCOUNT, token);
                }
                case ANONYMOUS: {
                    String token = anonymous.token(rejected);
                    if (token == null || token.isEmpty()) throw new Failure(401, "signed_out", "no anonymous session", 0);
                    return new Credential(Auth.ANONYMOUS, token);
                }
                default: {
                    String token = account.token(rejected);
                    if (token != null && !token.isEmpty()) return new Credential(Auth.ACCOUNT, token);
                    return credential(Auth.ANONYMOUS, rejected);
                }
            }
        } catch (Failure failure) {
            throw failure;
        } catch (Exception unavailable) {
            throw new Failure(0, "session_unavailable", unavailable.getMessage(), 0);
        }
    }

    /** 503 加上这两个错误码之一，说明这个部署没有开这项功能，而不是暂时出错。 */
    public static boolean featureUnavailable(int status, String code) {
        return status == 503 && ("provider_disabled".equals(code) || "service_disabled".equals(code));
    }

    /** 把一次非 2xx 的往返读成失败：错误码和原文来自 `{"error":{"code","message"}}`，读不出时为空。 */
    static Failure failure(Exchange exchange) {
        String code = "";
        String message = "";
        byte[] raw = exchange.body();
        if (raw != null && raw.length > 0) {
            try {
                JSONObject error = new JSONObject(new String(raw, StandardCharsets.UTF_8)).optJSONObject("error");
                if (error != null) {
                    Object rawCode = error.opt("code");
                    Object rawMessage = error.opt("message");
                    if (rawCode instanceof String value) code = value;
                    if (rawMessage instanceof String value) message = value;
                }
            } catch (JSONException notJson) {
                code = "";
            }
        }
        return new Failure(exchange.status(), code, message, retryAfterSeconds(exchange.retryAfter()));
    }

    /** `Retry-After` 的秒数形式；日期形式和读不出的值都当作没有。 */
    public static long retryAfterSeconds(String header) {
        if (header == null) return 0L;
        String value = header.trim();
        if (value.isEmpty() || value.length() > 9) return 0L;
        for (int index = 0; index < value.length(); index++) {
            if (value.charAt(index) < '0' || value.charAt(index) > '9') return 0L;
        }
        return Long.parseLong(value);
    }

    /** JSON 布尔值只有严格的 `true` 才算；字符串和数字都不算。 */
    static boolean strictTrue(Object value) {
        return Boolean.TRUE.equals(value);
    }

    // ---- multipart ----

    /** 一个新的随机分隔串：24 字节随机数的十六进制，前面加固定前缀。 */
    static String newBoundary() {
        byte[] bytes = new byte[24];
        RANDOM.nextBytes(bytes);
        StringBuilder value = new StringBuilder("msime-");
        for (byte raw : bytes) {
            value.append(Character.forDigit((raw >>> 4) & 0x0F, 16)).append(Character.forDigit(raw & 0x0F, 16));
        }
        return value.toString();
    }

    public static String multipartContentType(String boundary) {
        return "multipart/form-data; boundary=" + requireBoundary(boundary);
    }

    /**
     * 按 RFC 7578 编码 multipart/form-data：每段 `--boundary`、`Content-Disposition`（文件段带 filename）、`Content-Type`、空行、内容，最后 `--boundary--`，行尾一律 CRLF。
     *
     * <p>字段名和文件名里不允许出现引号、反斜杠和控制字符：它们是本应用自己写的固定值，出现了就是调用方的错误。内容里出现分隔串同样拒绝。
     */
    public static byte[] encodeMultipart(String boundary, List<Part> parts) {
        requireBoundary(boundary);
        if (parts == null || parts.isEmpty()) throw new IllegalArgumentException("multipart needs at least one part");
        byte[] delimiter = ("--" + boundary).getBytes(StandardCharsets.US_ASCII);
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        for (Part part : parts) {
            if (part == null || part.content() == null) throw new IllegalArgumentException("empty multipart part");
            if (contains(part.content(), delimiter)) throw new IllegalArgumentException("part contains the boundary");
            String type = part.contentType() == null ? "application/octet-stream" : part.contentType();
            StringBuilder head = new StringBuilder(96 + boundary.length()
                + headerLength(part.name()) + headerLength(part.filename()) + headerLength(type));
            head.append("--").append(boundary).append("\r\n");
            head.append("Content-Disposition: form-data; name=\"").append(headerToken(part.name())).append('"');
            if (part.filename() != null) head.append("; filename=\"").append(headerToken(part.filename())).append('"');
            head.append("\r\n");
            head.append("Content-Type: ").append(headerToken(type)).append("\r\n\r\n");
            write(output, head.toString().getBytes(StandardCharsets.UTF_8));
            write(output, part.content());
            write(output, "\r\n".getBytes(StandardCharsets.US_ASCII));
        }
        write(output, ("--" + boundary + "--\r\n").getBytes(StandardCharsets.US_ASCII));
        return output.toByteArray();
    }

    /** `ByteArrayOutputStream.writeBytes` 要到 API 33 才有，最低支持的版本是 28。 */
    private static void write(ByteArrayOutputStream output, byte[] bytes) {
        output.write(bytes, 0, bytes.length);
    }

    private static String requireBoundary(String boundary) {
        if (boundary == null || boundary.isEmpty() || boundary.length() > 70) {
            throw new IllegalArgumentException("invalid multipart boundary");
        }
        for (int index = 0; index < boundary.length(); index++) {
            char c = boundary.charAt(index);
            boolean allowed = c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= '0' && c <= '9' || c == '-' || c == '_';
            if (!allowed) throw new IllegalArgumentException("invalid multipart boundary");
        }
        return boundary;
    }

    private static int headerLength(String value) {
        return value == null ? 0 : value.length();
    }

    private static String headerToken(String value) {
        if (value == null || value.isEmpty()) throw new IllegalArgumentException("empty multipart header value");
        for (int index = 0; index < value.length(); index++) {
            char c = value.charAt(index);
            if (c < 0x20 || c == 0x7F || c == '"' || c == '\\') {
                throw new IllegalArgumentException("invalid multipart header value");
            }
        }
        return value;
    }

    private static boolean contains(byte[] haystack, byte[] needle) {
        outer:
        for (int start = 0; start + needle.length <= haystack.length; start++) {
            for (int offset = 0; offset < needle.length; offset++) {
                if (haystack[start + offset] != needle[offset]) continue outer;
            }
            return true;
        }
        return false;
    }

    // ---- HTTP ----

    private static Exchange httpExchange(String method, String path, Map<String, String> headers, byte[] body)
            throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(ORIGIN + path).openConnection();
        try {
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod(method);
            connection.setConnectTimeout(CONNECT_TIMEOUT_MILLIS);
            connection.setReadTimeout(READ_TIMEOUT_MILLIS);
            for (Map.Entry<String, String> header : headers.entrySet()) {
                connection.setRequestProperty(header.getKey(), header.getValue());
            }
            if (body != null) {
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(body.length);
                try (OutputStream output = connection.getOutputStream()) { output.write(body); }
            }
            int status = connection.getResponseCode();
            InputStream stream = status / 100 == 2 ? connection.getInputStream() : connection.getErrorStream();
            byte[] response = new byte[0];
            if (stream != null) {
                try (InputStream input = stream) {
                    response = HttpBodyPolicy.readRequired(input, MAX_RESPONSE_BYTES);
                }
            }
            return new Exchange(status, connection.getContentType(), connection.getHeaderField("Retry-After"), response);
        } finally {
            connection.disconnect();
        }
    }

}

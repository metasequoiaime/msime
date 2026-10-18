package app.msime.android;

import android.content.Context;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 水杉云端接口在 Java 里还剩的那一层传输：基础地址、会话 Bearer、固定 User-Agent、JSON 请求体、错误解析和超时。
 *
 * <p>账号、同步、反馈、诊断、社区、对话等请求都已经由 Rust 发出（#8201 迁移计划第 5 步），建在这一层上的只剩云剪贴板的端到端加密驱动（{@link CloudClipboardApi}）和打字统计、QQ 绑定（{@link TypingUploadApi}），各自只管路径和字段。这里统一发 `MSIME/Android`；带设备型号的详细 User-Agent 由 Rust 在登录和刷新时发出（{@link AccountStore}），后端只在登录时记录它。令牌也由 Rust 给：登录账号的经 {@link BackendAccount}，匿名的经 {@link BackendAnonymousAccount}。
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
    /** 响应体上限。 */
    static final int MAX_RESPONSE_BYTES = 4 * 1024 * 1024;

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
            this.retryAfterSeconds = BoundsPolicy.nonNegative(retryAfterSeconds);
        }

        /** 这个部署没有开这项功能。 */
        public boolean unavailable() { return featureUnavailable(status, code); }

        /** 没有可用的登录会话，或会话已被服务端拒绝。 */
        public boolean signedOut() { return status == 401; }

        /** 连不上服务器（DNS、TLS、超时等），没有拿到任何 HTTP 响应。 */
        public boolean network() { return status == 0; }

        /** 服务端要求调用方稍后再试。 */
        public boolean rateLimited() { return status == 429; }
    }

    /** 一次成功的响应。 */
    public record Response(int status, String contentType, byte[] body) {
        /** 响应体按 JSON 对象读；空响应（204 之类）读成空对象。 */
        public JSONObject json() throws JSONException {
            if (body == null || body.length == 0) return new JSONObject();
            return new JSONObject(TextPolicy.utf8(body));
        }

        public String text() {
            return TextPolicy.utf8(body);
        }
    }

    /** 已编码好的请求体与它的 `Content-Type`。 */
    public record Body(String contentType, byte[] bytes) {
        public static Body json(JSONObject value) {
            return new Body("application/json", TextPolicy.utf8Bytes(value.toString()));
        }
    }

    /** 服务端接受哪些登录方式（由 Rust 的 providers 读出，见 {@link BackendAccount#providers}）。 */
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

        /** Stable login identity, when this source represents a signed-in account. */
        default String sessionId() throws Exception { return null; }

        /** Obtain a token and its login identity from one session snapshot. */
        default TokenSnapshot snapshot(String rejected) throws Exception {
            String before = sessionId();
            String value = token(rejected);
            String after = sessionId();
            if (!Objects.equals(before, after)) throw new java.util.concurrent.CancellationException("account session changed");
            return new TokenSnapshot(value, after);
        }
    }

    public record TokenSnapshot(String token, String sessionId) {}

    private final Transport transport;
    private final Tokens account;
    private final Tokens anonymous;

    public CloudApi(Context context) {
        Context application = context.getApplicationContext();
        this.transport = CloudApi::httpExchange;
        this.account = accountTokens(application);
        this.anonymous = rejected -> new BackendAnonymousAccount(application).accessToken(rejected);
    }

    public CloudApi(Transport transport, Tokens account, Tokens anonymous) {
        this.transport = transport;
        this.account = account;
        this.anonymous = anonymous;
    }

    /** Identity of the currently signed-in account, captured on the calling worker thread. */
    public String currentAccountSessionId() throws Failure {
        String sessionId = credential(Auth.ACCOUNT, null).sessionId();
        if (sessionId == null || sessionId.isEmpty()) throw sessionChanged();
        return sessionId;
    }

    /** Keep every request and 401 retry on the login that supplied a settings page. */
    public CloudApi forAccountSession(String expectedSessionId) {
        if (expectedSessionId == null || expectedSessionId.isEmpty())
            throw new IllegalArgumentException("account session id required");
        Tokens bound = new Tokens() {
            @Override public String token(String rejected) throws Exception {
                return snapshot(rejected).token();
            }

            @Override public TokenSnapshot snapshot(String rejected) throws Exception {
                TokenSnapshot current = account.snapshot(rejected);
                if (!expectedSessionId.equals(current.sessionId()))
                    throw new java.util.concurrent.CancellationException("account session changed");
                return current;
            }
        };
        return new CloudApi(transport, bound, anonymous);
    }

    static Tokens accountTokens(Context application) {
        return accountTokens(application, null);
    }

    static Tokens accountTokens(Context application, String expectedSessionId) {
        return new Tokens() {
            @Override public String token(String rejected) throws Exception {
                return new BackendAccount(application).currentAccessToken(rejected);
            }

            @Override public TokenSnapshot snapshot(String rejected) throws Exception {
                BackendAccount.SessionCredential session = new BackendAccount(application).currentSession(rejected);
                TokenSnapshot result = new TokenSnapshot(session.token(), session.sessionId());
                if (expectedSessionId != null && !expectedSessionId.equals(result.sessionId()))
                    throw new java.util.concurrent.CancellationException("account session changed");
                return result;
            }
        };
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

    /**
     * 发一个请求，2xx 返回响应，其余抛 {@link Failure}。
     *
     * <p>带令牌的请求被 401 拒绝时，向令牌来源要一枚新的（同一个被拒的令牌不会再给回来）并只重试一次。
     */
    public Response send(String method, String path, Body body, Auth auth) throws Failure {
        Exchange exchange = exchange(method, path, body, auth);
        if (exchange.status() / 100 == 2) {
            return new Response(exchange.status(), exchange.contentType(), exchange.body());
        }
        throw failure(exchange);
    }

    /**
     * 发一个请求，不论状态码都把这次往返交回，给要自己读错误响应的调用方（云剪贴板的端到端驱动，见 {@link CloudClipboardE2e}）。
     *
     * <p>令牌、401 换令牌重试一次、登录变化的检查都与 {@link #send} 相同；连不上服务器仍抛状态为 0 的 {@link Failure}。
     */
    public Exchange exchange(String method, String path, Body body, Auth auth) throws Failure {
        if (path == null || !path.startsWith("/") || path.indexOf('\\') >= 0 || containsDotSegment(path)) {
            throw new IllegalArgumentException("path must be a safe absolute API path");
        }
        Credential credential = credential(auth, null);
        for (int attempt = 0; ; attempt++) {
            ensureCurrentLogin(credential);
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
            ensureCurrentLogin(credential);
            if (exchange.status() == 401 && credential.token() != null && attempt == 0) {
                Credential fresh = credential(credential.auth(), credential.token());
                if (credential.sessionId() != null
                        && !credential.sessionId().equals(fresh.sessionId())) throw sessionChanged();
                if (fresh.token() != null && !fresh.token().equals(credential.token())) {
                    credential = fresh;
                    continue;
                }
            }
            return exchange;
        }
    }

    private record Credential(Auth auth, String token, String sessionId) {}

    private void ensureCurrentLogin(Credential expected) throws Failure {
        if (expected.auth() != Auth.ACCOUNT || expected.sessionId() == null) return;
        try {
            TokenSnapshot now = account.snapshot(null);
            if (!expected.sessionId().equals(now.sessionId())) throw sessionChanged();
        } catch (Failure failure) {
            throw failure;
        } catch (java.util.concurrent.CancellationException changed) {
            throw sessionChanged();
        } catch (Exception unavailable) {
            throw new Failure(0, "session_unavailable", unavailable.getMessage(), 0);
        }
    }

    private static Failure sessionChanged() {
        return new Failure(409, "session_changed", "account session changed", 0);
    }

    private static boolean containsDotSegment(String path) {
        final String decoded;
        try {
            decoded = java.net.URLDecoder.decode(path, StandardCharsets.UTF_8.name());
        } catch (java.io.UnsupportedEncodingException impossible) {
            return true;
        } catch (IllegalArgumentException malformed) {
            return true;
        }
        for (String segment : decoded.split("/", -1)) {
            if (".".equals(segment) || "..".equals(segment)) return true;
        }
        return false;
    }

    /** 按身份取令牌；`ACCOUNT_OR_ANONYMOUS` 在没有真实账号时落到匿名账号，重试时沿用第一次选中的那一种。 */
    private Credential credential(Auth auth, String rejected) throws Failure {
        try {
            switch (auth) {
                case NONE:
                    return new Credential(Auth.NONE, null, null);
                case ACCOUNT: {
                    TokenSnapshot answer = account.snapshot(rejected);
                    if (answer.token() == null || answer.token().isEmpty())
                        throw new Failure(401, "signed_out", "not signed in", 0);
                    return new Credential(Auth.ACCOUNT, answer.token(), answer.sessionId());
                }
                case ANONYMOUS: {
                    TokenSnapshot answer = anonymous.snapshot(rejected);
                    if (answer.token() == null || answer.token().isEmpty())
                        throw new Failure(401, "signed_out", "no anonymous session", 0);
                    return new Credential(Auth.ANONYMOUS, answer.token(), answer.sessionId());
                }
                default: {
                    TokenSnapshot answer = account.snapshot(rejected);
                    if (answer.token() != null && !answer.token().isEmpty())
                        return new Credential(Auth.ACCOUNT, answer.token(), answer.sessionId());
                    return credential(Auth.ANONYMOUS, rejected);
                }
            }
        } catch (Failure failure) {
            throw failure;
        } catch (java.util.concurrent.CancellationException changed) {
            throw sessionChanged();
        } catch (BackendAnonymousAccount.RateLimited limited) {
            throw new Failure(429, "", limited.getMessage(), 0);
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
                JSONObject error = new JSONObject(TextPolicy.utf8(raw)).optJSONObject("error");
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
        String value = TextPolicy.trimmed(header);
        if (value.isEmpty() || value.length() > 9) return 0L;
        for (int index = 0; index < value.length(); index++) {
            if (!TextPolicy.isAsciiDigit(value.charAt(index))) return 0L;
        }
        return Long.parseLong(value);
    }

    // ---- HTTP ----

    private static Exchange httpExchange(String method, String path, Map<String, String> headers, byte[] body)
            throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(ORIGIN + path).openConnection();
        try {
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod(method);
            HttpConnectionPolicy.setTimeouts(connection, CONNECT_TIMEOUT_MILLIS, READ_TIMEOUT_MILLIS);
            for (Map.Entry<String, String> header : headers.entrySet()) {
                connection.setRequestProperty(header.getKey(), header.getValue());
            }
            if (body != null) {
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(body.length);
                try (OutputStream output = connection.getOutputStream()) { output.write(body); }
            }
            int status = connection.getResponseCode();
            InputStream stream = HttpConnectionPolicy.isSuccessful(status) ? connection.getInputStream() : connection.getErrorStream();
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

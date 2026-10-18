import app.msime.android.CloudApi;
import app.msime.android.CloudClipboardApi;
import app.msime.android.JsonPolicy;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

public final class CloudApiSmoke {
    public static void main(String[] arguments) throws Exception {
        check(Boolean.TRUE.equals(JsonPolicy.strictBoolean(Boolean.TRUE)),
            "cloud clipboard booleans accept JSON booleans");
        check(JsonPolicy.strictBoolean("true") == null,
            "cloud clipboard booleans must reject strings instead of coercing them");
        check(JsonPolicy.strictBoolean(1) == null,
            "cloud clipboard booleans must reject numbers instead of coercing them");
        check("synthetic".equals(JsonPolicy.strictString("synthetic")),
            "cloud clipboard strings accept JSON strings");
        check(JsonPolicy.strictString(7) == null,
            "cloud clipboard strings must reject numbers instead of coercing them");
        check(Integer.valueOf(7).equals(JsonPolicy.strictInteger(Integer.valueOf(7))),
            "cloud clipboard retention accepts JSON integers");
        check(JsonPolicy.strictInteger("7") == null,
            "cloud clipboard retention rejects numeric strings instead of coercing them");
        check(JsonPolicy.strictInteger(Double.valueOf(7.5)) == null,
            "cloud clipboard retention rejects fractional numbers instead of truncating them");
        // 503 + provider_disabled / service_disabled 是功能未开，不是暂时故障。
        check(CloudApi.featureUnavailable(503, "provider_disabled"), "provider_disabled is unavailable");
        check(CloudApi.featureUnavailable(503, "service_disabled"), "service_disabled is unavailable");
        check(!CloudApi.featureUnavailable(503, "server_busy"), "server_busy is a retry");
        check(!CloudApi.featureUnavailable(500, "service_disabled"), "only 503 counts");
        check(new CloudApi.Failure(503, "service_disabled", "", 0).unavailable(), "failure reports unavailable");
        check(new CloudApi.Failure(0, "network", null, 0).network(), "status 0 is a network failure");
        check(CloudApi.retryAfterSeconds(" 120 ") == 120, "Retry-After seconds");
        check(CloudApi.retryAfterSeconds("Wed, 21 Oct 2015 07:28:00 GMT") == 0, "Retry-After date ignored");
        check(CloudApi.retryAfterSeconds(null) == 0, "missing Retry-After");

        // 401 时换一枚新令牌只重试一次；User-Agent 固定为 MSIME/Android。
        String stale = "a".repeat(64);
        String fresh = "b".repeat(64);
        List<Map<String, String>> seen = new ArrayList<>();
        CloudApi api = new CloudApi((method, path, headers, body) -> {
            seen.add(Map.copyOf(headers));
            boolean ok = ("Bearer " + fresh).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(ok ? 204 : 401, null, null, new byte[0]);
        }, rejected -> rejected == null ? stale : fresh, rejected -> "");
        CloudApi.Response response = api.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
        check(response.status() == 204 && seen.size() == 2, "a rejected token is refreshed and retried once");
        check("MSIME/Android".equals(seen.get(0).get("User-Agent")), "plain User-Agent on every request");

        AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
        CloudApi.Tokens changing = new CloudApi.Tokens() {
            @Override public String token(String rejected) { return "synthetic-login-a".equals(login.get()) ? stale : fresh; }
            public String sessionId() { return login.get(); }
        };
        AtomicInteger oldCalls = new AtomicInteger();
        CloudApi oldRequest = new CloudApi((method, path, headers, body) -> {
            oldCalls.incrementAndGet();
            login.set("synthetic-login-b");
            return new CloudApi.Exchange(401, null, null, new byte[0]);
        }, changing, rejected -> "");
        try {
            oldRequest.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("an old request must not retry after a new login");
        } catch (CloudApi.Failure failure) {
            check("session_changed".equals(failure.code), "new login cancels a rejected old request");
            check(oldCalls.get() == 1, "new login receives no old request retry");
        }
        login.set("synthetic-login-a");
        CloudApi successfulOldRequest = new CloudApi((method, path, headers, body) -> {
            login.set("synthetic-login-b");
            return new CloudApi.Exchange(200, null, null, new byte[0]);
        }, changing, rejected -> "");
        try {
            successfulOldRequest.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("an old response must not survive a new login");
        } catch (CloudApi.Failure failure) {
            check("session_changed".equals(failure.code), "new login discards a successful old response");
        }
        AtomicReference<String> sameTokenLogin = new AtomicReference<>("synthetic-login-a");
        CloudApi.Tokens reusedToken = new CloudApi.Tokens() {
            @Override public String token(String rejected) { return stale; }
            @Override public String sessionId() { return sameTokenLogin.get(); }
        };
        CloudApi sameTokenRequest = new CloudApi((method, path, headers, body) -> {
            sameTokenLogin.set("synthetic-login-b");
            return new CloudApi.Exchange(200, null, null, new byte[0]);
        }, reusedToken, rejected -> "");
        try {
            sameTokenRequest.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("login identity must be checked even when tokens match");
        } catch (CloudApi.Failure failure) {
            check("session_changed".equals(failure.code), "new login with the same token is cancelled");
        }
        AtomicReference<String> rotatingToken = new AtomicReference<>(stale);
        CloudApi.Tokens rotatingLogin = new CloudApi.Tokens() {
            @Override public String token(String rejected) {
                if (rejected != null) rotatingToken.set(fresh);
                return rotatingToken.get();
            }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        CloudApi sameLoginRetry = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(("Bearer " + fresh).equals(headers.get("Authorization")) ? 204 : 401,
                null, null, new byte[0]), rotatingLogin, rejected -> "");
        check(sameLoginRetry.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT).status() == 204,
            "a token refresh in the same login still retries");

        // 云剪贴板页的端到端驱动仍经 CloudApi 绑定到打开页面时的登录：换了号以后旧页面的请求和复制都被拒绝（开关、清空等不涉及内容的操作改经 Rust，见 FeedbackApiSmoke 和设备冒烟）。
        AtomicReference<String> clipboardLogin = new AtomicReference<>("synthetic-login-a");
        AtomicInteger clipboardDeletes = new AtomicInteger();
        CloudApi.Tokens clipboardTokens = new CloudApi.Tokens() {
            @Override public String token(String rejected) { return stale; }
            @Override public String sessionId() { return clipboardLogin.get(); }
        };
        CloudApi clipboardTransport = new CloudApi((method, path, headers, body) -> {
            if ("DELETE".equals(method)) clipboardDeletes.incrementAndGet();
            return new CloudApi.Exchange(204, null, null, new byte[0]);
        }, clipboardTokens, rejected -> "");
        CloudApi boundClipboard = clipboardTransport.forAccountSession(
            clipboardTransport.currentAccountSessionId());
        boundClipboard.send("GET", CloudClipboardApi.PATH, null, CloudApi.Auth.ACCOUNT);
        clipboardLogin.set("synthetic-login-b");
        try {
            boundClipboard.currentAccountSessionId();
            throw new AssertionError("an old clipboard row must not be copied after a new login");
        } catch (CloudApi.Failure failure) {
            check("session_changed".equals(failure.code), "old clipboard row rejects a new login");
        }
        try {
            boundClipboard.exchange("DELETE", CloudClipboardApi.PATH, null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("an old clipboard page must not reach the new account");
        } catch (CloudApi.Failure failure) {
            check("session_changed".equals(failure.code), "old clipboard page rejects a new login");
            check(clipboardDeletes.get() == 0, "the new account receives no old-page request");
        }

        // 端到端驱动要自己读错误响应：exchange 不论状态码都交回，401 仍先换令牌重试一次，连不上仍抛出。
        byte[] conflict = "{\"error\":{\"code\":\"clipboard_key_conflict\"}}".getBytes(StandardCharsets.UTF_8);
        CloudApi.Exchange refused = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(409, "application/json", null, conflict), rejected -> stale, rejected -> "")
            .exchange("POST", CloudClipboardApi.PATH, null, CloudApi.Auth.ACCOUNT);
        check(refused.status() == 409 && java.util.Arrays.equals(refused.body(), conflict),
            "exchange hands a non-2xx status and its body back instead of throwing");
        List<String> exchangeTokens = new ArrayList<>();
        CloudApi.Exchange retried = new CloudApi((method, path, headers, body) -> {
            exchangeTokens.add(headers.get("Authorization"));
            boolean ok = ("Bearer " + fresh).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(ok ? 200 : 401, null, null, new byte[0]);
        }, rejected -> rejected == null ? stale : fresh, rejected -> "")
            .exchange("GET", CloudClipboardApi.PATH, null, CloudApi.Auth.ACCOUNT);
        check(retried.status() == 200 && exchangeTokens.size() == 2, "exchange still retries a rejected token once");
        CloudApi.Exchange stillDenied = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(401, null, null, new byte[0]), rejected -> stale, rejected -> "")
            .exchange("GET", CloudClipboardApi.PATH, null, CloudApi.Auth.ACCOUNT);
        check(stillDenied.status() == 401, "a repeated 401 is handed back to the driver");
        try {
            new CloudApi((method, path, headers, body) -> {
                throw new java.io.IOException("offline");
            }, rejected -> stale, rejected -> "").exchange("GET", CloudClipboardApi.PATH, null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("an offline exchange must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.network(), "an offline exchange is a transport failure, not a status for the driver");
        }

        CloudApi denied = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(401, null, null, new byte[0]), rejected -> stale, rejected -> "");
        try {
            denied.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("a repeated 401 must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.signedOut(), "a repeated 401 reads as signed out");
        }
        CloudApi signedOut = new CloudApi((method, path, headers, body) -> {
            throw new AssertionError("no request without a session");
        }, rejected -> "", rejected -> "");
        try {
            signedOut.send("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
            throw new AssertionError("an account request without a session must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.status == 401 && "signed_out".equals(failure.code), "signed out before any request");
        }
        List<String> anonymousAuth = new ArrayList<>();
        new CloudApi((method, path, headers, body) -> {
            anonymousAuth.add(headers.get("Authorization"));
            return new CloudApi.Exchange(200, null, null, new byte[0]);
        }, rejected -> "", rejected -> fresh).send("GET", "/v1/community/resources", null,
            CloudApi.Auth.ACCOUNT_OR_ANONYMOUS);
        check(("Bearer " + fresh).equals(anonymousAuth.get(0)), "falls back to the anonymous identity");
        List<String> none = new ArrayList<>();
        new CloudApi((method, path, headers, body) -> {
            none.add(String.valueOf(headers.get("Authorization")));
            return new CloudApi.Exchange(200, null, null, new byte[0]);
        }, rejected -> stale, rejected -> stale).send("GET", "/v1/auth/providers", null, CloudApi.Auth.NONE);
        check("null".equals(none.get(0)), "an unauthenticated request carries no token");
        try {
            new CloudApi((method, path, headers, body) -> {
                throw new java.io.IOException("offline");
            }, rejected -> "", rejected -> "").send("GET", "/v1/auth/providers", null, CloudApi.Auth.NONE);
            throw new AssertionError("an offline request must fail");
        } catch (CloudApi.Failure failure) {
            check(failure.network(), "an IOException is a network failure");
        }
        CloudApi pathApi = new CloudApi((method, path, headers, body) -> {
            throw new AssertionError("unsafe path reached transport: " + path);
        }, rejected -> "", rejected -> "");
        for (String path : List.of("/v1/../auth/logout", "/v1/users/../auth/logout", "/v1/%2e%2e/auth/logout")) {
            try {
                pathApi.send("GET", path, null, CloudApi.Auth.NONE);
                throw new AssertionError("dot-segment path must be rejected: " + path);
            } catch (IllegalArgumentException unsafe) { }
        }
        System.out.println("Android cloud API transport passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

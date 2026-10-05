import app.msime.android.CloudApi;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

public final class CloudApiSmoke {
    public static void main(String[] arguments) throws Exception {
        // multipart 按 RFC 7578 编码，行尾 CRLF，文件段带 filename，结尾 `--boundary--`。
        byte[] encoded = CloudApi.encodeMultipart("b0und", List.of(
            CloudApi.Part.json("payload", "{\"type\":\"bug\"}"),
            CloudApi.Part.file("screenshots", "1.png", "image/png", new byte[] {1, 2})));
        byte[] expected = concat(
            ("--b0und\r\nContent-Disposition: form-data; name=\"payload\"\r\nContent-Type: application/json\r\n\r\n"
                + "{\"type\":\"bug\"}\r\n--b0und\r\nContent-Disposition: form-data; name=\"screenshots\"; filename=\"1.png\"\r\n"
                + "Content-Type: image/png\r\n\r\n").getBytes(StandardCharsets.UTF_8),
            new byte[] {1, 2}, "\r\n--b0und--\r\n".getBytes(StandardCharsets.US_ASCII));
        check(java.util.Arrays.equals(encoded, expected), "multipart body must match RFC 7578 layout");
        check("multipart/form-data; boundary=b0und".equals(CloudApi.multipartContentType("b0und")),
            "content type names the boundary");
        rejects(() -> CloudApi.encodeMultipart("b0und", List.of(CloudApi.Part.text("a\"b", "x"))),
            "a quote in a field name is refused");
        rejects(() -> CloudApi.encodeMultipart("b0und", List.of(CloudApi.Part.text("a", "x--b0undy"))),
            "content containing the boundary is refused");
        rejects(() -> CloudApi.encodeMultipart("bad boundary", List.of(CloudApi.Part.text("a", "x"))),
            "a boundary with a space is refused");
        rejects(() -> CloudApi.encodeMultipart("b0und", List.of()), "an empty multipart body is refused");
        CloudApi.Body random = CloudApi.Body.multipart(List.of(CloudApi.Part.text("a", "x")));
        check(random.contentType().startsWith("multipart/form-data; boundary=msime-"), "random boundary prefix");

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
        System.out.println("Android cloud API transport passed");
    }

    private static byte[] concat(byte[]... parts) {
        java.io.ByteArrayOutputStream output = new java.io.ByteArrayOutputStream();
        for (byte[] part : parts) output.write(part, 0, part.length);
        return output.toByteArray();
    }

    private static void rejects(Runnable action, String message) {
        try {
            action.run();
        } catch (IllegalArgumentException expected) {
            return;
        }
        throw new AssertionError(message);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

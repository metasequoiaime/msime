package app.msime.android;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

/** 「水杉账号」候选释义的传输一半（#8201 迁移计划 5e）：请求和解读交给 Rust（这里换成内存实现），令牌只取登录优先、否则匿名的那一枚，401 时换一枚、换到不同的才重试一次，状态码、响应体和 Retry-After 原样交给 Rust 解读。 */
public final class BackendTranslationClientSmoke {
    private static final byte[] PLANNED = "{\"texts\":[\"合成词\"]}".getBytes(java.nio.charset.StandardCharsets.UTF_8);

    public static void main(String[] args) throws Exception {
        sendsRustsRequestWithTheSharedToken();
        retriesOnceWithAFreshToken();
        doesNotRetryWithoutANewToken();
        retriesOnlyOnce();
        validatesBeforeAskingForAToken();
        handsFailuresToRust();
        keepsTheResponseBound();
        System.out.println("backend translation client passed");
    }

    private static void sendsRustsRequestWithTheSharedToken() throws Exception {
        FakeGloss gloss = new FakeGloss();
        Calls calls = new Calls();
        List<String> rejected = new ArrayList<>();
        BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) -> {
            calls.record(method, path, headers, body);
            return new CloudApi.Exchange(200, "application/json", null, "answer".getBytes());
        }, previous -> { rejected.add(previous); return "token-a"; });
        List<String> result = client.translate(List.of("合成词", "坏词"), "en");
        check(gloss.requested.equals(List.of("合成词", "坏词")) && "en".equals(gloss.target),
            "the texts and the target go to Rust as given: " + gloss.requested + " " + gloss.target);
        check(rejected.equals(Arrays.asList((String) null)), "one token is asked for, with nothing rejected: " + rejected);
        check(calls.count == 1 && "POST".equals(calls.method) && "/v1/translate".equals(calls.path)
                && calls.body == PLANNED, "Rust's method, path and body are sent unchanged");
        check("Bearer token-a".equals(calls.headers.get("Authorization"))
                && "application/json".equals(calls.headers.get("Content-Type"))
                && CloudApi.USER_AGENT.equals(calls.headers.get("User-Agent")),
            "the request carries the shared token and the fixed headers: " + calls.headers);
        check(gloss.status == 200 && "answer".equals(new String(gloss.body)) && gloss.retryAfter == null,
            "the status and body go to Rust");
        check(result.equals(Arrays.asList("synthetic-gloss", null)),
            "an item Rust could not use stays null instead of failing the page: " + result);
    }

    private static void retriesOnceWithAFreshToken() throws Exception {
        FakeGloss gloss = new FakeGloss();
        Calls calls = new Calls();
        List<String> rejected = new ArrayList<>();
        List<String> sent = new ArrayList<>();
        BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) -> {
            calls.record(method, path, headers, body);
            sent.add(headers.get("Authorization"));
            boolean fresh = "Bearer token-b".equals(headers.get("Authorization"));
            return new CloudApi.Exchange(fresh ? 200 : 401, "application/json", null, new byte[0]);
        }, previous -> { rejected.add(previous); return previous == null ? "token-a" : "token-b"; });
        client.translate(List.of("合成词"), "EN");
        check(sent.equals(List.of("Bearer token-a", "Bearer token-b")) && calls.body == PLANNED,
            "a rejected token is exchanged and the same request sent again: " + sent);
        check(rejected.equals(Arrays.asList(null, "token-a")), "the rejected token goes back to the token source: " + rejected);
        check(gloss.status == 200, "the retried answer is the one Rust reads");
    }

    private static void doesNotRetryWithoutANewToken() throws Exception {
        for (String fresh : new String[] {"token-a", ""}) {
            FakeGloss gloss = new FakeGloss();
            Calls calls = new Calls();
            BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) -> {
                calls.record(method, path, headers, body);
                return new CloudApi.Exchange(401, "application/json", null, new byte[0]);
            }, previous -> previous == null ? "token-a" : fresh);
            boolean failed = false;
            try {
                client.translate(List.of("合成词"), "EN");
            } catch (AccountStore.Failure failure) {
                failed = "account_unauthorized".equals(failure.code);
            }
            check(failed && calls.count == 1 && gloss.status == 401,
                "without a different token the 401 goes to Rust and nothing is retried: '" + fresh + "'");
        }
    }

    private static void retriesOnlyOnce() throws Exception {
        FakeGloss gloss = new FakeGloss();
        Calls calls = new Calls();
        int[] tokens = {0};
        BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) -> {
            calls.record(method, path, headers, body);
            return new CloudApi.Exchange(401, "application/json", null, new byte[0]);
        }, previous -> "token-" + tokens[0]++);
        boolean failed = false;
        try {
            client.translate(List.of("合成词"), "EN");
        } catch (AccountStore.Failure failure) {
            failed = "account_unauthorized".equals(failure.code);
        }
        check(failed && calls.count == 2 && tokens[0] == 2, "a second 401 is not retried again: " + calls.count);
    }

    private static void validatesBeforeAskingForAToken() throws Exception {
        FakeGloss gloss = new FakeGloss();
        gloss.invalid = true;
        Calls calls = new Calls();
        int[] tokens = {0};
        BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) -> {
            calls.record(method, path, headers, body);
            return new CloudApi.Exchange(200, "application/json", null, new byte[0]);
        }, previous -> { tokens[0]++; return "token-a"; });
        boolean failed = false;
        try {
            client.translate(List.of(" "), "EN");
        } catch (AccountStore.Failure failure) {
            failed = "account_invalid".equals(failure.code);
        }
        check(failed && tokens[0] == 0 && calls.count == 0,
            "a request Rust rejects asks the main process for no token and sends nothing");
    }

    private static void handsFailuresToRust() throws Exception {
        FakeGloss gloss = new FakeGloss();
        BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) ->
            new CloudApi.Exchange(429, "application/json", "60", "{\"error\":{\"code\":\"rate_limit_exceeded\"}}".getBytes()),
            previous -> "token-a");
        long retryAfter = -1;
        try {
            client.translate(List.of("合成词"), "EN");
        } catch (AccountStore.Failure failure) {
            retryAfter = failure.retryAfterSeconds;
        }
        check(gloss.status == 429 && "60".equals(gloss.retryAfter) && retryAfter == 60
                && new String(gloss.body).contains("rate_limit_exceeded"),
            "a refusal reaches Rust with its body and Retry-After");
    }

    private static void keepsTheResponseBound() throws Exception {
        FakeGloss gloss = new FakeGloss();
        BackendTranslationClient client = new BackendTranslationClient(gloss, (method, path, headers, body) ->
            new CloudApi.Exchange(200, "application/json", null, new byte[BackendTranslationClient.MAX_RESPONSE_BYTES + 1]),
            previous -> "token-a");
        boolean tooLarge = false;
        try {
            client.translate(List.of("合成词"), "EN");
        } catch (IllegalStateException bounded) {
            tooLarge = bounded.getMessage().contains("too large");
        }
        check(tooLarge && gloss.status == -1, "an oversized response is not handed to Rust");
    }

    /** 代替 Rust：request 记下原文和目标语言，parse 记下收到的响应，非 2xx 照 Rust 的映射抛出。 */
    private static final class FakeGloss implements BackendTranslationClient.Gloss {
        List<String> requested;
        String target;
        boolean invalid;
        int status = -1;
        byte[] body;
        String retryAfter;

        @Override public BackendTranslationClient.Plan request(List<String> texts, String target) throws Exception {
            if (invalid) throw new AccountStore.Failure("account_invalid", 0);
            requested = texts;
            this.target = target;
            return new BackendTranslationClient.Plan("POST", "/v1/translate", PLANNED);
        }

        @Override public List<String> parse(List<String> texts, int status, byte[] body, String retryAfter)
                throws Exception {
            this.status = status;
            this.body = body;
            this.retryAfter = retryAfter;
            if (status == 401) throw new AccountStore.Failure("account_unauthorized", 0);
            if (status == 429) throw new AccountStore.Failure("account_rate_limited", Long.parseLong(retryAfter));
            List<String> values = new ArrayList<>(texts.size());
            for (int index = 0; index < texts.size(); index++) values.add(index == 0 ? "synthetic-gloss" : null);
            return values;
        }
    }

    private static final class Calls {
        int count;
        String method;
        String path;
        Map<String, String> headers;
        byte[] body;

        void record(String method, String path, Map<String, String> headers, byte[] body) {
            count++;
            this.method = method;
            this.path = path;
            this.headers = headers;
            this.body = body;
        }
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

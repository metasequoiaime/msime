package app.msime.android;

import android.content.Context;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 「水杉账号」的候选释义（#8201 迁移计划 5e）：请求体怎么拼、回答怎么读都在 Rust 的 `msime_client_account_gloss`（sans-IO 的 request 和 parse），这里只带着令牌把那一次请求发出去，与 Windows 和 iOS 键盘扩展相同；Linux 改用了由 Rust 发请求的 `translate`。
 *
 * <p>令牌是登录账号优先、否则本机匿名账号的那一枚（{@link BackendAnonymousAccount#accessToken}）：`:ime` 经 provider 的 shared_access_token 向主进程要，自己不持有刷新令牌、不刷新。服务端以 401 拒绝时把被拒的令牌交回去换一枚，换到不同的才重试一次。回答逐项是释义或 null（为空、与原文相同、含控制字符或超过 4096 字节），一条坏值不让整页作废；换号以后才到的旧回答由 {@link CandidateTranslationStore#bindTo} 挡住。阻塞网络，放在工作线程上。
 */
public final class BackendTranslationClient implements CandidateTranslationStore.Service {
    /** 响应体上限，与迁移前相同；Rust 的 parse 请求要把整个响应体带进 JSON，也不能让它逼近 JNI 的 4 MiB。 */
    static final int MAX_RESPONSE_BYTES = 256 * 1024;
    private static final int TIMEOUT_MILLIS = 30_000;

    /** Rust 账号释义的两半；冒烟测试换成内存实现。 */
    interface Gloss {
        /** 校验原文和目标语言，给出要发的那一次请求；不合法时抛出，不联网。 */
        Plan request(List<String> texts, String target) throws Exception;

        /** 读收到的响应：与 texts 等长，不可用的一项是 null；非 2xx 换成账号错误码抛出。 */
        List<String> parse(List<String> texts, int status, byte[] body, String retryAfter) throws Exception;
    }

    /** Rust 给出的请求：方法、`/v1/` 下的路径和 UTF-8 JSON 请求体。 */
    record Plan(String method, String path, byte[] body) {}

    /** 取令牌：参数是服务端刚以 401 拒绝的令牌（首次为 null），给了就要换一枚。 */
    interface Tokens {
        String token(String rejected) throws Exception;
    }

    private final Gloss gloss;
    private final CloudApi.Transport transport;
    private final Tokens tokens;

    public BackendTranslationClient(Context context) {
        this(BackendTranslationClient::httpExchange, sharedTokens(context.getApplicationContext()));
    }

    /** 设备冒烟用：请求和解读走真的 Rust，HTTP 和令牌换成内存实现。 */
    BackendTranslationClient(CloudApi.Transport transport, Tokens tokens) {
        this(RUST, transport, tokens);
    }

    BackendTranslationClient(Gloss gloss, CloudApi.Transport transport, Tokens tokens) {
        this.gloss = gloss;
        this.transport = transport;
        this.tokens = tokens;
    }

    private static Tokens sharedTokens(Context application) {
        return rejected -> new BackendAnonymousAccount(application).accessToken(rejected);
    }

    @Override public List<String> translate(List<String> texts, String target) throws Exception {
        // 先让 Rust 校验：不合法的请求不必向主进程要令牌。
        Plan plan = gloss.request(texts, target);
        String token = tokens.token(null);
        CloudApi.Exchange exchange = send(plan, token);
        if (exchange.status() == 401) {
            String fresh = tokens.token(token);
            if (fresh != null && !fresh.isEmpty() && !fresh.equals(token)) exchange = send(plan, fresh);
        }
        byte[] body = exchange.body() == null ? new byte[0] : exchange.body();
        if (!HttpBodyPolicy.isWithinLimit(body, MAX_RESPONSE_BYTES))
            throw new IllegalStateException("Translation response is too large");
        return gloss.parse(texts, exchange.status(), body, exchange.retryAfter());
    }

    private CloudApi.Exchange send(Plan plan, String token) throws IOException {
        Map<String, String> headers = new LinkedHashMap<>(4);
        headers.put("Accept", "application/json");
        headers.put("User-Agent", CloudApi.USER_AGENT);
        headers.put("Authorization", "Bearer " + token);
        headers.put("Content-Type", "application/json");
        return transport.exchange(plan.method(), plan.path(), headers, plan.body());
    }

    /** 经 JNI 调 Rust 的 {@code msime_client_account_gloss}：不联网、不读会话，任意线程。失败是 {@link AccountStore.Failure}，带 Rust 的账号错误码。 */
    private static final Gloss RUST = new Gloss() {
        @Override public Plan request(List<String> texts, String target) throws Exception {
            JSONObject plan = call(new JSONObject().put("operation", "request")
                .put("texts", new JSONArray(texts)).put("source_lang", "ZH").put("target_lang", target));
            return new Plan(JsonPolicy.strictStringOrEmpty(plan.opt("method")),
                JsonPolicy.strictStringOrEmpty(plan.opt("path")),
                TextPolicy.utf8Bytes(JsonPolicy.strictStringOrEmpty(plan.opt("body"))));
        }

        @Override public List<String> parse(List<String> texts, int status, byte[] body, String retryAfter)
                throws Exception {
            JSONObject request = new JSONObject().put("operation", "parse")
                .put("texts", new JSONArray(texts)).put("status", status).put("body", TextPolicy.utf8(body));
            if (retryAfter != null) request.put("retry_after", retryAfter);
            JSONArray values = call(request).optJSONArray("translations");
            if (values == null || values.length() != texts.size()) throw new AccountStore.Failure("account_unavailable", 0);
            ArrayList<String> result = new ArrayList<>(values.length());
            for (int index = 0; index < values.length(); index++) {
                Object value = values.opt(index);
                result.add(value instanceof String text ? text : null);
            }
            return result;
        }

        private JSONObject call(JSONObject request) throws Exception {
            Object value = AccountStore.value(NativeClient.accountGloss(request.toString()));
            if (!(value instanceof JSONObject answer)) throw new AccountStore.Failure("account_unavailable", 0);
            return answer;
        }
    };

    /** 不跟随重定向，响应体按 {@link #MAX_RESPONSE_BYTES} 读；错误响应读不完时当作空的交给 Rust 按状态码解读。 */
    private static CloudApi.Exchange httpExchange(String method, String path,
            Map<String, String> headers, byte[] request) throws IOException {
        HttpsURLConnection connection = (HttpsURLConnection) new URL(CloudApi.ORIGIN + path).openConnection();
        try {
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod(method);
            HttpConnectionPolicy.setTimeouts(connection, TIMEOUT_MILLIS, TIMEOUT_MILLIS);
            for (Map.Entry<String, String> header : headers.entrySet())
                connection.setRequestProperty(header.getKey(), header.getValue());
            if (request != null) {
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(request.length);
                try (OutputStream output = connection.getOutputStream()) { output.write(request); }
            }
            int status = connection.getResponseCode();
            InputStream stream = HttpConnectionPolicy.isSuccessful(status) ? connection.getInputStream() : connection.getErrorStream();
            byte[] response = new byte[0];
            if (stream != null) {
                try (InputStream input = stream) {
                    if (HttpConnectionPolicy.isSuccessful(status)) {
                        response = HttpBodyPolicy.readRequired(input, MAX_RESPONSE_BYTES);
                    } else {
                        try { response = HttpBodyPolicy.readRequired(input, MAX_RESPONSE_BYTES); }
                        catch (IOException unreadable) { response = new byte[0]; }
                    }
                }
            }
            return new CloudApi.Exchange(status, connection.getContentType(),
                connection.getHeaderField("Retry-After"), response);
        } finally {
            connection.disconnect();
        }
    }
}

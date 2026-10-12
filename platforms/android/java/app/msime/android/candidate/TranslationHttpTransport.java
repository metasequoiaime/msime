package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.MalformedURLException;
import java.net.URL;
import java.util.Iterator;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import org.json.JSONObject;

/**
 * 候选翻译接用户自己的服务（小牛、自定义、腾讯）时的 HTTP 传输，结构照 {@link OnlineCandidateTransport}。
 *
 * <p>请求的地址、请求头和正文全部来自共享层的描述器（`msime_client_*_translation_http_request`），签名和鉴权都在 Rust 里完成，这里只搬字节：有 {@code body_utf8} 时原样发送这些字节（腾讯和小牛签的就是它们），否则发 {@code body} 对象。不跟随重定向，只接受 2xx，超时夹在 1 到 10 秒，回复按描述器给的上限（最多 1 MiB）在总时限内读完，到点由守护线程断开连接。任何失败都返回 null 而不是抛出：服务不可用是正常情况，不能影响输入。
 *
 * <p>地址规则与共享层 `translation::is_secure_endpoint` 一致：https，或指向 `localhost`、`127.0.0.1`、`[::1]` 的 http，并且不能带用户名、密码或 # 片段。局域网的 http 不放行，这一点与 AI 候选（{@link AiEndpointPolicy}）不同；iOS 只放行 https，比这里更严。`network_security_config.xml` 整体允许明文，回环 http 的边界只靠这里再查一遍，不能只信描述器。
 */
public final class TranslationHttpTransport {
    /** 共享描述器写的超时；描述器没写时也用它。 */
    public static final int DEFAULT_TIMEOUT_MILLIS = 2_500;
    public static final int MIN_TIMEOUT_MILLIS = 1_000;
    public static final int MAX_TIMEOUT_MILLIS = 10_000;
    /** 回复的上限，与共享解析器能收的上限相同。 */
    public static final int MAX_RESPONSE_BYTES = 1024 * 1024;
    private static final int MAX_URL_BYTES = 2048;
    private static final Set<String> LOOPBACK_HOSTS = Set.of("localhost", "127.0.0.1", "[::1]");

    // 每个请求的整体时限。readTimeout 只管两次读之间的空闲，到点由这个线程断开连接，阻塞中的读会立刻抛出 IOException；守护线程，不拖住进程退出。
    private static final ScheduledExecutorService DEADLINES =
        Executors.newSingleThreadScheduledExecutor(
            ThreadPolicy.namedDaemonFactory("msime-translation-deadline"));

    private TranslationHttpTransport() {}

    /**
     * 描述器给的地址能不能发：https 不限主机，http 只能是本机回环；都不能带用户名、密码或片段。
     *
     * <p>用 {@link URL} 而不是 {@link java.net.URI} 解析：共享层按 WHATWG 规则（reqwest）放行的 https 地址里，带下划线的主机名、中文等国际化域名、查询串里的 `|` 这类字符，{@code URI} 都会拒绝或取不到主机，结果是设置页说地址不对、键盘每次都不发，而共享层的校验和「测试连接」都说没问题。{@code URL} 只按结构切分，这些地址都能放行；地址的完整校验已经由共享层在保存偏好和构造描述器时做过，这里只守 scheme、回环主机、用户信息和片段这几条安全边界。
     */
    public static boolean allowedURL(String url) {
        if (!TextPolicy.boundedNonEmpty(url, MAX_URL_BYTES) || url.indexOf('#') >= 0) return false;
        // `https:///v1` 要求 `://` 后面紧跟主机，免得路径被当成主机。
        int separator = url.indexOf("://");
        if (separator < 0 || url.length() <= separator + 3 || url.charAt(separator + 3) == '/') return false;
        URL parsed;
        try {
            parsed = new URL(url);
        } catch (MalformedURLException error) {
            return false;
        }
        String scheme = parsed.getProtocol();
        String host = parsed.getHost();
        if (scheme == null || host == null || host.isEmpty() || parsed.getUserInfo() != null
                || parsed.getRef() != null || parsed.getPort() < -1 || parsed.getPort() > 65535) return false;
        if ("https".equalsIgnoreCase(scheme)) return true;
        return "http".equalsIgnoreCase(scheme) && LOOPBACK_HOSTS.contains(host.toLowerCase(Locale.ROOT));
    }

    /** 描述器的 {@code timeout_ms}：没写时 2.5 秒，写了就夹在 1 到 10 秒；不是整数时按没写处理。 */
    public static int timeoutMillis(Object raw) {
        return BoundsPolicy.bounded(NumberPolicy.strictInt(raw, DEFAULT_TIMEOUT_MILLIS),
            MIN_TIMEOUT_MILLIS, MAX_TIMEOUT_MILLIS);
    }

    /** 描述器的 {@code max_response_bytes}：没写（{@code opt} 得到 null）时 1 MiB，写了就不超过 1 MiB；不是正整数（包括显式的 JSON null）时为 0，表示这个描述器不能用。 */
    public static int responseLimit(Object raw) {
        if (raw == null) return MAX_RESPONSE_BYTES;
        int value = NumberPolicy.strictInt(raw, 0);
        return value <= 0 ? 0 : BoundsPolicy.atMost(value, MAX_RESPONSE_BYTES);
    }

    /** 按描述器发一次 POST，返回 2xx 回复的正文；地址不合规、超时、非 2xx、回复超限或任何 I/O 失败都返回 null。 */
    public static byte[] post(JSONObject descriptor) {
        HttpURLConnection connection = null;
        ScheduledFuture<?> watchdog = null;
        try {
            String rawUrl = JsonPolicy.strictString(descriptor.opt("url"));
            if (!allowedURL(rawUrl)) return null;
            if (!"POST".equals(JsonPolicy.strictString(descriptor.opt("method"), "POST"))) return null;
            byte[] payload = payload(descriptor);
            int limit = responseLimit(descriptor.opt("max_response_bytes"));
            if (payload == null || limit == 0) return null;
            int timeout = timeoutMillis(descriptor.opt("timeout_ms"));
            long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeout);
            URL target = new URL(rawUrl);
            connection = (HttpURLConnection) AiEndpointPolicy.open(target);
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod("POST");
            HttpConnectionPolicy.setTimeouts(connection, timeout, timeout);
            connection.setDoOutput(true);
            connection.setFixedLengthStreamingMode(payload.length);
            JSONObject headers = descriptor.optJSONObject("headers");
            if (headers != null) {
                for (Iterator<String> names = headers.keys(); names.hasNext();) {
                    String name = names.next();
                    String header = JsonPolicy.strictString(headers.opt(name));
                    if (header == null) return null;
                    connection.setRequestProperty(name, header);
                }
            }
            final HttpURLConnection open = connection;
            watchdog = DEADLINES.schedule(open::disconnect, timeout, TimeUnit.MILLISECONDS);
            try (OutputStream output = connection.getOutputStream()) { output.write(payload); }
            int status = connection.getResponseCode();
            if (!HttpConnectionPolicy.isSuccessful(status)) return null;
            try (InputStream input = connection.getInputStream()) {
                return HttpBodyPolicy.readWithin(input, limit, deadline);
            }
        } catch (IOException | RuntimeException error) {
            // 不记录描述器和回复：它们带着凭据和用户的候选词。
            return null;
        } finally {
            if (watchdog != null) watchdog.cancel(false);
            if (connection != null) connection.disconnect();
        }
    }

    /** 要发送的字节：{@code body_utf8} 逐字节原样发（签名覆盖的就是它们），否则把 {@code body} 对象序列化。两者都没有或类型不对时为 null。 */
    private static byte[] payload(JSONObject descriptor) {
        Object exact = descriptor.opt("body_utf8");
        if (exact != null && exact != JSONObject.NULL) {
            String text = JsonPolicy.strictString(exact);
            return text == null ? null : TextPolicy.utf8Bytes(text);
        }
        JSONObject body = descriptor.optJSONObject("body");
        return body == null ? null : TextPolicy.utf8Bytes(body.toString());
    }
}

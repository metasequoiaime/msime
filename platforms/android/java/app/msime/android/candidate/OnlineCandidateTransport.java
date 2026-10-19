package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.Iterator;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import javax.net.ssl.HttpsURLConnection;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 执行共享宿主构造的云候选与 AI 描述符，只传输字节，不复制协议解析。
 * 云候选允许 HTTPS 与本机 HTTP；AI 另允许 AiEndpointPolicy 放行的局域网 HTTP。
 * 失败返回 null，保留本地候选，描述符中的凭据不得写入日志。
 */
public final class OnlineCandidateTransport {
    private static final int CONNECT_TIMEOUT_MILLIS = 2_500;
    private static final int READ_TIMEOUT_MILLIS = 8_000;

    // 云候选的整体时限。readTimeout 只管两次读之间的空闲，到点由这个线程断开连接，阻塞中的读会立刻抛出 IOException；守护线程，不拖住进程退出。
    private static final ScheduledExecutorService CLOUD_DEADLINES =
        Executors.newSingleThreadScheduledExecutor(
            ThreadPolicy.namedDaemonFactory("msime-cloud-deadline"));

    private OnlineCandidateTransport() {}

    /** 执行共享云候选描述符；命中本机缓存时直接交回响应，不建立连接。 */
    public static String cloud(JSONObject descriptor) {
        if (descriptor == null) return null;
        String cached = JsonPolicy.strictString(descriptor.opt("cached_response"));
        if (cached != null) return OnlineCandidatePolicy.acceptsCloudBody(cached) ? cached : null;
        int timeout = NumberPolicy.strictInt(descriptor, "timeout_ms", 0);
        int connectTimeout = NumberPolicy.strictInt(descriptor, "connect_timeout_ms", 0);
        if (timeout < 100 || timeout > 10_000 || connectTimeout <= 0 || connectTimeout > timeout
                || NumberPolicy.strictInt(descriptor, "max_response_bytes", 0)
                    != OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES) return null;
        final long deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(timeout);
        HttpURLConnection connection = null;
        ScheduledFuture<?> watchdog = null;
        try {
            String rawUrl = JsonPolicy.strictString(descriptor.opt("url"));
            if (!OnlineCandidatePolicy.validAiURL(rawUrl)) return null;
            URL target = AiEndpointPolicy.uri(rawUrl).toURL();
            String method = descriptor.optString("method", "");
            if (!"GET".equals(method) && !"POST".equals(method)) return null;
            connection = (HttpURLConnection) AiEndpointPolicy.open(target);
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod(method);
            HttpConnectionPolicy.setTimeouts(connection, connectTimeout, timeout);
            JSONObject headers = descriptor.getJSONObject("headers");
            for (Iterator<String> names = headers.keys(); names.hasNext();) {
                String name = names.next();
                connection.setRequestProperty(name, headers.getString(name));
            }
            watchdog = CLOUD_DEADLINES.schedule(connection::disconnect, timeout, TimeUnit.MILLISECONDS);
            if ("POST".equals(method)) {
                byte[] payload = TextPolicy.utf8Bytes(descriptor.getString("body_utf8"));
                if (payload.length > 8192) return null;
                connection.setDoOutput(true);
                connection.setFixedLengthStreamingMode(payload.length);
                try (OutputStream output = connection.getOutputStream()) { output.write(payload); }
            }
            if (!HttpConnectionPolicy.isSuccessful(connection.getResponseCode())) return null;
            try (InputStream input = connection.getInputStream()) {
                byte[] body = HttpBodyPolicy.readWithin(input,
                    OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES, deadline);
                return body == null ? null : TextPolicy.utf8(body);
            }
        } catch (IOException | JSONException | RuntimeException error) {
            return null;
        } finally {
            if (watchdog != null) watchdog.cancel(false);
            if (connection != null) connection.disconnect();
        }
    }

    /** POST the descriptor the session built. Returns null when the provider is unusable. */
    public static String ai(JSONObject descriptor) {
        HttpURLConnection connection = null;
        try {
            String rawUrl = JsonPolicy.strictString(descriptor.opt("url"));
            if (!OnlineCandidatePolicy.validAiURL(rawUrl)) return null;
            URL target = AiEndpointPolicy.uri(rawUrl).toURL();
            byte[] payload = TextPolicy.utf8Bytes(descriptor.getJSONObject("body").toString());
            connection = (HttpURLConnection) AiEndpointPolicy.open(target);
            HttpConnectionPolicy.rejectRedirects(connection);
            connection.setRequestMethod("POST");
            HttpConnectionPolicy.setTimeouts(connection, BoundsPolicy.bounded(
                NumberPolicy.strictInt(descriptor, "connect_timeout_ms", CONNECT_TIMEOUT_MILLIS),
                1_000, 10_000), BoundsPolicy.bounded(
                NumberPolicy.strictInt(descriptor, "timeout_ms", READ_TIMEOUT_MILLIS),
                1_000, 10_000));
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
            try (OutputStream output = connection.getOutputStream()) { output.write(payload); }
            int status = connection.getResponseCode();
            if (!HttpConnectionPolicy.isSuccessful(status)) return null;
            try (InputStream input = connection.getInputStream()) {
                byte[] body = HttpBodyPolicy.readBounded(input,
                    OnlineCandidatePolicy.MAX_AI_RESPONSE_BYTES);
                return body == null ? null : TextPolicy.utf8(body);
            }
        } catch (IOException | JSONException | RuntimeException error) {
            return null;
        } finally {
            if (connection != null) connection.disconnect();
        }
    }
}

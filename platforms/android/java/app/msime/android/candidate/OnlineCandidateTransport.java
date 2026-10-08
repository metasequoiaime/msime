package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
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
 * Bounded HTTPS transport for the optional cloud and AI candidate providers.
 *
 * <p>Both requests are built by the shared host: the cloud URL comes from `cloud_request_url` and
 * the AI descriptor from `ai_request_for_query`, so credentials stay inside the session and this
 * class only carries bytes. It returns null rather than throwing, because an unavailable provider
 * is an ordinary outcome that must leave the composition alone.
 */
public final class OnlineCandidateTransport {
    private static final int CONNECT_TIMEOUT_MILLIS = 2_500;
    private static final int READ_TIMEOUT_MILLIS = 8_000;

    // 云候选的整体时限。readTimeout 只管两次读之间的空闲，到点由这个线程断开连接，阻塞中的读会立刻抛出 IOException；守护线程，不拖住进程退出。
    private static final ScheduledExecutorService CLOUD_DEADLINES =
        Executors.newSingleThreadScheduledExecutor(task -> {
            Thread thread = new Thread(task, "msime-cloud-deadline");
            thread.setDaemon(true);
            return thread;
        });

    private OnlineCandidateTransport() {}

    /** GET the cloud candidate service. Returns null when it is unusable or answers too much. */
    public static String cloud(String url) {
        final long deadline = System.nanoTime()
            + TimeUnit.MILLISECONDS.toNanos(OnlineCandidatePolicy.CLOUD_TIMEOUT_MILLIS);
        HttpsURLConnection connection = null;
        ScheduledFuture<?> watchdog = null;
        try {
            URL target = new URL(url);
            if (!OnlineCandidatePolicy.validURL(target)) return null;
            connection = (HttpsURLConnection) target.openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("GET");
            connection.setConnectTimeout(OnlineCandidatePolicy.CLOUD_TIMEOUT_MILLIS);
            connection.setReadTimeout(OnlineCandidatePolicy.CLOUD_TIMEOUT_MILLIS);
            connection.setRequestProperty("Accept", "application/json");
            watchdog = CLOUD_DEADLINES.schedule(connection::disconnect,
                OnlineCandidatePolicy.CLOUD_TIMEOUT_MILLIS, TimeUnit.MILLISECONDS);
            int status = connection.getResponseCode();
            if (status < 200 || status >= 300) return null;
            try (InputStream input = connection.getInputStream()) {
                byte[] body = HttpBodyPolicy.readWithin(input,
                    OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES, deadline);
                return body == null ? null : TextPolicy.utf8(body);
            }
        } catch (IOException | RuntimeException error) {
            return null;
        } finally {
            if (watchdog != null) watchdog.cancel(false);
            if (connection != null) connection.disconnect();
        }
    }

    /** POST the descriptor the session built. Returns null when the provider is unusable. */
    public static String ai(JSONObject descriptor) {
        HttpsURLConnection connection = null;
        try {
            String rawUrl = JsonPolicy.strictString(descriptor.opt("url"));
            if (rawUrl == null) return null;
            URL target = new URL(rawUrl);
            if (!OnlineCandidatePolicy.validURL(target)) return null;
            byte[] payload = TextPolicy.utf8Bytes(descriptor.getJSONObject("body").toString());
            connection = (HttpsURLConnection) target.openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("POST");
            connection.setConnectTimeout(KeyboardGeometry.bounded(
                KeyboardGeometry.strictInt(descriptor, "connect_timeout_ms", CONNECT_TIMEOUT_MILLIS),
                1_000, 10_000));
            connection.setReadTimeout(KeyboardGeometry.bounded(
                KeyboardGeometry.strictInt(descriptor, "timeout_ms", READ_TIMEOUT_MILLIS),
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
            if (status < 200 || status >= 300) return null;
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

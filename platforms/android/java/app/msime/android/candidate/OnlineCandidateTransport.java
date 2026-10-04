package app.msime.android;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.Iterator;
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

    private OnlineCandidateTransport() {}

    /** GET the cloud candidate service. Returns null when it is unusable or answers too much. */
    public static String cloud(String url) {
        HttpsURLConnection connection = null;
        try {
            URL target = new URL(url);
            if (!"https".equalsIgnoreCase(target.getProtocol())) return null;
            connection = (HttpsURLConnection) target.openConnection();
            connection.setInstanceFollowRedirects(false);
            connection.setRequestMethod("GET");
            connection.setConnectTimeout(CONNECT_TIMEOUT_MILLIS);
            connection.setReadTimeout(READ_TIMEOUT_MILLIS);
            connection.setRequestProperty("Accept", "application/json");
            int status = connection.getResponseCode();
            if (status < 200 || status >= 300) return null;
            try (InputStream input = connection.getInputStream()) {
                return readBounded(input, OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES);
            }
        } catch (IOException | RuntimeException error) {
            return null;
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    /** POST the descriptor the session built. Returns null when the provider is unusable. */
    public static String ai(JSONObject descriptor) {
        HttpsURLConnection connection = null;
        try {
            URL target = new URL(descriptor.getString("url"));
            if (!"https".equalsIgnoreCase(target.getProtocol())) return null;
            byte[] payload = descriptor.getJSONObject("body").toString()
                .getBytes(StandardCharsets.UTF_8);
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
                    connection.setRequestProperty(name, headers.getString(name));
                }
            }
            try (OutputStream output = connection.getOutputStream()) { output.write(payload); }
            int status = connection.getResponseCode();
            if (status < 200 || status >= 300) return null;
            try (InputStream input = connection.getInputStream()) {
                return readBounded(input, OnlineCandidatePolicy.MAX_AI_RESPONSE_BYTES);
            }
        } catch (IOException | JSONException | RuntimeException error) {
            return null;
        } finally {
            if (connection != null) connection.disconnect();
        }
    }

    private static String readBounded(InputStream input, int limit) throws IOException {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        byte[] buffer = new byte[8192];
        int count;
        while ((count = input.read(buffer)) != -1) {
            if (output.size() + count > limit) return null;
            output.write(buffer, 0, count);
        }
        return output.toString(StandardCharsets.UTF_8.name());
    }
}

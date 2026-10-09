package app.msime.android;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Sends one transcript through the user's AI service and returns the rewrite.
 *
 * <p>Best effort by construction: polishing improves a transcript the user already has, so every
 * failure returns null and the caller keeps the original. Losing a recognised sentence because a
 * rewrite service was unreachable would be a worse outcome than not polishing it.
 */
public final class VoicePolisher {
    private static final int CONNECT_TIMEOUT_MILLIS = 5_000;
    private static final int READ_TIMEOUT_MILLIS = 60_000;
    private static final int MAX_RESPONSE_BYTES = 1024 * 1024;

    private volatile HttpURLConnection connection;
    private volatile boolean cancelled;

    /** Interrupt a polish request, including one blocked in a response read. */
    public void cancel() {
        cancelled = true;
        HttpURLConnection active = connection;
        if (active != null) active.disconnect();
    }

    /** The polished text, or null to keep what was recognised. */
    public String polish(String endpoint, String model, String token, String prompt,
                                String text) {
        if (cancelled) return null;
        if (!VoicePolishPolicy.usable(endpoint, model, token, prompt)
                || !VoicePolishPolicy.sendable(text)) {
            return null;
        }
        byte[] body = TextPolicy.utf8Bytes(VoicePolishPolicy.requestBody(model, prompt, text));
        HttpURLConnection connection = null;
        try {
            connection = (HttpURLConnection) new URL(endpoint).openConnection();
            this.connection = connection;
            if (cancelled) return null;
            connection.setConnectTimeout(CONNECT_TIMEOUT_MILLIS);
            connection.setReadTimeout(READ_TIMEOUT_MILLIS);
            connection.setRequestMethod("POST");
            connection.setDoOutput(true);
            // The bearer token belongs to this configured origin. Never let HttpURLConnection
            // replay it after a redirect to another host or protocol.
            connection.setInstanceFollowRedirects(false);
            connection.setFixedLengthStreamingMode(body.length);
            for (Map.Entry<String, String> header : authenticationHeaders(
                    endpoint, token).entrySet()) {
                connection.setRequestProperty(header.getKey(), header.getValue());
            }
            connection.setRequestProperty("Content-Type", "application/json");
            connection.setRequestProperty("Accept", "application/json");
            try (OutputStream out = connection.getOutputStream()) {
                out.write(body);
            }
            if (cancelled) return null;
            int status = connection.getResponseCode();
            if (status < 200 || status >= 300) return null;
            String response;
            try (InputStream input = connection.getInputStream()) {
                byte[] responseBytes = HttpBodyPolicy.readBounded(input, MAX_RESPONSE_BYTES);
                response = responseBytes == null
                    ? null : TextPolicy.utf8(responseBytes);
            }
            String content = content(response);
            return cancelled ? null
                : (VoicePolishPolicy.sendable(content) ? TextPolicy.trimmed(content) : null);
        } catch (IOException error) {
            return null;
        } finally {
            if (this.connection == connection) this.connection = null;
            if (connection != null) connection.disconnect();
        }
    }

    private static String content(String response) {
        if (response == null) return "";
        try {
            JSONArray choices = new JSONObject(response).optJSONArray("choices");
            JSONObject first = choices == null ? null : choices.optJSONObject(0);
            JSONObject message = first == null ? null : first.optJSONObject("message");
            return message == null ? "" : JsonPolicy.strictStringOrEmpty(message.opt("content"));
        } catch (JSONException error) {
            return "";
        }
    }

    /** Authentication headers for the provider endpoint, matching the shared AI transport. */
    public static Map<String, String> authenticationHeaders(String endpoint, String token) {
        String host;
        try {
            host = new URL(endpoint).getHost();
        } catch (IOException | SecurityException error) {
            host = "";
        }
        return AiProviderHeaders.forHost(host, token);
    }
}

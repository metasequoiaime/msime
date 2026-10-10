package app.msime.android;

import java.io.IOException;
import java.io.OutputStream;
import javax.net.ssl.HttpsURLConnection;

/** Shared request setup for the two account login transports. Response policies stay with each account. */
final class AccountHttpRequest {
    private AccountHttpRequest() {}

    static void writeJson(HttpsURLConnection connection, String method, String token,
            String userAgent, byte[] payload) throws IOException {
        connection.setInstanceFollowRedirects(false);
        connection.setRequestMethod(method);
        connection.setConnectTimeout(30_000);
        connection.setReadTimeout(30_000);
        connection.setRequestProperty("Accept", "application/json");
        connection.setRequestProperty("User-Agent", userAgent);
        if (token != null) connection.setRequestProperty("Authorization", "Bearer " + token);
        if (payload == null) return;
        connection.setDoOutput(true);
        connection.setFixedLengthStreamingMode(payload.length);
        connection.setRequestProperty("Content-Type", "application/json");
        try (OutputStream output = connection.getOutputStream()) { output.write(payload); }
    }
}

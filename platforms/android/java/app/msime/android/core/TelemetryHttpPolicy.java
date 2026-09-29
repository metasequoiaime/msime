package app.msime.android.core;

import java.net.HttpURLConnection;

/** Transport settings for the fixed telemetry endpoint. */
public final class TelemetryHttpPolicy {
    private TelemetryHttpPolicy() {}

    public static void configure(HttpURLConnection connection) {
        connection.setInstanceFollowRedirects(false);
        connection.setRequestProperty("Content-Type", "application/json");
    }
}

package app.msime.android;

import java.net.HttpURLConnection;

/** Keep each request on its configured origin instead of forwarding it through an HTTP redirect. */
public final class HttpConnectionPolicy {
    private HttpConnectionPolicy() {}

    public static void rejectRedirects(HttpURLConnection connection) {
        connection.setInstanceFollowRedirects(false);
    }
}

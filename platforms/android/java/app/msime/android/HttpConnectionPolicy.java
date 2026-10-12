package app.msime.android;

import java.net.HttpURLConnection;
import java.net.URLConnection;

/** Shared per-connection HTTP settings. */
public final class HttpConnectionPolicy {
    private HttpConnectionPolicy() {}

    public static void rejectRedirects(HttpURLConnection connection) {
        connection.setInstanceFollowRedirects(false);
    }

    public static void setTimeouts(URLConnection connection, int connectMillis, int readMillis) {
        connection.setConnectTimeout(connectMillis);
        connection.setReadTimeout(readMillis);
    }

    /** Whether an HTTP response has a successful 2xx status. */
    public static boolean isSuccessful(int status) {
        return status >= 200 && status < 300;
    }
}

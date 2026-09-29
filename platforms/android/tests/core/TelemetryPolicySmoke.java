import app.msime.android.core.TelemetryHttpPolicy;
import java.net.HttpURLConnection;
import java.net.URL;

public final class TelemetryPolicySmoke {
    private static final class FakeConnection extends HttpURLConnection {
        FakeConnection() throws Exception { super(new URL("https://fixture.invalid/events")); }
        @Override public void disconnect() {}
        @Override public boolean usingProxy() { return false; }
        @Override public void connect() {}
    }

    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        FakeConnection connection = new FakeConnection();
        TelemetryHttpPolicy.configure(connection);
        check(!connection.getInstanceFollowRedirects(), "telemetry must not follow redirects");
        check("application/json".equals(connection.getRequestProperty("Content-Type")),
            "telemetry sends JSON");
        System.out.println("Android telemetry policy passed");
    }
}

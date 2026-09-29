package app.msime.android.core;

import android.content.Context;
import android.content.SharedPreferences;
import org.json.JSONArray;
import org.json.JSONObject;
import java.net.HttpURLConnection;
import java.net.URL;
import java.io.OutputStream;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Best-effort, bounded telemetry queue shared by the Android app and IME process. */
public final class Telemetry {
    private static final String PREFS = "msime-telemetry", EVENTS = "events";
    private static final ExecutorService WORKER = Executors.newSingleThreadExecutor();
    private Telemetry() {}
    public static void start(Context context) {
        Context app = context.getApplicationContext(); SharedPreferences prefs = app.getSharedPreferences(PREFS, 0);
        if (!prefs.getBoolean("first-launch", false)) { prefs.edit().putBoolean("first-launch", true).apply(); enqueue(app, event("download", null, null)); }
        flush(app);
        Thread.UncaughtExceptionHandler previous = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((thread, error) -> { enqueue(app, event("crash", error.toString(), stack(error))); if (previous != null) previous.uncaughtException(thread, error); });
    }
    private static JSONObject event(String kind, String message, String stack) { JSONObject value = new JSONObject(); try { value.put("id", UUID.randomUUID().toString()); value.put("kind", kind); value.put("platform", "android"); value.put("version", "0.1.0-dev"); if (message != null) value.put("message", clip(message, 2048)); if (stack != null) value.put("stack", clip(stack, 12000)); } catch (Exception ignored) {} return value; }
    private static String stack(Throwable error) { StringBuilder out = new StringBuilder(); for (StackTraceElement frame : error.getStackTrace()) out.append(frame).append('\n'); return out.toString(); }
    private static String clip(String value, int limit) { return value.length() <= limit ? value : value.substring(0, limit); }
    private static synchronized void enqueue(Context context, JSONObject event) { try { SharedPreferences prefs = context.getSharedPreferences(PREFS, 0); JSONArray old = new JSONArray(prefs.getString(EVENTS, "[]")); JSONArray next = new JSONArray(); for (int i = Math.max(0, old.length() - 63); i < old.length(); i++) next.put(old.getJSONObject(i)); next.put(event); prefs.edit().putString(EVENTS, next.toString()).apply(); } catch (Exception ignored) {} }
    private static void flush(Context context) { WORKER.execute(() -> { try { SharedPreferences prefs = context.getSharedPreferences(PREFS, 0); JSONArray events = new JSONArray(prefs.getString(EVENTS, "[]")), pending = new JSONArray(); for (int i = 0; i < events.length(); i++) { JSONObject event = events.getJSONObject(i); if (!send(event)) pending.put(event); } prefs.edit().putString(EVENTS, pending.toString()).apply(); } catch (Exception ignored) {} }); }
    private static boolean send(JSONObject event) { HttpURLConnection connection = null; try { connection = (HttpURLConnection) new URL("https://api.msime.app/v1/telemetry/events").openConnection(); connection.setRequestMethod("POST"); connection.setConnectTimeout(5000); connection.setReadTimeout(10000); TelemetryHttpPolicy.configure(connection); connection.setDoOutput(true); try (OutputStream output = connection.getOutputStream()) { output.write(event.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8)); } int status = connection.getResponseCode(); return status >= 200 && status < 300; } catch (Exception ignored) { return false; } finally { if (connection != null) connection.disconnect(); } }
}

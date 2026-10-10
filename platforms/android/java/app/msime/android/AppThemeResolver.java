package app.msime.android;

import org.json.JSONException;
import org.json.JSONObject;

/** Resolves one app theme palette through the native host's strict JSON envelope. */
public final class AppThemeResolver {
    private AppThemeResolver() {}

    /** Returns the resolved palette, or null when the native host is unavailable or rejects it. */
    public static JSONObject resolve(String theme, int month, boolean dark) {
        try {
            JSONObject root = new JSONObject(NativeClient.resolveAppTheme(theme, month, dark));
            return JsonPolicy.strictTrue(root.opt("ok"))
                ? root.optJSONObject("value") : null;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }
}

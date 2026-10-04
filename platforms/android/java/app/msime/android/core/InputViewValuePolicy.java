package app.msime.android.core;

import app.msime.android.KeyboardGeometry;
import org.json.JSONObject;

/** Reads integer fields returned in the native input view without lossy JSON conversion. */
public final class InputViewValuePolicy {
    private InputViewValuePolicy() {}

    public static int scheme(JSONObject view, int fallback) {
        return scheme(view, "scheme", fallback);
    }

    public static int scheme(JSONObject view, String key, int fallback) {
        return KeyboardGeometry.strictInt(view == null ? null : view.opt(key), fallback);
    }

    public static int schemeValue(Object raw, int fallback) {
        return KeyboardGeometry.strictInt(raw, fallback);
    }
}

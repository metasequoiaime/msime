package app.msime.android.core;

import app.msime.android.KeyboardGeometry;
import app.msime.android.JsonPolicy;
import org.json.JSONObject;

/** Reads fields returned in the native input view without lossy JSON conversion. */
public final class InputViewValuePolicy {
    private InputViewValuePolicy() {}

    public static int scheme(JSONObject view, int fallback) {
        return scheme(view, "scheme", fallback);
    }

    public static int scheme(JSONObject view, String key, int fallback) {
        return integer(view, key, fallback);
    }

    public static int integer(JSONObject object, String key, int fallback) {
        return KeyboardGeometry.strictInt(object == null ? null : object.opt(key), fallback);
    }

    /** Read a JSON boolean without org.json's implicit string coercion. */
    public static boolean booleanValue(JSONObject object, String key, boolean fallback) {
        Object raw = object == null ? null : object.opt(key);
        return booleanValue(raw, fallback);
    }

    public static boolean booleanValue(Object raw, boolean fallback) {
        return JsonPolicy.strictBoolean(raw, fallback);
    }

    /** Read a JSON string field without converting numbers, booleans, or JSON null to text. */
    public static String text(JSONObject object, String key) {
        return text(object == null ? null : object.opt(key));
    }

    public static String text(Object raw) {
        return JsonPolicy.strictStringOrEmpty(raw);
    }

    /** Read a JSON string or use the supplied fallback for missing or invalid values. */
    public static String textOr(Object raw, String fallback) {
        return JsonPolicy.strictString(raw, fallback);
    }

    public static String textOr(JSONObject object, String key, String fallback) {
        return textOr(object == null ? null : object.opt(key), fallback);
    }

    /** Read the input view's editing text using the shared strict text policy. */
    public static String editingText(JSONObject view) {
        return text(view, "editing_text");
    }
}

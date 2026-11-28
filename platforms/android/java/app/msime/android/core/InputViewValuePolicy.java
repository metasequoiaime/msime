package app.msime.android.core;

import app.msime.android.NumberPolicy;

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
        return NumberPolicy.strictInt(object == null ? null : object.opt(key), fallback);
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

    /** View 的 `punctuation_marks`：引号与方头括号样式、顿号键和保留英文的键。缺失或取值不认识时是缺省（弯引号、实心括号、`\` 出顿号、没有保留英文的键）。 */
    public static app.msime.android.ChineseSymbolFaces.Marks punctuationMarks(JSONObject view) {
        JSONObject marks = view == null ? null : view.optJSONObject("punctuation_marks");
        if (marks == null) return app.msime.android.ChineseSymbolFaces.Marks.DEFAULT;
        return app.msime.android.ChineseSymbolFaces.Marks.of(
            textOr(marks, "quotes", "curly"), textOr(marks, "brackets", "solid"),
            booleanValue(marks, "enumeration_comma_backslash", true),
            booleanValue(marks, "enumeration_comma_slash", false),
            preserveEnglish(marks.optJSONArray("preserve_english_punctuation")));
    }

    /** `preserve_english_punctuation` 的单字符键拼成一串；缺失或格式不对的项不算。 */
    static String preserveEnglish(org.json.JSONArray keys) {
        if (keys == null) return "";
        StringBuilder joined = new StringBuilder();
        for (int index = 0; index < keys.length(); index++) {
            Object key = keys.opt(index);
            if (key instanceof String text && text.length() == 1 && text.charAt(0) < 0x80
                && joined.indexOf(text) < 0) joined.append(text);
        }
        return joined.toString();
    }

    /** Read the input view's editing text using the shared strict text policy. */
    public static String editingText(JSONObject view) {
        return text(view, "editing_text");
    }
}

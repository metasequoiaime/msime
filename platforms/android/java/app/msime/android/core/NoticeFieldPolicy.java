package app.msime.android.core;

/** Typed fields carried by the native notice response. */
public final class NoticeFieldPolicy {
    private NoticeFieldPolicy() {}

    /** Notice text must remain a JSON string; org.json's optString would coerce other values. */
    public static String strictString(Object value) {
        return value instanceof String ? (String) value : null;
    }
}

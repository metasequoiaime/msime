package app.msime.android.core;

import app.msime.android.JsonPolicy;

/** Typed fields carried by the native notice response. */
public final class NoticeFieldPolicy {
    private NoticeFieldPolicy() {}

    /** Notice text must remain a JSON string; org.json's optString would coerce other values. */
    public static String strictString(Object value) {
        return JsonPolicy.strictString(value);
    }
}

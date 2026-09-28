package app.msime.client.clipboard;

import app.msime.client.TextPolicy;

/** The account API's cloud clipboard text contract, shared by validation and the native screen. */
public final class CloudClipboardTextPolicy {
    /** The service counts UTF-16 units for this field, as does the shared client-core validator. */
    public static final int MAX_UTF16_UNITS = 4_000;

    private CloudClipboardTextPolicy() {}

    public static boolean valid(String text) {
        if (text == null || TextPolicy.blank(text) || text.length() > MAX_UTF16_UNITS) {
            return false;
        }
        return !TextPolicy.hasControlExceptWhitespace(text);
    }
}

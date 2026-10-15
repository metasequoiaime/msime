package app.msime.android.clipboard;

import app.msime.android.TextPolicy;

/** The account API's cloud clipboard text contract, shared by validation and the native screen. */
public final class CloudClipboardTextPolicy {
    /** 服务端按 UTF-16 码元计算这个字段的长度，共享的 client-services 校验器也是如此。 */
    public static final int MAX_UTF16_UNITS = 4_000;

    private CloudClipboardTextPolicy() {}

    public static boolean valid(String text) {
        if (text == null || TextPolicy.blank(text) || text.length() > MAX_UTF16_UNITS) {
            return false;
        }
        return !TextPolicy.hasControlExceptWhitespace(text) && TextPolicy.validUnicode(text);
    }
}

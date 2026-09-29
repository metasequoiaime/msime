package app.msime.android;

/** Converts only direct printable ASCII input to Unicode fullwidth forms. */
public final class FullWidthInputPolicy {
    private FullWidthInputPolicy() {}

    public static String output(String text, boolean enabled) {
        if (!enabled || text == null || text.isEmpty()) return text;
        StringBuilder converted = new StringBuilder(text.length());
        for (int offset = 0; offset < text.length();) {
            int original = text.codePointAt(offset);
            int output = original == 0x20 ? 0x3000
                : original >= 0x21 && original <= 0x7e ? original + 0xfee0 : original;
            converted.appendCodePoint(output);
            offset += Character.charCount(original);
        }
        return converted.toString();
    }
}

package app.msime.android;

/** The request boundary shared by the Android voice plugin and its pure JVM contract tests. */
public final class VoiceRequestPolicy {
    public static final int MAX_REQUEST_ID_BYTES = 64;
    public static final int MAX_LANGUAGE_BYTES = 64;

    private VoiceRequestPolicy() {}

    /** Mirrors the shared host API: ASCII request ids and bounded, well-formed language text. */
    public static boolean valid(String requestId, String language) {
        return validRequestId(requestId)
            && language != null && !language.isEmpty()
            && TextPolicy.utf8Length(language) <= MAX_LANGUAGE_BYTES
            && !TextPolicy.hasControl(language) && TextPolicy.validUnicode(language);
    }

    private static boolean validRequestId(String value) {
        if (value == null || value.isEmpty() || TextPolicy.utf8Length(value) > MAX_REQUEST_ID_BYTES) {
            return false;
        }
        for (int index = 0; index < value.length(); index++) {
            char unit = value.charAt(index);
            if (!((unit >= 'a' && unit <= 'z') || (unit >= 'A' && unit <= 'Z')
                    || (unit >= '0' && unit <= '9') || unit == '-')) return false;
        }
        return true;
    }
}

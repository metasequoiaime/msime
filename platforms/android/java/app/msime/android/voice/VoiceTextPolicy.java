package app.msime.android;

/** Small text sizing rules shared by the voice transports. */
public final class VoiceTextPolicy {
    private VoiceTextPolicy() {}

    /** Estimate the UTF-16 length contribution while reserving four characters for null. */
    public static int length(String value) {
        return value == null ? 4 : value.length();
    }
}

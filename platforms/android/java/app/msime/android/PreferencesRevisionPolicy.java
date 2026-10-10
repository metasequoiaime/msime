package app.msime.android;

/** Reads the shared preferences revision without lossy JSON number conversion. */
public final class PreferencesRevisionPolicy {
    private PreferencesRevisionPolicy() {}

    /** Return a non-negative exact integer revision, or {@code fallback} when malformed. */
    public static long read(Object raw, long fallback) {
        long revision = NumberPolicy.strictLong(raw, fallback);
        return revision >= 0 ? revision : fallback;
    }
}

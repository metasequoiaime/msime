package app.msime.android;

/** Validates native dictionary snapshot handles before they cross back into JNI. */
public final class DictionarySnapshotPolicy {
    private DictionarySnapshotPolicy() {}

    /** Return a positive exact handle, or {@code fallback} for malformed values. */
    public static long handle(Object raw, long fallback) {
        long value = KeyboardGeometry.strictLong(raw, fallback);
        return value > 0 ? value : fallback;
    }
}

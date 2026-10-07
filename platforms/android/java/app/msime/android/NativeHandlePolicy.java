package app.msime.android;

/** Validation shared by Java entry points that pass opaque native pointers. */
public final class NativeHandlePolicy {
    private NativeHandlePolicy() {}

    public static long requirePositive(long handle) {
        if (handle <= 0) throw new IllegalArgumentException("Invalid native handle");
        return handle;
    }

    public static boolean isOptional(long handle) {
        return handle >= 0;
    }
}

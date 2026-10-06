package app.msime.android;

/** Guards an asynchronous save from replacing a newer preference snapshot. */
public final class PreferencesSavePolicy {
    private PreferencesSavePolicy() {}

    /** A save envelope succeeds only when its {@code ok} member is a JSON boolean true. */
    public static boolean accepted(Object value) {
        return value instanceof Boolean && (Boolean) value;
    }

    public static boolean shouldApplyResponse(long currentRevision, long savedRevision) {
        return currentRevision < 0 || savedRevision >= currentRevision;
    }
}

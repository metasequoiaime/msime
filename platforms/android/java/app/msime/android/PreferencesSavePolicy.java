package app.msime.android;

/** Guards an asynchronous save from replacing a newer preference snapshot. */
public final class PreferencesSavePolicy {
    private PreferencesSavePolicy() {}

    public static boolean shouldApplyResponse(long currentRevision, long savedRevision) {
        return currentRevision < 0 || savedRevision >= currentRevision;
    }
}

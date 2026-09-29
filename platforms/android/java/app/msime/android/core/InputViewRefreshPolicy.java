package app.msime.android;

/** Guards a delayed input-view refresh against a stale editor or active composition. */
public final class InputViewRefreshPolicy {
    private InputViewRefreshPolicy() {}

    public static boolean shouldRefresh(
            long expectedGeneration,
            long currentGeneration,
            Object expectedConnection,
            Object currentConnection,
            boolean viewValid,
            boolean hasComposition) {
        return expectedConnection != null
            && currentConnection != null
            && expectedGeneration == currentGeneration
            && expectedConnection == currentConnection
            && viewValid
            && !hasComposition;
    }
}

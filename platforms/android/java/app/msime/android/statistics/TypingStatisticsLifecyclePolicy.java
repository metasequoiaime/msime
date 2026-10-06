package app.msime.android;

/** Reject statistics worker failures that belong to an earlier input lifecycle. */
public final class TypingStatisticsLifecyclePolicy {
    private TypingStatisticsLifecyclePolicy() {}

    public static boolean acceptsFailure(long requestGeneration, long currentGeneration) {
        return requestGeneration == currentGeneration;
    }
}

package app.msime.android;

/** Small bounds shared by microphone capture paths. */
public final class VoiceCapturePolicy {
    private VoiceCapturePolicy() {}

    /**
     * Returns how many capture units the next read may request without exceeding {@code limit}.
     *
     * <p>{@code AudioRecord.read} can fill the supplied buffer, so a loop condition that only
     * checks the amount already captured is not enough for the final partial buffer.
     */
    public static int readLength(long limit, long captured, int requested) {
        if (limit <= 0 || captured < 0 || requested <= 0 || captured >= limit) return 0;
        return (int) BoundsPolicy.atMost((long) requested, limit - captured);
    }
}

package app.msime.android;

/** Identity fence for the horizontal candidate strip's scroll position. */
public final class CandidateScrollPolicy {
    private CandidateScrollPolicy() {}

    public static boolean changed(long previousSession, long previousGeneration, int previousPage,
                                  long session, long generation, int page) {
        return previousSession != session || previousGeneration != generation || previousPage != page;
    }
}

package app.msime.client;

/** Pure lifecycle and presentation rules for optional offline candidate glosses. */
public final class CandidateGlossPolicy {
    public static final int MAX_ENTRY_BYTES = 4096;

    public record Token(long session, long generation, long epoch) {
        public Token {
            if (session <= 0 || generation < 0 || epoch < 0)
                throw new IllegalArgumentException("Invalid candidate gloss token");
        }

        public boolean isCurrent(long currentSession, long currentGeneration, long currentEpoch) {
            return session == currentSession && generation == currentGeneration
                && epoch == currentEpoch;
        }
    }

    private CandidateGlossPolicy() {}

    /** Engine annotations (for example Wubi codes) occupy the shared hint slot first. */
    public static String annotation(
            String engineAnnotation, String translation, boolean glossEnabled) {
        if (engineAnnotation != null && !engineAnnotation.isEmpty()) return engineAnnotation;
        return glossEnabled && validEntry(translation) ? translation : "";
    }

    public static String accessibilitySuffix(
            String engineAnnotation, String translation, boolean glossEnabled) {
        if (engineAnnotation != null && !engineAnnotation.isEmpty())
            return "，提示：" + engineAnnotation;
        String gloss = annotation(engineAnnotation, translation, glossEnabled);
        return gloss.isEmpty() ? "" : "，英文释义：" + gloss;
    }

    public static boolean validEntry(String value) {
        return value != null && !value.isEmpty()
            && value.getBytes(java.nio.charset.StandardCharsets.UTF_8).length <= MAX_ENTRY_BYTES;
    }
}

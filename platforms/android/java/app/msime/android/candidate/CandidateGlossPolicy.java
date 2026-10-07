package app.msime.android;

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

    /** Read a JSON integer without org.json's lossy numeric coercion. */
    public static long strictInteger(Object value) {
        if (!(value instanceof Integer) && !(value instanceof Long))
            throw new IllegalArgumentException("Expected JSON integer");
        return ((Number) value).longValue();
    }

    /** Return a fallback for a missing or malformed protocol integer instead of coercing it. */
    public static long strictOr(Object value, long fallback) {
        try {
            return strictInteger(value);
        } catch (IllegalArgumentException error) {
            return fallback;
        }
    }

    /** Read a JSON string without org.json's implicit scalar-to-string coercion. */
    public static String strictString(Object value) {
        if (!(value instanceof String)) throw new IllegalArgumentException("Expected JSON string");
        return (String) value;
    }

    /** Read a JSON boolean without org.json's implicit string coercion. */
    public static Boolean strictBoolean(Object value) {
        return JsonPolicy.strictBoolean(value);
    }

    /** Whether an apply-translations response carries a typed JSON success flag. */
    public static boolean isApplied(Object value) {
        return Boolean.TRUE.equals(strictBoolean(value));
    }

    /** Engine annotations (for example Wubi codes) occupy the shared hint slot first. */
    public static String annotation(
            String engineAnnotation, String translation, boolean glossEnabled) {
        if (engineAnnotation != null && !engineAnnotation.isEmpty()) return engineAnnotation;
        return glossEnabled && validEntry(translation) ? translation : "";
    }

    /**
     * The secondary rows of a Korean Hanja candidate, each drawn on its own line under the Hanja.
     *
     * <p>The 훈음 (for example 나라 이름 한) is the Engine annotation of a Hanja row and is always shown, whatever the gloss and translation preferences say, because it is how a Hanja is told apart from its homophones. A gloss or translation the candidate carries follows on the next row when glosses are on, instead of being displaced by the 훈음 as the shared hint slot would. Display text only: selection commits the candidate text by its index, never these rows.
     */
    public static String hanjaAnnotation(
            String reading, String translation, boolean glossEnabled) {
        String gloss = glossEnabled && validEntry(translation) ? translation : "";
        if (reading == null || reading.isEmpty()) return gloss;
        // 훈음和第一行释义共用一行：另起一行会让韩语的候选条比其他方案多出一行，键盘一切到韩语就整体变高。
        return gloss.isEmpty() ? reading : reading + " · " + gloss;
    }

    public static String hanjaAccessibilitySuffix(
            String reading, String translation, boolean glossEnabled) {
        String suffix = reading == null || reading.isEmpty() ? "" : "，训音：" + reading;
        String gloss = glossEnabled && validEntry(translation) ? translation : "";
        return gloss.isEmpty() ? suffix : suffix + "，释义：" + gloss;
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
            && TextPolicy.utf8Length(value) <= MAX_ENTRY_BYTES;
    }
}

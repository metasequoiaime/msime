package app.msime.android;

/**
 * The host's half of 以词定字, for the candidates the Engine declines.
 *
 * <p>`msime_client_select_edge` handles a candidate that contains Han text and clears the
 * composition. A candidate with none — the English word offered for the same spelling, say — it
 * refuses outright and leaves the composition alone, deliberately manufacturing no fallback. The
 * source resolves that case in its host: it extracts a Han character from the candidate's own
 * text, and when there is not one it commits the whole candidate instead. Both halves are here.
 */
public final class CandidateTextPolicy {
    private CandidateTextPolicy() {}

    /**
     * Whether this scalar is a Han character.
     *
     * <p>Covers 〇 and the unified ideograph blocks including the compatibility block and the two
     * supplementary planes, so a surrogate pair is one character rather than two halves of one.
     */
    public static boolean isHanCodePoint(int value) {
        return value == 0x3007
            || value >= 0x3400 && value <= 0x4dbf
            || value >= 0x4e00 && value <= 0x9fff
            || value >= 0xf900 && value <= 0xfaff
            || value >= 0x20000 && value <= 0x2fa1f
            || value >= 0x30000 && value <= 0x323af;
    }

    /**
     * The candidate's first or last Han character, or null when it has none.
     *
     * <p>"Last" is the last Han scalar in the text, not the last scalar: a candidate ending in a
     * non-Han mark still yields the Han character before it.
     */
    public static String hanCharacter(String text, WordCharacterPolicy.Edge edge) {
        if (text == null || text.isEmpty() || edge == WordCharacterPolicy.Edge.NONE) return null;
        String result = null;
        for (int offset = 0; offset < text.length();) {
            int codePoint = text.codePointAt(offset);
            offset += Character.charCount(codePoint);
            if (!isHanCodePoint(codePoint)) continue;
            result = new String(Character.toChars(codePoint));
            if (edge == WordCharacterPolicy.Edge.FIRST) return result;
        }
        return result;
    }

    /**
     * What the host commits when the Engine declined: the edge character, else the whole candidate.
     *
     * <p>Committing the whole candidate is what the source does when its own `ExtractHanCharacter`
     * comes back empty, rather than swallowing the key press and leaving nothing on screen.
     */
    public static String fallbackCommit(String text, WordCharacterPolicy.Edge edge) {
        if (text == null || text.isEmpty()) return null;
        String character = hanCharacter(text, edge);
        return character != null ? character : text;
    }
}

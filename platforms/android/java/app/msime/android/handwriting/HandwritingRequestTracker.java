package app.msime.android;

/** Rejects delayed recognition work after ink, layout or input-session changes. */
public final class HandwritingRequestTracker {
    public record Token(long generation, long session, long revision) {}

    private long generation;

    public Token begin(long session, long revision) {
        return new Token(++generation, session, revision);
    }

    public void invalidate() { generation++; }

    public boolean accepts(Token token, long session, long revision, boolean handwritingActive) {
        return token != null && handwritingActive && token.generation() == generation
            && token.session() == session && token.revision() == revision;
    }
}

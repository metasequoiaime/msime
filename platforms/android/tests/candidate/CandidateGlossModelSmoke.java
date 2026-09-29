import app.msime.android.CandidateGlossPolicy;

public final class CandidateGlossModelSmoke {
    public static void main(String[] args) throws Exception {
        check(CandidateGlossPolicy.annotation("", "hello", false).isEmpty(), "opt-in display");
        check(CandidateGlossPolicy.annotation("", "hello", true).equals("hello"),
            "gloss display");
        check(CandidateGlossPolicy.accessibilitySuffix("", "hello", true).contains("英文释义"),
            "accessible gloss");
        check(CandidateGlossPolicy.annotation("nihao", "hello", true).equals("nihao"),
            "Engine annotation has priority");
        check(CandidateGlossPolicy.accessibilitySuffix("nihao", "hello", true).contains("提示"),
            "accessible Engine annotation");
        check(CandidateGlossPolicy.annotation("", "x".repeat(4097), true).isEmpty(),
            "oversized gloss hidden");

        CandidateGlossPolicy.Token token = new CandidateGlossPolicy.Token(11, 7, 3);
        check(token.isCurrent(11, 7, 3), "matching token");
        check(!token.isCurrent(11, 8, 3) && !token.isCurrent(12, 7, 3)
            && !token.isCurrent(11, 7, 4), "stale session, generation and epoch rejected");
        expectFailure(() -> new CandidateGlossPolicy.Token(0, 7, 3));
        expectFailure(() -> new CandidateGlossPolicy.Token(11, -1, 3));

        System.out.println("Android candidate gloss model: bounds, priority and stale guards passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void expectFailure(Runnable action) {
        try { action.run(); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("Invalid candidate gloss token accepted");
    }
}

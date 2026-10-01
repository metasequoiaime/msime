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

        // Korean Hanja rows: the 훈음 always shows, on its own row, and a gloss follows it rather than being displaced.
        check(CandidateGlossPolicy.hanjaAnnotation("나라 이름 한", "", false).equals("나라 이름 한"),
            "the 훈음 shows with glosses off");
        check(CandidateGlossPolicy.hanjaAnnotation("나라 이름 한", "Korea", false).equals("나라 이름 한"),
            "a gloss stays hidden while glosses are off");
        check(CandidateGlossPolicy.hanjaAnnotation("나라 이름 한", "Korea", true).equals("나라 이름 한\nKorea"),
            "a gloss follows the 훈음 on the next row");
        check(CandidateGlossPolicy.hanjaAnnotation("나라 이름 한", "Korea\n韓国", true)
                .equals("나라 이름 한\nKorea\n韓国"), "two-language glosses keep their rows under the 훈음");
        check(CandidateGlossPolicy.hanjaAnnotation("", "Korea", true).equals("Korea"),
            "a Hanja without 훈음 shows its gloss alone");
        check(CandidateGlossPolicy.hanjaAnnotation(null, null, true).isEmpty(),
            "a Hanja with neither has no secondary row");
        check(CandidateGlossPolicy.hanjaAnnotation("나라 이름 한", "x".repeat(4097), true).equals("나라 이름 한"),
            "an oversized gloss is hidden under the 훈음");
        check(CandidateGlossPolicy.hanjaAccessibilitySuffix("나라 이름 한", "Korea", true)
                .equals("，训音：나라 이름 한，释义：Korea"), "accessible 훈음 and gloss");
        check(CandidateGlossPolicy.hanjaAccessibilitySuffix("나라 이름 한", "Korea", false)
                .equals("，训音：나라 이름 한"), "accessible 훈음 with glosses off");

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

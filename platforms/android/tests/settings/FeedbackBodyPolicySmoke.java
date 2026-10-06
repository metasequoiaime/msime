import app.msime.android.policy.FeedbackBodyPolicy;

public final class FeedbackBodyPolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("feedback body policy assertion failed");
    }

    public static void main(String[] args) {
        check(FeedbackBodyPolicy.clip(null) == null);
        check(FeedbackBodyPolicy.clip("x".repeat(FeedbackBodyPolicy.MAX_LENGTH)).length()
            == FeedbackBodyPolicy.MAX_LENGTH);
        String splitEmoji = "x".repeat(FeedbackBodyPolicy.MAX_LENGTH - 1) + "😀tail";
        check(FeedbackBodyPolicy.clip(splitEmoji)
            .equals("x".repeat(FeedbackBodyPolicy.MAX_LENGTH - 1)));
    }
}

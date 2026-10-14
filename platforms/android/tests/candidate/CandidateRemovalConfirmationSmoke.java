import app.msime.android.CandidateRemovalConfirmation;

public final class CandidateRemovalConfirmationSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        CandidateRemovalConfirmation confirmation = new CandidateRemovalConfirmation();
        check(!confirmation.armed(), "nothing is armed before the first selection");
        check(!confirmation.confirms(7, 12, 3), "the first selection only arms the candidate");
        check(confirmation.armed(), "the first selection arms the candidate");
        check(confirmation.confirms(7, 12, 3), "a second selection of the same candidate confirms");
        check(!confirmation.armed(), "confirming clears the armed candidate");

        check(!confirmation.confirms(7, 12, 3), "after a removal the same identity arms again");
        check(!confirmation.confirms(7, 12, 4), "another slot re-arms instead of confirming");
        check(confirmation.confirms(7, 12, 4), "the re-armed slot confirms on its second selection");
        check(!confirmation.confirms(7, 13, 4), "a new engine generation re-arms instead of confirming");
        check(!confirmation.confirms(8, 13, 4), "another engine session re-arms instead of confirming");

        confirmation.clear();
        check(!confirmation.armed(), "clearing forgets the armed candidate");
        check(!confirmation.confirms(8, 13, 4), "after a clear the selection arms again");

        System.out.println("Android candidate removal confirmation: arm and confirm passed");
    }
}

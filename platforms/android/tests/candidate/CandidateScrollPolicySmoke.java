import app.msime.android.CandidateScrollPolicy;

public final class CandidateScrollPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        check(CandidateScrollPolicy.changed(-1, -1, -1, 4, 8, 0));
        check(CandidateScrollPolicy.changed(4, 8, 0, 4, 8, 1));
        check(CandidateScrollPolicy.changed(4, 8, 0, 4, 9, 0));
        check(!CandidateScrollPolicy.changed(4, 8, 0, 4, 8, 0));
        System.out.println("Android candidate scroll reset identity passed");
    }
}

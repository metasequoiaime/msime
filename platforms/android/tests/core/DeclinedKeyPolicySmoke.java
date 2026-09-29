import app.msime.android.DeclinedKeyPolicy;

public final class DeclinedKeyPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // A declined mark during composition is an automatic commit: finish first, then insert.
        check(DeclinedKeyPolicy.finishesComposition(true, true));
        // Nothing composing means there is nothing to finish; the host just inserts the mark.
        check(!DeclinedKeyPolicy.finishesComposition(true, false));
        // A declined digit is a candidate key with no chip on this page, not an automatic commit.
        check(!DeclinedKeyPolicy.finishesComposition(false, true));
        check(!DeclinedKeyPolicy.finishesComposition(false, false));
        System.out.println("Android declined key policy: automatic commit boundary passed");
    }
}

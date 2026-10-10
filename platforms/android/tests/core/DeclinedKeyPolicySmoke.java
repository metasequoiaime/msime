import app.msime.android.DeclinedKeyPolicy;

public final class DeclinedKeyPolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    public static void main(String[] args) {
        // A declined mark during composition is an automatic commit: finish first, then insert.
        check(DeclinedKeyPolicy.finishesComposition(true, '@', true));
        // Nothing composing means there is nothing to finish; the host just inserts the mark.
        check(!DeclinedKeyPolicy.finishesComposition(true, '@', false));
        // 数字行的 0、没有候选时的 1–9 被拒：同样先结束组合，免得数字顶掉组字区里的拼音（#6022）。
        check(DeclinedKeyPolicy.finishesComposition(false, '0', true));
        check(DeclinedKeyPolicy.finishesComposition(false, '7', true));
        check(!DeclinedKeyPolicy.finishesComposition(false, '0', false));
        // 被拒的字母不在这条路上，保持原样。
        check(!DeclinedKeyPolicy.finishesComposition(false, 'a', true));
        check(!DeclinedKeyPolicy.finishesComposition(false, 'a', false));
        System.out.println("Android declined key policy: automatic commit boundary passed");
    }
}

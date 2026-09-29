package app.msime.android.test;

import app.msime.android.ChineseHelpcodePolicy;

public final class ChineseHelpcodePolicySmoke {
    private static void check(boolean value) {
        if (!value) throw new AssertionError("Chinese helpcode policy assertion failed");
    }

    public static void main(String[] args) {
        check(ChineseHelpcodePolicy.eligible(false, "ni", 0, "none"));
        check(ChineseHelpcodePolicy.eligible(false, "nih", 1, "none"));
        check(ChineseHelpcodePolicy.entersHelpcode(false, true, "ni", 0, "none"));
        check(!ChineseHelpcodePolicy.entersHelpcode(false, false, "ni", 0, "none"));
        check(!ChineseHelpcodePolicy.eligible(true, "ni", 0, "none"));
        check(!ChineseHelpcodePolicy.eligible(false, "", 0, "none"));
        check(!ChineseHelpcodePolicy.eligible(false, "ni", 2, "none"));
        check(!ChineseHelpcodePolicy.eligible(false, "ni", 3, "none"));
        check(!ChineseHelpcodePolicy.eligible(false, "ni", 0, "unicode"));
        System.out.println("Android Chinese helpcode policy passed");
    }
}

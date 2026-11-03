package app.msime.android;

/** 候选栏页码按共享偏好 `show_candidate_page_number` 显示或隐藏，缺值按开（#10709）。JVM 冒烟里 org.json 只有桩，这里直接传字段原值。 */
public final class CandidatePageLabelPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(CandidatePageLabelPolicy.shown(null), "a missing field keeps the page number");
        check(CandidatePageLabelPolicy.shown("false"), "a string is not coerced to false");
        check(CandidatePageLabelPolicy.shown(0), "a number is not coerced to false");
        check(CandidatePageLabelPolicy.shown(Boolean.TRUE), "true shows the page number");
        check(!CandidatePageLabelPolicy.shown(Boolean.FALSE), "false hides the page number");

        check(CandidatePageLabelPolicy.label(0, 3, true).equals("1/3"), "the first page reads 1/3");
        check(CandidatePageLabelPolicy.label(2, 33, true).equals("3/33"), "pages count from one");
        check(CandidatePageLabelPolicy.label(0, 3, false).isEmpty(), "the switch off draws nothing");
        check(CandidatePageLabelPolicy.label(0, 0, true).isEmpty(), "no page count draws nothing");
        check(CandidatePageLabelPolicy.label(-1, 3, true).isEmpty(), "an invalid page draws nothing");
    }
}

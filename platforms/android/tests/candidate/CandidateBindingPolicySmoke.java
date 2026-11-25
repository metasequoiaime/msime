import app.msime.android.CandidateBindingPolicy;

/** 同一代直接按身份选词；换代后按同一个会话里的文字和来源找回同一个词（#11816）。 */
public final class CandidateBindingPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static CandidateBindingPolicy.Drawn drawn(long session, long generation, long index,
                                                      String text, int source) {
        return new CandidateBindingPolicy.Drawn(session, generation, index, text, source);
    }

    public static void main(String[] args) {
        // 同一代：会话和代次都没换，画出来的身份可以直接交给引擎，不按槽位重新取。
        check(CandidateBindingPolicy.sameGeneration(drawn(4, 8, 1, "拟好", 0), 4, 8), "同一代");
        check(!CandidateBindingPolicy.sameGeneration(drawn(4, 8, 1, "拟好", 0), 4, 9), "换代了");
        check(!CandidateBindingPolicy.sameGeneration(drawn(4, 8, 1, "拟好", 0), 5, 8), "换了会话");
        check(!CandidateBindingPolicy.sameGeneration(drawn(0, 8, 1, "拟好", 0), 0, 8), "没有会话");
        check(!CandidateBindingPolicy.sameGeneration(null, 4, 8), "没有画出来的候选");

        java.util.List<CandidateBindingPolicy.Drawn> reordered = java.util.List.of(
            drawn(4, 9, 0, "你号", 0), drawn(4, 9, 1, "拟好", 0), drawn(4, 9, 2, "你好", 0));
        check(CandidateBindingPolicy.find(reordered, drawn(4, 8, 1, "拟好", 0)) == 1, "换代后按文字找回同一个词");
        check(CandidateBindingPolicy.find(reordered, drawn(4, 8, 2, "你号", 0)) == 0, "找回挪到前面的凑");
        check(CandidateBindingPolicy.find(reordered, drawn(4, 8, 3, "其他", 0))
            == CandidateBindingPolicy.MISSING, "不在列表里的候选不按槽位顶替");

        // 同一个词来自云端算另一个候选：不能把画着的本地词换成云候选上屏。
        check(CandidateBindingPolicy.find(java.util.List.of(drawn(4, 9, 0, "拟好", 2)),
            drawn(4, 8, 1, "拟好", 0)) == CandidateBindingPolicy.MISSING, "换了来源不算同一个候选");

        // 换了会话：整批候选都不再属于这次组字，按文字找同一个词也不成立。
        check(CandidateBindingPolicy.find(java.util.List.of(drawn(5, 1, 0, "拟好", 0)),
            drawn(4, 8, 1, "拟好", 0)) == CandidateBindingPolicy.MISSING, "换了会话不算同一个候选");

        check(CandidateBindingPolicy.find(java.util.List.of(), drawn(4, 8, 1, "拟好", 0))
            == CandidateBindingPolicy.MISSING, "空列表");
        check(CandidateBindingPolicy.find(null, drawn(4, 8, 1, "拟好", 0))
            == CandidateBindingPolicy.MISSING, "没有列表");
        check(CandidateBindingPolicy.find(reordered, null) == CandidateBindingPolicy.MISSING, "没有画出来的候选");
        check(CandidateBindingPolicy.find(reordered, drawn(0, 8, 1, "拟好", 0))
            == CandidateBindingPolicy.MISSING, "没有会话的候选不参与绑定");
        check(CandidateBindingPolicy.find(java.util.Arrays.asList(
            null, drawn(4, 9, 1, "拟好", 0)), drawn(4, 8, 1, "拟好", 0)) == 1, "空项占位不影响后面的索引");
        System.out.println("Android candidate binding passed");
    }
}

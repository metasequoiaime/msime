import app.msime.android.AiPendingCandidatePolicy;

/** 在线占位行（来源 12/15）的画法与响应规则：AI 画淡、云占位隐藏、读屏念状态、长按不弹菜单，其他来源原样（#6888）。 */
public final class AiPendingCandidatePolicySmoke {
    static void check(boolean condition, String message) { if (!condition) throw new AssertionError(message); }

    public static void main(String[] args) {
        check(AiPendingCandidatePolicy.isPlaceholder(12), "source 12 is the AI placeholder");
        check(AiPendingCandidatePolicy.isCloudPending(15), "source 15 is the cloud placeholder");
        // 0 词库、1 用户词、2 云、3 AI、4 英文、10 退役的 NeuralDesktop、11 整句模型：都是真候选。
        for (int source : new int[] {-1, 0, 1, 2, 3, 4, 10, 11, 13}) {
            check(!AiPendingCandidatePolicy.isPlaceholder(source), "source " + source + " is a candidate");
            check(AiPendingCandidatePolicy.alpha(source) == 1f, "source " + source + " drawn opaque");
            check(AiPendingCandidatePolicy.offersMenu(source), "source " + source + " keeps its long press");
            check(AiPendingCandidatePolicy.description(source, "候选 2：", "你好", "，英文 hello")
                .equals("候选 2：你好，英文 hello"), "source " + source + " description unchanged");
        }
        check(AiPendingCandidatePolicy.alpha(15) == 0.45f, "cloud placeholder drawn dimmed");
        check(!AiPendingCandidatePolicy.offersMenu(15), "cloud placeholder has no long-press menu");
        check(!AiPendingCandidatePolicy.isVisible(15), "cloud placeholder is hidden on Android");
        check(AiPendingCandidatePolicy.isVisible(2), "completed cloud candidate remains visible");
        check(AiPendingCandidatePolicy.description(15, "候选 2：", "☁", "")
            .equals("候选 2：云候选加载中"), "cloud placeholder read as loading");
        check(AiPendingCandidatePolicy.alpha(12) == 0.45f, "placeholder drawn dimmed");
        check(!AiPendingCandidatePolicy.offersMenu(12), "placeholder has no long-press menu");
        check(AiPendingCandidatePolicy.description(12, "候选 2：", "…", "；长按管理")
            .equals("候选 2：AI 候选等待中"), "waiting placeholder read as waiting");
        check(AiPendingCandidatePolicy.description(12, "候选 2：", "—", "")
            .equals("候选 2：AI 候选没有结果"), "resolved placeholder read as no result");
        System.out.println("Android AI placeholder presentation passed");
    }
}

package app.msime.android;

/**
 * 候选按钮绑定画出来的那一个（#11816）。候选栏会异步换代：按住期间停顿重排、云或 AI 候选晚到、释义和译文回包都可能让视图换到新的一代，而屏幕上还是上一代。点按、数字键和长按如果按槽位从新视图取候选，就会上屏那格的新词，或者因为身份过期静默丢弃。
 *
 * <p>这里先判断画出来的身份是不是眼前这一代（会话和代次都没换，身份可以直接交给引擎）；换了一代再按文字和来源在这一代里找回同一个词。两者都找不到时返回 {@link #MISSING}，由宿主提示候选已更新，既不按槽位选错，也不静默丢弃。
 */
public final class CandidateBindingPolicy {
    /** 画出来的候选已经不在列表里：宿主用它提示用户重新选择。 */
    public static final String STALE = "候选已更新，请重新选择";
    /** {@link #find} 找不到画出来的候选。 */
    public static final int MISSING = -1;

    private CandidateBindingPolicy() {}

    /** 画出来那一刻的候选：身份、文字和来源。 */
    public record Drawn(long session, long generation, long index, String text, int source) {}

    /** 画出来的候选还是眼前这一代：会话没换、代次没换，这个身份就能直接交给引擎。 */
    public static boolean sameGeneration(Drawn drawn, long session, long generation) {
        return drawn != null && drawn.session() != 0 && drawn.session() == session
            && drawn.generation() == generation;
    }

    /**
     * 换代后在这一代里找回画出来的同一个词，返回它的位置；不在了为 {@link #MISSING}。
     *
     * <p>换了会话整批候选都不再属于这次组字，按文字找同一个词也不该成立，所以文字回退只在同一个会话里进行。
     */
    public static int find(java.util.List<Drawn> current, Drawn drawn) {
        if (current == null || drawn == null || drawn.session() == 0) return MISSING;
        String key = CandidateScrollAnchor.key(drawn.text(), drawn.source());
        for (int slot = 0; slot < current.size(); slot++) {
            Drawn candidate = current.get(slot);
            if (candidate != null && candidate.session() == drawn.session()
                    && key.equals(CandidateScrollAnchor.key(candidate.text(), candidate.source()))) {
                return slot;
            }
        }
        return MISSING;
    }
}

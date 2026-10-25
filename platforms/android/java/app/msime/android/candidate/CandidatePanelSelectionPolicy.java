package app.msime.android;

/**
 * 在展开候选面板里选词之后面板留不留，Android 宿主与 JVM 冒烟共用；iOS（`CandidatePanelSelectionPolicy.swift`）和鸿蒙（`CandidatePanelSelectionPolicy.ts`）按同一条规则判断（#9193）。
 *
 * <p>选词被接受、会话没换、组字还在（只选掉了前面一段）时面板留着，换上剩下组字的完整候选，用户接着在面板里往下选；整句上屏、组字清空或会话换了才收起。剩下的候选只够一页也留着：用户正在面板里逐段选词，收起只会让下一段再点一次展开。
 */
public final class CandidatePanelSelectionPolicy {
    private CandidatePanelSelectionPolicy() {}

    /** 选词之后面板是否留着：{@code editingText} 是选词之后引擎视图里的 `editing_text`。 */
    public static boolean keepsOpen(boolean handled, long selectedSession, long currentSession,
            String editingText) {
        return handled && selectedSession != 0 && selectedSession == currentSession
            && editingText != null && !editingText.isEmpty();
    }

    /**
     * 部分选词后九键面板要发给 `msime_client_set_nine_key_filter` 的笔画前缀，null 表示不必发请求。笔画比的是候选的第一个字，那个字刚被选掉，前缀对剩下的组字不再适用，清空；「只留单字」不动，照旧作用到组字结束，逐字选时不用每段重开。
     */
    public static String strokesAfterPartialSelection(String strokes) {
        return strokes == null || strokes.isEmpty() ? null : "";
    }
}

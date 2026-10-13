package app.msime.android;

/** 保护输入视图延迟刷新，并判断收起视图取消的引擎启动能否在同一编辑器重新显示时恢复。 */
public final class InputViewRefreshPolicy {
    private InputViewRefreshPolicy() {}

    public static boolean shouldRefresh(
            long expectedGeneration,
            long currentGeneration,
            Object expectedConnection,
            Object currentConnection,
            boolean viewValid,
            boolean hasComposition) {
        return expectedConnection != null
            && currentConnection != null
            && expectedGeneration == currentGeneration
            && expectedConnection == currentConnection
            && viewValid
            && !hasComposition;
    }

    public static boolean shouldResumeCancelledStartup(
            long pendingGeneration,
            long currentGeneration,
            long session,
            Object expectedConnection,
            Object currentConnection,
            boolean engineEditor) {
        return pendingGeneration >= 0
            && pendingGeneration != currentGeneration
            && session == 0
            && expectedConnection != null
            && expectedConnection == currentConnection
            && engineEditor;
    }
}

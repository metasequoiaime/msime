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

    /**
     * 焦点落到 TYPE_NULL 的窗口（桌面、设置页这类不打字的窗口也报 TYPE_NULL）而键盘没显示时，引擎会话等 onStartInputView 再建：原来每切一次应用都在后台建一个没人用的会话、再在失焦时销毁，电池统计里全算输入法后台耗电。真要打字的 TYPE_NULL 聊天框会叫出键盘，那时照常建。接着实体键盘时不推迟：实体键盘在键盘没显示时就要经引擎组字（组字后才叫出候选条）。
     */
    public static boolean defersEngineStartup(
            boolean engineWanted, int inputType, boolean inputViewShown, boolean hardwareKeyboardAttached) {
        return engineWanted && inputType == 0 && !inputViewShown && !hardwareKeyboardAttached;
    }

    /** 推迟的引擎启动只在同一次输入开始、同一个连接上接着做；已经有会话或换了输入框的不做。 */
    public static boolean shouldResumeDeferredStartup(
            long deferredGeneration,
            long currentGeneration,
            long session,
            Object expectedConnection,
            Object currentConnection,
            boolean engineEditor) {
        return deferredGeneration >= 0
            && deferredGeneration == currentGeneration
            && session == 0
            && expectedConnection != null
            && expectedConnection == currentConnection
            && engineEditor;
    }

    /**
     * 应用对同一个输入框 restartInput（聊天应用发送后清空输入框就会这样）时能否留着原引擎会话，只在焦点边界上复位，而不是销毁后经工作线程整套重建。
     *
     * <p>输入框必须是同一个（字段、输入类型、imeOptions、包名都相同），引擎里没有组字（有组字时原来的销毁会丢掉它，留着会话会把旧组字带进改写过的文字），没有等着键盘收起时做的隐私重建，而且这个输入框现在该不该学习与会话建立时一致：引擎的 learning 只能在建会话时定（见 restartSessionForPrivacy），不一致时必须重建。
     */
    public static boolean keepsSessionAcrossRestart(
            boolean restarting,
            long session,
            boolean hasView,
            long documentIdentifier,
            boolean sameEditor,
            boolean engineEditor,
            boolean hasComposition,
            boolean privacyRestartPending,
            boolean learningSuppressed,
            boolean sessionLearningSuppressed) {
        return restarting
            && session != 0
            && hasView
            && documentIdentifier != 0
            && sameEditor
            && engineEditor
            && !hasComposition
            && !privacyRestartPending
            && learningSuppressed == sessionLearningSuppressed;
    }
}

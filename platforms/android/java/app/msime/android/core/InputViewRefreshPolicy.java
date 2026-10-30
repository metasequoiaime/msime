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

    /** 主线程上的建会话最多等键盘这次弹出的第一帧这么久（毫秒），从 onStartInputView 算起；窗口刚显示就被收起、视图被重建这类第一帧迟迟不来的情况，到点照常建。 */
    public static final long FIRST_FRAME_WAIT_MILLIS = 500;

    /**
     * onStartInputView 时是否开始等这次弹出的第一帧（#9916）：键盘窗口还没显示（这次是从收起到弹出，键盘要先画出这一帧才看得见：窗口新建或重新布局时系统要等画完的报告才把它亮出来，API 30 起进程还活着时键盘由插入动画显隐，也要先有这一帧的内容），而且键盘视图已经建好。窗口本来就显示着（同一应用里换输入框）时不等：那时没有新的一帧必然到来，等只会白白推迟会话。
     */
    public static boolean armsFirstFrameWait(boolean windowVisible, boolean inputViewCreated) {
        return !windowVisible && inputViewCreated;
    }

    /**
     * 工作线程把建会话交回主线程时是否先等第一帧（#9916）。建会话（`NativeClient.create`、按实时偏好应用快照、重画）在主线程上，会话句柄绑定线程，挪不走；新打开的应用里点输入框时 onStartInput 和弹出键盘几乎同时到，工作线程读完偏好、同步完词库常常赶在键盘第一帧之前，这一整段就插在第一帧前面，键盘晚这么久才出来。第一帧交出去（帧提交回调）后马上建，用户在键盘出来之前本来也点不了键。
     *
     * @param armedAt 开始等第一帧的时刻（uptime 毫秒），没有在等时为负
     */
    public static boolean waitsForFirstFrame(long armedAt, long now) {
        return armedAt >= 0 && now - armedAt < FIRST_FRAME_WAIT_MILLIS;
    }

    /** 还要等第一帧多久（毫秒）再照常建会话，不为负。 */
    public static long firstFrameWaitRemaining(long armedAt, long now) {
        return Math.max(0, FIRST_FRAME_WAIT_MILLIS - (now - armedAt));
    }
}

package app.msime.android;

/**
 * 开发者选项「上传日志供开发者通过 MCP 读取」开关：显示成什么、点了之后做什么。
 *
 * <p>打开开关不会立即上传，而是先在「确认上传」组里等用户点「上传」。等待确认的这段时间开关显示为打开，再点一次就是取消；云端已有快照时开关一直是打开的，点它关闭是删除云端快照（要再确认一次）。以前等待确认时开关仍显示为关闭，点了只是又进一次等待确认，看起来毫无反应（#5539）。
 */
public final class McpUploadSwitchPolicy {
    /** 点开关之后要做的事。 */
    public enum Action {
        /** 进入「确认上传」。 */
        START_CONFIRM,
        /** 退出「确认上传」，什么也不上传。 */
        CANCEL_CONFIRM,
        /** 删除云端快照（先弹确认框）。 */
        DELETE,
    }

    private McpUploadSwitchPolicy() {}

    /** 开关显示为打开：云端已有快照，或正在等待确认上传。 */
    public static boolean checked(boolean uploaded, boolean confirming) {
        return uploaded || confirming;
    }

    /** 开关切换到 `on` 之后要做的事；`on` 是切换后的状态。 */
    public static Action onToggle(boolean on, boolean confirming) {
        if (on) return Action.START_CONFIRM;
        return confirming ? Action.CANCEL_CONFIRM : Action.DELETE;
    }
}

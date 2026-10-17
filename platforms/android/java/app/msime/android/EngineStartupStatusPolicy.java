package app.msime.android;

/** 决定引擎异步启动期间是否把内部状态显示到键盘状态行。 */
public final class EngineStartupStatusPolicy {
    private EngineStartupStatusPolicy() {}

    /** 引擎启动在后台进行，不显示会改变键盘高度的临时状态行。 */
    public static String initialMessage(boolean engineWanted) {
        return engineWanted ? "" : "直接输入";
    }

    /** 重建引擎会话时同样保持键盘高度稳定。 */
    public static String restartingMessage() {
        return "";
    }
}

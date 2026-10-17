package app.msime.android;

/** 回复模板读取结果只有在同一代且回复面板仍可见时才能写回界面。 */
public final class ReplyTemplateLifecyclePolicy {
    private ReplyTemplateLifecyclePolicy() {}

    public static boolean accepts(int expectedGeneration, int currentGeneration,
                                  boolean replyOpen, boolean panelVisible) {
        return expectedGeneration == currentGeneration && replyOpen && panelVisible;
    }
}

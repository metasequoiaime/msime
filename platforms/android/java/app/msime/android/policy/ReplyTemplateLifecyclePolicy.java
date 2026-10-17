package app.msime.android;

/** 回复模板读取结果只有在同一代且回复面板仍可见时才能写回界面。 */
public final class ReplyTemplateLifecyclePolicy {
    private ReplyTemplateLifecyclePolicy() {}

    public record Generations(int replyRead, int menuRead) {}

    public static boolean accepts(int expectedGeneration, int currentGeneration,
                                  boolean replyOpen, boolean panelVisible) {
        return expectedGeneration == currentGeneration && replyOpen && panelVisible;
    }

    public static int advance(int generation) {
        return generation == Integer.MAX_VALUE ? 0 : generation + 1;
    }

    public static Generations advanceOnPanelClose(int replyRead, int menuRead) {
        return new Generations(advance(replyRead), advance(menuRead));
    }
}

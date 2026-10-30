package app.msime.android;

/** 符号集插件读取结果只有在同一代、面板仍可见且偏好目录未变时才能写回。 */
public final class PluginSymbolLifecyclePolicy {
    public static final long MAX_GENERATION = Long.MAX_VALUE;

    private PluginSymbolLifecyclePolicy() {}

    public static boolean accepts(long expectedGeneration, long currentGeneration,
                                  boolean panelVisible, String expectedDirectory,
                                  String currentDirectory) {
        return expectedGeneration == currentGeneration
            && panelVisible
            && expectedDirectory != null && !expectedDirectory.isEmpty()
            && expectedDirectory.equals(currentDirectory);
    }

    public static long advance(long generation) {
        return generation == MAX_GENERATION ? 0 : generation + 1;
    }
}

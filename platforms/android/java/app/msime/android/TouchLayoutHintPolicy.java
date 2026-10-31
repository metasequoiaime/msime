package app.msime.android;

/** 决定引擎实时偏好尚未到达时，是否继续采用已记住的触屏键面。 */
public final class TouchLayoutHintPolicy {
    private TouchLayoutHintPolicy() {}

    /**
     * 选择当前要画的键面。
     *
     * <p>运行时启动缓存里的 26 键视图可能早于实时偏好到达。全拼和双拼的这个视图不能
     * 覆盖上一次已经确认的九键；其余方案、非默认网格和本地模式则已有足够上下文，应以
     * 引擎返回的键面为准。实时偏好一到，视图始终具有最高优先级。
     */
    public static int choose(int remembered, int runtime, boolean viewPresent,
            boolean preferencesReady, boolean dedicatedEnglish, boolean localMode, int scheme) {
        if (dedicatedEnglish) return KeyboardLayout.STANDARD_TOUCH_LAYOUT;
        if (!viewPresent) return remembered;
        if (preferencesReady || runtime != KeyboardLayout.STANDARD_TOUCH_LAYOUT
                || localMode || (InputSchemeTraits.known(scheme)
                    && !InputSchemeTraits.opensLocalModes(scheme))) return runtime;
        return remembered;
    }
}

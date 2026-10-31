package app.msime.android;

/**
 * 首帧布局诊断（#10389）：把建会话时偏好的来源和各阶段画的触屏布局换成固定的 {@link DiagnosticEventKind}。
 *
 * <p>诊断包的输入事件只认 `{t_ms, kind}`，多一个键整行丢弃，所以布局编进种类名里。这里只接受布尔值和 {@link KeyboardLayout} 的整数常量，种类名由固定片段拼出，再经 {@link DiagnosticEventKind#fromWire} 换回枚举，不认识的布局给 null、不记，不会带出任何输入内容。
 */
public final class TouchLayoutDiagnostics {
    private TouchLayoutDiagnostics() {}

    /** 记布局的时机。 */
    public enum Stage {
        /** 建会话前键行已经按什么布局画着：会话没到时按记住的布局画，用户在切输入法时先看到的就是它。窗口没显示时不记，那时键行是上一个输入框留下的，用户没看到过。 */
        FIRST_DRAWN("first_drawn"),
        /** 会话刚建好时引擎视图给出的布局，由建会话用的偏好决定。 */
        CREATED("created"),
        /** 这个会话第一次应用上实时偏好之后画的布局：建会话时就应用上了在建会话里记，否则在 preferencesReloader 之后第一次有快照的那次重读里记；每个会话只记一次。 */
        LIVE("live");

        private final String wire;

        Stage(String wire) {
            this.wire = wire;
        }
    }

    /**
     * 建会话用的偏好从哪来。
     *
     * @param livePreferences 建会话前在工作线程上读到了实时偏好
     * @param hinted 没读到实时偏好，出厂副本的 `touch_keyboard_layout` 已按记住的布局改过（{@link TouchLayoutHintPolicy}）
     */
    public static DiagnosticEventKind sessionPreferences(boolean livePreferences, boolean hinted) {
        if (livePreferences) return DiagnosticEventKind.ENGINE_SESSION_LIVE_PREFERENCES;
        return hinted ? DiagnosticEventKind.ENGINE_SESSION_HINTED_PREFERENCES
            : DiagnosticEventKind.ENGINE_SESSION_FACTORY_PREFERENCES;
    }

    /** 某个时机画的触屏布局；布局不是已知的 {@link KeyboardLayout} 常量时返回 null，调用方不记。 */
    public static DiagnosticEventKind layout(Stage stage, int layout) {
        String name = layoutName(layout);
        if (stage == null || name == null) return null;
        return DiagnosticEventKind.fromWire("touch_layout_" + stage.wire + "_" + name);
    }

    /** 触屏布局常量在种类名里的写法；英文模式画的是 26 键，记作 `twenty_six_key`。 */
    static String layoutName(int layout) {
        return switch (layout) {
            case KeyboardLayout.STANDARD_TOUCH_LAYOUT -> "twenty_six_key";
            case KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT -> "nine_key";
            case KeyboardLayout.JAPANESE_NINE_KEY_LAYOUT -> "japanese_nine_key";
            case KeyboardLayout.HANDWRITING_LAYOUT -> "handwriting";
            case KeyboardLayout.KOREAN_LAYOUT -> "korean";
            case KeyboardLayout.ZHUYIN_LAYOUT -> "zhuyin";
            case KeyboardLayout.STROKE_LAYOUT -> "stroke";
            case KeyboardLayout.ZHUYIN_NINE_KEY_LAYOUT -> "zhuyin_nine_key";
            case KeyboardLayout.FOURTEEN_KEY_LAYOUT -> "fourteen_key";
            case KeyboardLayout.EIGHTEEN_KEY_LAYOUT -> "eighteen_key";
            default -> null;
        };
    }
}

package app.msime.android;

/** 固定诊断事件；本地失败类别只进入手动导出的诊断包。 */
public enum DiagnosticEventKind {
    KEY_DOWN("key_down"),
    KEY_UP("key_up"),
    CANDIDATE_SHOWN("candidate_shown"),
    CANDIDATE_SELECTED("candidate_selected"),
    COMMIT("commit"),
    BACKSPACE("backspace"),
    PANEL_OPEN("panel_open"),
    PANEL_CLOSE("panel_close"),
    IME_START("ime_start"),
    IME_FINISH("ime_finish"),
    INPUT_FAILURE_RUNTIME_REJECTED("input_failure_runtime_rejected", false),
    INPUT_FAILURE_INVALID_RESPONSE("input_failure_invalid_response", false),
    INPUT_FAILURE_EDITOR_REJECTED("input_failure_editor_rejected", false),
    INPUT_FAILURE_JNI_LINK("input_failure_jni_link", false);

    private final String wire;

    private final boolean uploadable;

    DiagnosticEventKind(String wire) { this(wire, true); }
    DiagnosticEventKind(String wire, boolean uploadable) {
        this.wire = wire;
        this.uploadable = uploadable;
    }

    public boolean uploadable() { return uploadable; }

    public String wire() { return wire; }

    /** 不认识的类别直接丢弃，不接受自由文本。 */
    public static DiagnosticEventKind fromWire(String value) {
        for (DiagnosticEventKind kind : values()) {
            if (kind.wire.equals(value)) return kind;
        }
        return null;
    }
}

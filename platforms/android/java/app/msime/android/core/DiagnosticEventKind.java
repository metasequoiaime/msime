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
    INPUT_FAILURE_JNI_LINK("input_failure_jni_link", false),
    VOICE_ENGINE_LOCAL("voice_engine_local", false),
    VOICE_ENGINE_STREAMING("voice_engine_streaming", false),
    VOICE_ENGINE_PLATFORM("voice_engine_platform", false),
    VOICE_ENGINE_HTTP("voice_engine_http", false),
    VOICE_ACTIVITY_LAUNCH("voice_activity_launch", false),
    VOICE_LOCAL_MODEL_CHECK("voice_local_model_check", false),
    VOICE_LOCAL_RUNTIME_CHECK("voice_local_runtime_check", false),
    VOICE_LOCAL_RECORDER_OPEN("voice_local_recorder_open", false),
    VOICE_LOCAL_NATIVE_START("voice_local_native_start", false),
    VOICE_LOCAL_CAPTURE_START("voice_local_capture_start", false),
    VOICE_LOCAL_CAPTURE_READ_FAILURE("voice_local_capture_read_failure", false),
    VOICE_LOCAL_CAPTURE_START_FAILURE("voice_local_capture_start_failure", false),
    VOICE_LOCAL_CAPTURE_EXCEPTION("voice_local_capture_exception", false),
    VOICE_FAILURE_PERMISSION("voice_failure_permission", false),
    VOICE_FAILURE_AUDIO("voice_failure_audio", false),
    VOICE_FAILURE_MODEL("voice_failure_model", false),
    VOICE_FAILURE_RUNTIME_MISSING("voice_failure_runtime_missing", false),
    VOICE_FAILURE_RUNTIME("voice_failure_runtime", false),
    VOICE_CANCELLED("voice_cancelled", false),
    VOICE_EMPTY("voice_empty", false),
    VOICE_SUCCESS("voice_success", false),
    VOICE_FAILURE_SECURITY("voice_failure_security", false),
    VOICE_FAILURE_LINK("voice_failure_link", false),
    VOICE_FAILURE_NULL_POINTER("voice_failure_null_pointer", false),
    VOICE_FAILURE_UNEXPECTED("voice_failure_unexpected", false),
    VOICE_FAILURE_NETWORK("voice_failure_network", false),
    VOICE_PLATFORM_SERVER("voice_platform_server", false),
    VOICE_PLATFORM_CLIENT("voice_platform_client", false),
    VOICE_PLATFORM_BUSY("voice_platform_busy", false),
    VOICE_PLATFORM_UNAVAILABLE("voice_platform_unavailable", false),
    VOICE_PLATFORM_OTHER("voice_platform_other", false),
    VOICE_FAILURE_RESPONSE("voice_failure_response", false);

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

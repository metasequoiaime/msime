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
    VOICE_FAILURE_RESPONSE("voice_failure_response", false),
    /** 本机这一侧等服务回应超时（连接、握手或等最终结果）；服务端报的 408/504 仍记 {@link #VOICE_FAILURE_RESPONSE}。追加在末尾：识别窗口跨进程只传序号。 */
    VOICE_FAILURE_TIMEOUT("voice_failure_timeout", false),
    // 引擎会话建立的耗时（#10086），只进本机导出的诊断包的性能记录：主线程上建会话本身，和从启动排上到会话可用的整段等待。追加在 VOICE_FAILURE_TIMEOUT 之后，不改已有种类的序号。
    ENGINE_SESSION_CREATE("engine_session_create", false),
    ENGINE_SESSION_READY("engine_session_ready", false),
    // 键盘每次弹出各阶段的耗时（#9916），同样只进本机导出的诊断包：键盘进程冷启动的 onCreate、建键盘视图、从 onStartInputView 到这次弹出的第一帧交出去（API 29 起在帧提交回调里记，即 RenderThread 画完并交出这一帧之后；API 28 没有这个回调，在下一帧回调里记，是近似值）、建会话前工作线程上读实时偏好和同步个人词库、主线程上建会话到第一次画完的整段，以及换输入框时主线程上销毁旧会话。
    IME_SERVICE_CREATE("ime_service_create", false),
    INPUT_VIEW_CREATE("input_view_create", false),
    KEYBOARD_FIRST_DRAW("keyboard_first_draw", false),
    ENGINE_STARTUP_PREFERENCES("engine_startup_preferences", false),
    ENGINE_STARTUP_DICTIONARY_SYNC("engine_startup_dictionary_sync", false),
    ENGINE_SESSION_MAIN("engine_session_main", false),
    ENGINE_SESSION_DESTROY("engine_session_destroy", false),
    // 首帧布局诊断（#10389），只进本机导出的诊断包的输入事件：建会话时用的是实时偏好、出厂偏好副本，还是按记住的九键改过的副本；会话建好前键行画的触屏布局、会话建好时引擎视图给出的布局，以及实时偏好应用后画的布局。布局只取 KeyboardLayout 的固定常量，经 TouchLayoutDiagnostics 换成种类，不带任何输入内容。追加在 ENGINE_SESSION_DESTROY 之后，不改已有种类的序号。
    ENGINE_SESSION_LIVE_PREFERENCES("engine_session_live_preferences", false),
    ENGINE_SESSION_FACTORY_PREFERENCES("engine_session_factory_preferences", false),
    ENGINE_SESSION_HINTED_PREFERENCES("engine_session_hinted_preferences", false),
    TOUCH_LAYOUT_FIRST_DRAWN_TWENTY_SIX_KEY("touch_layout_first_drawn_twenty_six_key", false),
    TOUCH_LAYOUT_FIRST_DRAWN_NINE_KEY("touch_layout_first_drawn_nine_key", false),
    TOUCH_LAYOUT_FIRST_DRAWN_JAPANESE_NINE_KEY("touch_layout_first_drawn_japanese_nine_key", false),
    TOUCH_LAYOUT_FIRST_DRAWN_HANDWRITING("touch_layout_first_drawn_handwriting", false),
    TOUCH_LAYOUT_FIRST_DRAWN_KOREAN("touch_layout_first_drawn_korean", false),
    TOUCH_LAYOUT_FIRST_DRAWN_ZHUYIN("touch_layout_first_drawn_zhuyin", false),
    TOUCH_LAYOUT_FIRST_DRAWN_STROKE("touch_layout_first_drawn_stroke", false),
    TOUCH_LAYOUT_FIRST_DRAWN_ZHUYIN_NINE_KEY("touch_layout_first_drawn_zhuyin_nine_key", false),
    TOUCH_LAYOUT_FIRST_DRAWN_FOURTEEN_KEY("touch_layout_first_drawn_fourteen_key", false),
    TOUCH_LAYOUT_FIRST_DRAWN_EIGHTEEN_KEY("touch_layout_first_drawn_eighteen_key", false),
    TOUCH_LAYOUT_CREATED_TWENTY_SIX_KEY("touch_layout_created_twenty_six_key", false),
    TOUCH_LAYOUT_CREATED_NINE_KEY("touch_layout_created_nine_key", false),
    TOUCH_LAYOUT_CREATED_JAPANESE_NINE_KEY("touch_layout_created_japanese_nine_key", false),
    TOUCH_LAYOUT_CREATED_HANDWRITING("touch_layout_created_handwriting", false),
    TOUCH_LAYOUT_CREATED_KOREAN("touch_layout_created_korean", false),
    TOUCH_LAYOUT_CREATED_ZHUYIN("touch_layout_created_zhuyin", false),
    TOUCH_LAYOUT_CREATED_STROKE("touch_layout_created_stroke", false),
    TOUCH_LAYOUT_CREATED_ZHUYIN_NINE_KEY("touch_layout_created_zhuyin_nine_key", false),
    TOUCH_LAYOUT_CREATED_FOURTEEN_KEY("touch_layout_created_fourteen_key", false),
    TOUCH_LAYOUT_CREATED_EIGHTEEN_KEY("touch_layout_created_eighteen_key", false),
    TOUCH_LAYOUT_LIVE_TWENTY_SIX_KEY("touch_layout_live_twenty_six_key", false),
    TOUCH_LAYOUT_LIVE_NINE_KEY("touch_layout_live_nine_key", false),
    TOUCH_LAYOUT_LIVE_JAPANESE_NINE_KEY("touch_layout_live_japanese_nine_key", false),
    TOUCH_LAYOUT_LIVE_HANDWRITING("touch_layout_live_handwriting", false),
    TOUCH_LAYOUT_LIVE_KOREAN("touch_layout_live_korean", false),
    TOUCH_LAYOUT_LIVE_ZHUYIN("touch_layout_live_zhuyin", false),
    TOUCH_LAYOUT_LIVE_STROKE("touch_layout_live_stroke", false),
    TOUCH_LAYOUT_LIVE_ZHUYIN_NINE_KEY("touch_layout_live_zhuyin_nine_key", false),
    TOUCH_LAYOUT_LIVE_FOURTEEN_KEY("touch_layout_live_fourteen_key", false),
    TOUCH_LAYOUT_LIVE_EIGHTEEN_KEY("touch_layout_live_eighteen_key", false),
    // 按键路径拆段计时（#10457），只进本机导出的诊断包的性能记录：engine_call 是一次按键在主线程上同步调引擎（JNI 进出）的耗时，candidate_rendered 是从按键到 render() 把候选和键盘视图改完的耗时（不含之后这一帧的测量、布局和绘制）。candidate_shown 照旧是按键到引擎结果写进输入框、还没开始 render 的那一刻。追加在末尾，不改已有种类的序号。
    ENGINE_CALL("engine_call", false),
    CANDIDATE_RENDERED("candidate_rendered", false);

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

package app.msime.android;

import org.json.JSONException;

/** 输入失败只保留固定类别，不读取可能带有输入内容的异常说明或堆栈。 */
final class InputFailurePolicy {
    enum Reason {
        RUNTIME_REJECTED("runtime_rejected"),
        INVALID_RESPONSE("invalid_response"),
        EDITOR_REJECTED("editor_rejected"),
        JNI_LINK("jni_link");

        private final String code;

        Reason(String code) { this.code = code; }
    }

    /** 仍属于原来的 JSON 捕获范围，只额外保存来源枚举。 */
    static final class Failure extends JSONException {
        private static final long serialVersionUID = 1L;
        private final Reason reason;

        Failure(Reason reason) {
            super(reason.code);
            this.reason = reason;
        }
    }

    private InputFailurePolicy() {}

    private static Reason reason(Throwable error) {
        return error instanceof Failure failure ? failure.reason
            : error instanceof LinkageError ? Reason.JNI_LINK : Reason.INVALID_RESPONSE;
    }

    static String logMessage(Throwable error) {
        return "input_failure reason=" + reason(error).code;
    }

    static DiagnosticEventKind eventKind(Throwable error) {
        return switch (reason(error)) {
            case RUNTIME_REJECTED -> DiagnosticEventKind.INPUT_FAILURE_RUNTIME_REJECTED;
            case INVALID_RESPONSE -> DiagnosticEventKind.INPUT_FAILURE_INVALID_RESPONSE;
            case EDITOR_REJECTED -> DiagnosticEventKind.INPUT_FAILURE_EDITOR_REJECTED;
            case JNI_LINK -> DiagnosticEventKind.INPUT_FAILURE_JNI_LINK;
        };
    }
}

package app.msime.android;

/** 语音诊断只传固定类别，不接收异常消息、路径、识别文本或音频。 */
final class VoiceDiagnostics {
    private VoiceDiagnostics() { }

    static DiagnosticEventKind fromCode(int code) {
        DiagnosticEventKind[] kinds = DiagnosticEventKind.values();
        if (code < 0 || code >= kinds.length) return null;
        DiagnosticEventKind kind = kinds[code];
        return kind.wire().startsWith("voice_") ? kind : null;
    }

    static DiagnosticEventKind unexpected(Throwable error) {
        if (error instanceof SecurityException) return DiagnosticEventKind.VOICE_FAILURE_SECURITY;
        if (error instanceof LinkageError) return DiagnosticEventKind.VOICE_FAILURE_LINK;
        if (error instanceof NullPointerException) return DiagnosticEventKind.VOICE_FAILURE_NULL_POINTER;
        return DiagnosticEventKind.VOICE_FAILURE_UNEXPECTED;
    }

    static DiagnosticEventKind platformFailure(int error) {
        return switch (error) {
            // 1 是 ERROR_NETWORK_TIMEOUT：提示说的是「响应超时」，诊断也记超时，不和 2（网络错误）混在一起。
            case 1 -> DiagnosticEventKind.VOICE_FAILURE_TIMEOUT;
            case 2 -> DiagnosticEventKind.VOICE_FAILURE_NETWORK;
            case 3 -> DiagnosticEventKind.VOICE_FAILURE_AUDIO;
            case 4, 10, 11 -> DiagnosticEventKind.VOICE_PLATFORM_SERVER;
            case 5 -> DiagnosticEventKind.VOICE_PLATFORM_CLIENT;
            case 6, 7 -> DiagnosticEventKind.VOICE_EMPTY;
            case 8 -> DiagnosticEventKind.VOICE_PLATFORM_BUSY;
            case 9 -> DiagnosticEventKind.VOICE_FAILURE_PERMISSION;
            case 12, 13 -> DiagnosticEventKind.VOICE_PLATFORM_UNAVAILABLE;
            default -> DiagnosticEventKind.VOICE_PLATFORM_OTHER;
        };
    }
}

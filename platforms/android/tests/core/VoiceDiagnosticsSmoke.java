package app.msime.android;

/** 错误码与异常只映射成有限类别，IPC 不能借此写入普通按键或越界类别。 */
public final class VoiceDiagnosticsSmoke {
    public static void main(String[] args) {
        check(VoiceDiagnostics.platformFailure(9) == DiagnosticEventKind.VOICE_FAILURE_PERMISSION);
        check(VoiceDiagnostics.platformFailure(3) == DiagnosticEventKind.VOICE_FAILURE_AUDIO);
        check(VoiceDiagnostics.platformFailure(8) == DiagnosticEventKind.VOICE_PLATFORM_BUSY);
        check(VoiceDiagnostics.platformFailure(1) == DiagnosticEventKind.VOICE_FAILURE_NETWORK);
        check(VoiceDiagnostics.platformFailure(7) == DiagnosticEventKind.VOICE_EMPTY);
        check(VoiceDiagnostics.platformFailure(-1) == DiagnosticEventKind.VOICE_PLATFORM_OTHER);
        check(VoiceDiagnostics.unexpected(new SecurityException("synthetic-private-marker"))
            == DiagnosticEventKind.VOICE_FAILURE_SECURITY);
        check(VoiceDiagnostics.unexpected(new UnsatisfiedLinkError("synthetic-private-marker"))
            == DiagnosticEventKind.VOICE_FAILURE_LINK);
        check(VoiceDiagnostics.unexpected(new NullPointerException("synthetic-private-marker"))
            == DiagnosticEventKind.VOICE_FAILURE_NULL_POINTER);
        check(VoiceDiagnostics.unexpected(new IllegalStateException("synthetic-private-marker"))
            == DiagnosticEventKind.VOICE_FAILURE_UNEXPECTED);
        for (LocalAsrRecognizer.Failure failure : LocalAsrRecognizer.Failure.values()) {
            LocalAsrRecognizer.Refused refused = new LocalAsrRecognizer.Refused(failure);
            DiagnosticEventKind expected = switch (failure) {
                case PERMISSION -> DiagnosticEventKind.VOICE_FAILURE_PERMISSION;
                case UNAVAILABLE -> DiagnosticEventKind.VOICE_FAILURE_AUDIO;
                case MODEL -> DiagnosticEventKind.VOICE_FAILURE_MODEL;
                case RUNTIME -> DiagnosticEventKind.VOICE_FAILURE_RUNTIME;
                case CANCELLED -> DiagnosticEventKind.VOICE_CANCELLED;
                case EMPTY -> DiagnosticEventKind.VOICE_EMPTY;
            };
            check(refused.diagnosticKind() == expected);
        }
        check(VoiceDiagnostics.fromCode(-1) == null);
        check(VoiceDiagnostics.fromCode(Integer.MAX_VALUE) == null);
        for (DiagnosticEventKind kind : DiagnosticEventKind.values()) {
            check(VoiceDiagnostics.fromCode(kind.ordinal()) ==
                (kind.wire().startsWith("voice_") ? kind : null));
            if (kind.wire().startsWith("voice_")) {
                check(!kind.uploadable());
                check(!ImeDebugOverlay.EventLog.line(kind, 1).contains("synthetic-private-marker"));
            }
        }
        System.out.println("VoiceDiagnosticsSmoke passed");
    }

    private static void check(boolean value) {
        if (!value) throw new AssertionError("fixed voice diagnostic category");
    }
}

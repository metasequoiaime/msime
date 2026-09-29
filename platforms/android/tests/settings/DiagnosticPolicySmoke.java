import app.msime.android.InputDiagnosticPolicy;

public final class DiagnosticPolicySmoke {
    private static void check(boolean condition) {
        if (!condition) throw new AssertionError("diagnostic policy assertion failed");
    }

    public static void main(String[] args) {
        check(InputDiagnosticPolicy.normalize(null).isEmpty());
        check(InputDiagnosticPolicy.normalize(" \n\t").isEmpty());
        check(InputDiagnosticPolicy.normalize("  当前候选不支持此操作  ")
            .equals("当前候选不支持此操作"));
        String bounded = InputDiagnosticPolicy.normalize("x".repeat(
            InputDiagnosticPolicy.MAX_LENGTH + 20));
        check(bounded.length() == InputDiagnosticPolicy.MAX_LENGTH);
        check(bounded.endsWith("…"));
        check(InputDiagnosticPolicy.visible("提示"));
        check(!InputDiagnosticPolicy.visible("   "));
        check(InputDiagnosticPolicy.DISMISS_DELAY_MILLIS == 4_000L);
    }
}

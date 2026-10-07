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
        String splitEmoji = "x".repeat(InputDiagnosticPolicy.MAX_LENGTH - 2) + "😀tail";
        String boundedEmoji = InputDiagnosticPolicy.normalize(splitEmoji);
        check(boundedEmoji.equals("x".repeat(InputDiagnosticPolicy.MAX_LENGTH - 2) + "…"));
        check(InputDiagnosticPolicy.visible("提示"));
        check(!InputDiagnosticPolicy.visible("   "));
        check(InputDiagnosticPolicy.DISMISS_DELAY_MILLIS == 4_000L);
        // 未能保存学习的诊断不在键盘上显示，数据库不可用的说明译成中文，已是中文的照旧。
        check(!InputDiagnosticPolicy.visible("English word could not be learned."));
        check(!InputDiagnosticPolicy.visible(" Unable to persist nine-key candidate position. "));
        check(InputDiagnosticPolicy.normalize("Emoji database is unavailable.").equals("表情数据不可用"));
        check(InputDiagnosticPolicy.normalize("九键最多输入 32 位，请先选择候选").equals("九键最多输入 32 位，请先选择候选"));
    }
}

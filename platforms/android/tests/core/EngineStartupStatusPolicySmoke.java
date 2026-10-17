import app.msime.android.EngineStartupStatusPolicy;

public final class EngineStartupStatusPolicySmoke {
    public static void main(String[] arguments) {
        check(EngineStartupStatusPolicy.initialMessage(true).isEmpty(),
            "engine startup must stay out of the visible status row");
        check("直接输入".equals(EngineStartupStatusPolicy.initialMessage(false)),
            "direct-input editors keep their status label");
        check(EngineStartupStatusPolicy.restartingMessage().isEmpty(),
            "session restart must not change keyboard height");
        System.out.println("Android engine startup status policy passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

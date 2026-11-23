import app.msime.android.policy.CompositionStopPolicy;

/** 停下会话时对还在组字区里的按键发哪条命令：原样上屏只留给组字区里本来就是文字的方案。 */
public final class CompositionStopPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static void expect(String name, boolean finish, boolean keyLetters, int command) {
        check(CompositionStopPolicy.command(finish, keyLetters) == command, name);
    }

    public static void main(String[] args) {
        check(CompositionStopPolicy.NO_COMMAND == 0 && CompositionStopPolicy.COMMIT_RAW == 2
                && CompositionStopPolicy.FINISH == 9,
            "host command numbers stay the shared ones (2 CommitRaw, 9 Action::Finish)");
        expect("stopping without finishing writes nothing", false, false, CompositionStopPolicy.NO_COMMAND);
        expect("stopping without finishing drops key letters", false, true, CompositionStopPolicy.NO_COMMAND);
        expect("written text is still committed raw", true, false, CompositionStopPolicy.COMMIT_RAW);
        expect("key letters end at the highlighted candidate", true, true, CompositionStopPolicy.FINISH);
        System.out.println("Android composition stop: key letters never reach the editor as raw text passed");
    }
}

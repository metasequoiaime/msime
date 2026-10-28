import app.msime.android.InputViewRefreshPolicy;

public final class InputViewRefreshPolicySmoke {
    private static final class Connection {}

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        Connection connection = new Connection();
        check(InputViewRefreshPolicy.shouldRefresh(7, 7, connection, connection, true, false),
            "matching editor view should refresh");
        check(!InputViewRefreshPolicy.shouldRefresh(6, 7, connection, connection, true, false),
            "stale input generation must not refresh");
        check(!InputViewRefreshPolicy.shouldRefresh(7, 7, connection, new Connection(), true, false),
            "changed input connection must not refresh");
        check(!InputViewRefreshPolicy.shouldRefresh(7, 7, connection, connection, false, false),
            "detached input view must not refresh");
        check(!InputViewRefreshPolicy.shouldRefresh(7, 7, connection, connection, true, true),
            "active composition must not be interrupted");
        check(!InputViewRefreshPolicy.shouldRefresh(7, 7, null, null, true, false),
            "missing input connection must not refresh");
        check(InputViewRefreshPolicy.shouldResumeCancelledStartup(
                7, 8, 0, connection, connection, true),
            "showing the same editor must restart a startup cancelled when its view hid");
        check(!InputViewRefreshPolicy.shouldResumeCancelledStartup(
                8, 8, 0, connection, connection, true),
            "a current startup must not be queued twice");
        check(!InputViewRefreshPolicy.shouldResumeCancelledStartup(
                -1, 8, 0, connection, connection, true),
            "a failed or stopped startup must not retry on every show");
        check(!InputViewRefreshPolicy.shouldResumeCancelledStartup(
                7, 8, 3, connection, connection, true),
            "an active session must not be replaced");
        check(!InputViewRefreshPolicy.shouldResumeCancelledStartup(
                7, 8, 0, connection, new Connection(), true),
            "a different editor connection must not inherit the old startup");
        check(!InputViewRefreshPolicy.shouldResumeCancelledStartup(
                7, 8, 0, connection, connection, false),
            "a direct-input editor must not create a session");
        int typeNull = 0;
        int text = 1;
        check(InputViewRefreshPolicy.defersEngineStartup(true, typeNull, false, false),
            "a TYPE_NULL window with the keyboard hidden must not build a session yet");
        check(!InputViewRefreshPolicy.defersEngineStartup(true, typeNull, true, false),
            "a TYPE_NULL editor with the keyboard shown builds its session right away");
        check(!InputViewRefreshPolicy.defersEngineStartup(true, typeNull, false, true),
            "a hardware keyboard composes through the engine while the soft keyboard is hidden");
        check(!InputViewRefreshPolicy.defersEngineStartup(true, text, false, false),
            "an ordinary text field keeps building its session before the window shows");
        check(!InputViewRefreshPolicy.defersEngineStartup(false, typeNull, false, false),
            "nothing is deferred for an input start that wants no engine");
        check(InputViewRefreshPolicy.shouldResumeDeferredStartup(7, 7, 0, connection, connection, true),
            "showing the keyboard on the deferring editor starts its session");
        check(!InputViewRefreshPolicy.shouldResumeDeferredStartup(-1, 7, 0, connection, connection, true),
            "no deferral means nothing to resume");
        check(!InputViewRefreshPolicy.shouldResumeDeferredStartup(6, 7, 0, connection, connection, true),
            "a deferral from an earlier input start must not start a session");
        check(!InputViewRefreshPolicy.shouldResumeDeferredStartup(7, 7, 3, connection, connection, true),
            "an existing session must not be replaced");
        check(!InputViewRefreshPolicy.shouldResumeDeferredStartup(7, 7, 0, connection, new Connection(), true),
            "a different connection must not inherit the deferral");
        check(!InputViewRefreshPolicy.shouldResumeDeferredStartup(7, 7, 0, connection, connection, false),
            "a direct-input editor must not create a session");
        check(InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, true, false, false, false, false),
            "a same-editor restart without composition keeps the session");
        check(InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, true, false, false, true, true),
            "a no-learning editor keeps the session that was built without learning");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                false, 3, true, 5, true, true, false, false, false, false),
            "a new input start must rebuild the session");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 0, true, 5, true, true, false, false, false, false),
            "there is no session to keep");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, false, 5, true, true, false, false, false, false),
            "a session without a view must be rebuilt");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 0, true, true, false, false, false, false),
            "a restart that starts a new document must rebuild the session");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, false, true, false, false, false, false),
            "a different field, input type, action or package must rebuild the session");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, false, false, false, false, false),
            "a direct-input editor must not keep an engine session");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, true, true, false, false, false),
            "a composition the editor dropped must not survive into the rewritten text");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, true, false, true, false, false),
            "a pending privacy rebuild must not be skipped");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, true, false, false, true, false),
            "a learning session must not stay in an editor that now forbids learning");
        check(!InputViewRefreshPolicy.keepsSessionAcrossRestart(
                true, 3, true, 5, true, true, false, false, false, true),
            "a session built without learning is rebuilt once learning is allowed again");
        System.out.println("Android delayed input-view refresh guards passed");
    }
}

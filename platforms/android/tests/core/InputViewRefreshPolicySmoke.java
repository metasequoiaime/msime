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
        System.out.println("Android delayed input-view refresh guards passed");
    }
}

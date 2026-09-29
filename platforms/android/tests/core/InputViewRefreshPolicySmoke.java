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
        System.out.println("Android delayed input-view refresh guards passed");
    }
}

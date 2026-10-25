package app.msime.android;

/** 剪贴板历史页和统计页：暂停时认证还开着，回来是否重新认证按认证结果决定，结果在可见之前或之后送达都一样。 */
public final class AuthenticationPauseTrackerSmoke {
    public static void main(String[] args) {
        AuthenticationPauseTracker tracker = new AuthenticationPauseTracker();

        // 普通的离开再回来：重新认证。
        tracker.paused(false);
        check(tracker.reentersOnVisible(false), "plain revisit re-authenticates");

        // Android 9、10 凭据确认 Activity：成功结果在可见之前送达，那次可见不再认证。
        tracker.paused(true);
        check(!tracker.reentersOnEnd(false, false), "success before visible does not re-enter");
        check(!tracker.reentersOnVisible(false), "visible after credential success does not prompt again");
        check(tracker.reentersOnVisible(false), "the next revisit re-authenticates");

        // 成功结果在可见之后才送达。
        tracker.paused(true);
        check(!tracker.reentersOnVisible(true), "pending prompt waits for its result");
        check(!tracker.reentersOnEnd(false, true), "late success does not re-enter");
        check(tracker.reentersOnVisible(false), "revisit after late success re-authenticates");

        // 在凭据确认页按返回：不算离开，不再弹认证。
        tracker.paused(true);
        check(!tracker.reentersOnEnd(false, false), "user cancel before visible does not re-enter");
        check(!tracker.reentersOnVisible(false), "user cancel does not loop");
        tracker.paused(true);
        check(!tracker.reentersOnVisible(true), "pending prompt waits for a late user cancel");
        check(!tracker.reentersOnEnd(false, true), "late user cancel does not loop");

        // 认证窗口开着时离开，系统取消：结果在可见之前送达，那次可见按重新进入处理。
        tracker.paused(true);
        check(!tracker.reentersOnEnd(true, false), "system cancel while hidden waits for visibility");
        check(tracker.reentersOnVisible(false), "visible after system cancel re-authenticates");

        // 系统取消在可见之后才送达：立刻按重新进入处理，且只处理一次。
        tracker.paused(true);
        check(!tracker.reentersOnVisible(true), "pending prompt waits for a late system cancel");
        check(tracker.reentersOnEnd(true, true), "late system cancel re-authenticates now");
        check(!tracker.reentersOnEnd(true, true), "a later cancel outside the pause does not re-enter");

        // 不跨越暂停的认证结果不触发任何重新进入。
        tracker.paused(false);
        check(!tracker.reentersOnEnd(true, true), "cancel without a pause does not re-enter");

        // 错误码的归类：ERROR_CANCELED 一直算系统取消；Android 11 起 SystemUI 关掉后台应用的认证窗口报 ERROR_USER_CANCELED（10），也算；Android 9、10 凭据确认页按返回报的同一个错误码是用户自己取消。
        check(AuthenticationPauseTracker.systemCanceled(5, 28), "AndroidX onStop cancel on Android 9");
        check(AuthenticationPauseTracker.systemCanceled(5, 35), "system cancel on Android 15");
        check(AuthenticationPauseTracker.systemCanceled(10, 30), "SystemUI eviction on Android 11");
        check(AuthenticationPauseTracker.systemCanceled(10, 35), "SystemUI eviction on Android 15");
        check(!AuthenticationPauseTracker.systemCanceled(10, 28), "credential confirmation back on Android 9");
        check(!AuthenticationPauseTracker.systemCanceled(10, 29), "credential confirmation back on Android 10");

        tracker.paused(true);
        tracker.reset();
        check(tracker.reentersOnVisible(false), "destroyed view forgets the pause");
        System.out.println("AuthenticationPauseTrackerSmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

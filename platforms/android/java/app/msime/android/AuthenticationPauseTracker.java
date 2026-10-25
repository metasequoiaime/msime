package app.msime.android;

/**
 * 页面暂停时系统认证窗口还开着：回来的那次可见是不是「重新进入」，由这次认证怎么结束决定，不由暂停本身决定。
 *
 * <p>Android 9、10 的锁屏凭据确认是独立 Activity，输入凭据时宿主页会暂停；它返回时的那次可见只是认证结束，照常清掉授权再认证的话，认证成功后会立刻再弹一次。可暂停也可能是用户在认证窗口还开着时离开了页面（回到桌面、切换应用、熄屏），这时系统会取消认证（AndroidX 在 Android 10 以下的 `onStop` 里主动取消，Android 10 起系统关掉后台应用的认证窗口），回调报 `ERROR_CANCELED`；这种暂停必须按离开处理，回来时清掉授权并重新认证，否则页面既没有授权也没有认证窗口。
 *
 * <p>认证结果可能在页面重新可见之前或之后才送达：AndroidX 只在它的 Fragment 启动后才把结果交给回调，再经主线程执行器转发，所以重新可见时认证可能看起来还在进行。因此结果没到之前不下结论，等结果到了再判断。用户自己取消（在凭据确认页按返回）不算离开，不再弹认证，免得取消一次又弹一次。
 */
public final class AuthenticationPauseTracker {
    private boolean spansPause;

    /** 页面暂停时调用，记下认证窗口是否仍在进行。 */
    public void paused(boolean authenticating) { spansPause = authenticating; }

    /** 页面重新可见时调用；返回 true 表示按重新进入处理：清掉授权并重新认证。 */
    public boolean reentersOnVisible(boolean authenticating) {
        if (!spansPause) return true;
        // 认证在暂停期间已经结束：它的回调已经决定了要不要重新认证（被系统取消时已清掉这条记录，不会走到这里）。
        if (!authenticating) spansPause = false;
        return false;
    }

    /**
     * 认证结束（成功或出错）时调用；返回 true 表示页面已经可见、这次跨越暂停的认证却被系统取消，应立刻按重新进入处理。
     *
     * <p>页面还没可见时被系统取消，只清掉记录，下一次可见自然按重新进入处理；成功或用户取消且页面还没可见时保留记录，让那次可见不再重新认证。
     */
    public boolean reentersOnEnd(boolean systemCanceled, boolean visible) {
        if (!spansPause) return false;
        if (systemCanceled) {
            spansPause = false;
            return visible;
        }
        if (visible) spansPause = false;
        return false;
    }

    /** 视图销毁时调用。 */
    public void reset() { spansPause = false; }
}

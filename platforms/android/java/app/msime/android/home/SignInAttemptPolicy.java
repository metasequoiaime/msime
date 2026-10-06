package app.msime.android.home;

/** Keeps a sign-in surface from starting multiple provider flows for one tap sequence. */
public final class SignInAttemptPolicy {
    private boolean active;

    public boolean begin() {
        if (active) return false;
        active = true;
        return true;
    }

    public void finish() {
        active = false;
    }

    /** 宿主销毁时取消尚未完成的登录，避免旧页面把新页面的入口一直锁住。 */
    public void cancel() {
        active = false;
    }

    public boolean active() {
        return active;
    }
}

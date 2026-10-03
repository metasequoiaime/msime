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

    public boolean active() {
        return active;
    }
}

package app.msime.android;

import android.content.Context;

/**
 * 设置界面读得到的那点账号信息。
 *
 * <p>This host has one identity and it makes it itself: a random subject and secret, held on the
 * device. Nothing here is signed in to and nothing needs to be -- the identity is what the
 * community catalogue is read with, and it exists as soon as anything asks for it.
 */
public final class AccountIdentity {
    private AccountIdentity() {}

    /**
     * The anonymous subject, creating it if this device has none.
     *
     * <p>Creating it here costs nothing and reaches nothing: subject and secret are both generated
     * locally. Waiting for a backend request to create it meant the account page could sit on "还
     * 没有匿名身份" indefinitely whenever the login endpoint was refusing requests -- which reads as
     * a sign-in the user is missing, when there is nothing to sign in to.
     */
    public static String subject(Context context) {
        try {
            return new BackendAnonymousAccount(context).ensureSubject();
        } catch (Exception | LinkageError error) {
            return "";
        }
    }

    /** A shortened form for a settings row; the full subject is not a secret but is not readable. */
    public static String shortSubject(String subject) {
        if (subject == null || subject.isEmpty()) return "";
        return subject.length() <= 14 ? subject : subject.substring(0, 14) + "…";
    }
}

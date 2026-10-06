package app.msime.android;

import android.content.Context;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * 设置界面读得到的那点账号信息。
 *
 * <p>This host has one identity and it makes it itself: a random subject and secret, held on the
 * device. Nothing here is signed in to and nothing needs to be -- the identity is what the
 * community catalogue is read with, and it exists as soon as anything asks for it.
 */
public final class AccountIdentity {
    private static final AtomicBoolean REGISTERING = new AtomicBoolean();

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

    /**
     * Registers this device's anonymous account with the backend on a background thread, so a new install has one before any feature asks for it.
     *
     * <p>A saved session, even an expired one, makes this a local read; a refused or failed request is left for the next start or for the first feature that needs a token. Only the locally generated subject and secret are sent, never input.
     */
    public static void register(Context context) {
        if (!REGISTERING.compareAndSet(false, true)) return;
        Context application = context.getApplicationContext();
        Thread registration = new Thread(() -> {
            try {
                new BackendAnonymousAccount(application).ensureRegistered();
            } catch (Exception | LinkageError ignored) {
                // Retried on the next start or by the first feature that needs a token.
            } finally {
                REGISTERING.set(false);
            }
        }, "msime-anonymous-account");
        try {
            registration.start();
        } catch (RuntimeException error) {
            REGISTERING.set(false);
        }
    }

    /** A shortened form for a settings row; the full subject is not a secret but is not readable. */
    public static String shortSubject(String subject) {
        if (subject == null || subject.isEmpty()) return "";
        return subject.length() <= 14 ? subject : subject.substring(0, 14) + "…";
    }
}

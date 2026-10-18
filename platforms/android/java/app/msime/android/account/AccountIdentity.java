package app.msime.android;

import android.content.Context;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * 本机匿名账号的注册时机。
 *
 * <p>This host has one anonymous identity: a random subject and secret, held on the device. 生成、保存和注册都由 Rust 做（{@link BackendAnonymousAccount}，#8201 迁移计划 5b），这里只决定什么时候注册。
 */
public final class AccountIdentity {
    private static final AtomicBoolean REGISTERING = new AtomicBoolean();

    private AccountIdentity() {}

    /**
     * Registers this device's anonymous account with the backend on a background thread, so a new install has one before any feature asks for it.
     *
     * <p>A saved session, even an expired one, makes this a local read; a refused or failed request is left for the next start or for the first feature that needs a token. Only the locally generated subject and secret are sent, never input.
     */
    public static void register(Context context) {
        if (!REGISTERING.compareAndSet(false, true)) return;
        Context application = context.getApplicationContext();
        try {
            ThreadPolicy.startNamedThread("msime-anonymous-account", () -> {
                try {
                    new BackendAnonymousAccount(application).ensureRegistered();
                } catch (Exception | LinkageError ignored) {
                    // Retried on the next start or by the first feature that needs a token.
                } finally {
                    REGISTERING.set(false);
                }
            });
        } catch (RuntimeException error) {
            REGISTERING.set(false);
        }
    }
}

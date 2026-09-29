package app.msime.android.home;

import android.app.Activity;
import android.content.Context;
import androidx.core.content.ContextCompat;
import app.msime.android.BackendAccount;
import app.msime.android.GoogleSignInFlow;
import app.msime.android.R;
import java.util.function.Consumer;

/**
 * Google 登录，我的页和新手引导共用这一份。
 *
 * <p>Whether to offer it and how to run it: challenge, Google's chooser, token exchange. The offer depends on the backend accepting Google and on this build carrying a client ID -- `/v1/auth/providers` answers false for a provider with no client ID configured, and a button that is certain to fail is worse than no button.
 */
final class SignIn {
    /** Signed in, sign-in offered, or neither (no client ID, the backend does not accept Google, or it could not be asked). */
    enum State { SIGNED_IN, OFFERED, ABSENT }

    private SignIn() {}

    /** The current state. Blocking: it reads the stored session and may ask the backend, so never call it on the main thread. */
    static State state(Context context) {
        BackendAccount account = new BackendAccount(context);
        if (account.signedIn()) return State.SIGNED_IN;
        String clientId = context.getString(R.string.google_server_client_id);
        return !clientId.isEmpty() && account.supports("google") ? State.OFFERED : State.ABSENT;
    }

    /**
     * Run a sign-in from the main thread. `finished` receives an empty string on success or a message to show on failure, always on the main thread; it may arrive after the caller has gone, so the caller checks its own lifecycle before touching views.
     */
    static void start(Activity activity, Consumer<String> finished) {
        String clientId = activity.getString(R.string.google_server_client_id);
        Context context = activity.getApplicationContext();
        offMainThread(() -> {
            BackendAccount.Challenge challenge;
            try {
                challenge = new BackendAccount(context).challenge("google", null);
            } catch (Exception | LinkageError error) {
                challenge = null;
            }
            BackendAccount.Challenge started = challenge;
            activity.runOnUiThread(() -> {
                if (started == null) {
                    finished.accept("现在无法开始 Google 登录，请稍后再试。");
                    return;
                }
                if (activity.isFinishing() || activity.isDestroyed()) return;
                GoogleSignInFlow.start(activity, clientId, started.nonce(),
                    // Credential Manager only dispatches the result; the main executor avoids a thread per attempt that nothing shuts down.
                    ContextCompat.getMainExecutor(activity),
                    new GoogleSignInFlow.Listener() {
                        @Override public void onToken(String idToken) {
                            offMainThread(() -> {
                                String failure;
                                try {
                                    new BackendAccount(context).login(started, idToken);
                                    failure = "";
                                } catch (Exception | LinkageError error) {
                                    failure = "登录没有完成：" + error.getMessage();
                                }
                                String result = failure;
                                activity.runOnUiThread(() -> finished.accept(result));
                            });
                        }

                        @Override public void onFailure(String message) {
                            activity.runOnUiThread(() -> finished.accept(message));
                        }
                    });
            });
        });
    }

    private static void offMainThread(Runnable work) {
        new Thread(work, "msime-sign-in").start();
    }
}

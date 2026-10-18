package app.msime.android.home;

import app.msime.android.TextPolicy;
import android.app.Activity;
import android.content.Context;
import android.os.Handler;
import androidx.core.content.ContextCompat;
import app.msime.android.AccountFailureText;
import app.msime.android.AccountStore;
import app.msime.android.AppleWebSignIn;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import app.msime.android.DeviceDataApi;
import app.msime.android.GoogleSignInFlow;
import app.msime.android.MainThreadPolicy;
import app.msime.android.R;
import app.msime.android.RustAccount;
import app.msime.android.RustCloudSync;
import app.msime.android.SignInFailurePolicy;
import app.msime.android.SyncSwitch;
import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.ThreadPolicy;
import android.util.Log;
import java.io.IOException;
import app.msime.android.TextPolicy;
import java.util.function.Consumer;

/**
 * 登录：Google、Apple（网页授权）与邮箱验证码三种方式，我的页、新手引导和登录面板共用这一份。
 *
 * <p>Whether to offer it and how to run it. The offer depends on the backend accepting at least one provider this build can use -- the providers Rust reads from the backend answer false for a provider with no client configured, and a button that is certain to fail is worse than no button. 挑战、凭据和会话都经 Rust 的账号入口（{@link BackendAccount}），失败按 Rust 的错误码说成一句话（{@link SignInFailurePolicy}）。登录成功后记下账号与登录方式（{@link SyncSwitch#bindAccount}），并让 Rust 的云同步按新登录对齐一次（换账号时 Rust 关闭同步、清空进度），但不触发任何同步；退出登录时调用 {@link #signOut}，先让 Rust 清空同步状态、取消旧账号排队中的词库快照，再清会话。{@link BackendAccount} 在设备测试 APK 的编译清单里，不能引用 SyncSwitch，所以这两个时机放在这里。
 */
final class SignIn {
    /** Signed in, sign-in offered, or neither (no provider this build can use, or the backend could not be asked). */
    enum State { SIGNED_IN, OFFERED, ABSENT }

    /** 一次邮箱验证码请求的结果：成功时 `challenge` 非空，失败时 `failure` 是给人看的一句话。 */
    record EmailCode(BackendAccount.EmailChallenge challenge, String failure) {}

    private static final Handler MAIN = MainThreadPolicy.mainHandler();
    /** 等 Apple 回调的那个界面；回调到达时在主线程上收到空字符串（成功）或失败说明。只此一个，新流程覆盖旧流程。 */
    private static Consumer<String> appleWaiter;

    private SignIn() {}

    /** The current state. Blocking: it reads the stored session and may ask the backend, so never call it on the main thread. */
    static State state(Context context) {
        BackendAccount account = new BackendAccount(context);
        if (account.signedIn()) return State.SIGNED_IN;
        CloudApi.Providers providers = providers(context);
        return google(context, providers) || providers.appleWeb() || providers.email() ? State.OFFERED : State.ABSENT;
    }

    /** 后端现在接受的登录方式；读不到时都当作不提供。阻塞。 */
    static CloudApi.Providers providers(Context context) {
        try {
            return new BackendAccount(context).providers();
        } catch (Exception | LinkageError error) {
            return CloudApi.Providers.NONE;
        }
    }

    /** Google 登录要后端接受、且这个安装包带着 server client ID。 */
    static boolean google(Context context, CloudApi.Providers providers) {
        return providers.google() && !context.getString(R.string.google_server_client_id).isEmpty();
    }

    /**
     * Run a sign-in from the main thread: opens {@link LoginSheet}, which offers whichever providers the backend accepts. `finished` receives an empty string on success or a message to show on failure or dismissal, always on the main thread; it may arrive after the caller has gone, so the caller checks its own lifecycle before touching views.
     */
    static void start(Activity activity, Consumer<String> finished) {
        LoginSheet.show(activity, "login", finished);
    }

    /** Google 的那一路：向后端要 nonce、拉起 Google 的账号选择、用 ID token 换会话。 */
    static void startGoogle(Activity activity, Consumer<String> finished) {
        startGoogle(activity, "login", finished);
    }

    /** Google 登录或关联：link 流程始终绑定发起它的账号会话。 */
    static void startGoogle(Activity activity, String purpose, Consumer<String> finished) {
        String clientId = activity.getString(R.string.google_server_client_id);
        Context context = activity.getApplicationContext();
        offMainThread(() -> {
            BackendAccount.Challenge challenge;
            try {
                challenge = new BackendAccount(context).googleChallenge(purpose);
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
                                    synchronized (SyncSwitch.bindingLock()) {
                                        new BackendAccount(context).login(started, idToken);
                                        if (!"link".equals(purpose)) bind(context, "google");
                                    }
                                    failure = "";
                                } catch (Exception | LinkageError error) {
                                    failure = SignInFailurePolicy.login(AccountStore.code(error));
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

    /**
     * Apple 的那一路：生成 verifier、向后端要授权地址并在浏览器里打开。结果经 {@link AuthRedirectActivity} 回来，到时 `finished` 在主线程上收到空字符串或失败说明；打不开浏览器或拿不到地址时立刻收到失败说明。
     *
     * @param purpose `login`，或把 Apple 绑到当前账号的 `link`
     */
    static void startApple(Activity activity, String purpose, Consumer<String> finished) {
        Context context = activity.getApplicationContext();
        appleWaiter = finished;
        offMainThread(() -> {
            String url;
            String failure;
            try {
                url = AppleWebSignIn.start(context, purpose);
                failure = "";
            } catch (Exception | LinkageError error) {
                url = null;
                failure = SignInFailurePolicy.appleStart(AccountStore.code(error));
            }
            String address = url;
            String message = failure;
            activity.runOnUiThread(() -> {
                if (address == null) {
                    if (appleWaiter == finished) appleWaiter = null;
                    finished.accept(message);
                    return;
                }
                if (!LinkActions.openBrowsable(activity, address)) {
                    AppleWebSignIn.clearPending(context);
                    if (appleWaiter == finished) appleWaiter = null;
                    finished.accept("这台设备上没有可以打开 Apple 登录页的浏览器");
                }
            });
        });
    }

    /** 不再等 Apple 回调（登录面板关了）。回调晚到时仍会完成登录，只是没有界面再收结果。 */
    static void stopWaitingForApple(Consumer<String> waiter) {
        if (appleWaiter == waiter) appleWaiter = null;
    }

    /** 串行化 Apple 回调的兑换：同一条等待中的流程不会被两个回调同时兑换。 */
    private static final Object APPLE_EXCHANGE = new Object();

    /**
     * 处理一次形状已经检查过的 Apple 回调（{@link AuthRedirectActivity}）。阻塞，不要在主线程调用；结果交给等待中的界面。
     *
     * <p>在锁里读出等待中的流程再兑换，成功或其他失败都按 verifier 删掉它并报告结果；Rust 报 account_invalid（服务端不认这个 grant：不认识、已用过或属于别的 verifier，例如伪造的或旧标签页的回调）时保留它、也不报告失败，真正的回调随后仍能完成。`error=` 回调同样保留、不报告：回调 Activity 是导出的，任何应用或网页都能发一个带 error 的 VIEW，原先它会删掉流程、让正在进行的登录失败；现在流程留到过期，用户也可以自己关掉登录面板。
     *
     * @param error 回调里的 `error`，没有时为 null
     */
    static void completeApple(Context context, String grant, String error) {
        String failure;
        synchronized (APPLE_EXCHANGE) {
            AppleWebSignIn.Pending pending = AppleWebSignIn.peekPending(context);
            if (pending == null) return;
            if (error != null && !error.isEmpty()) return;
            try {
                synchronized (SyncSwitch.bindingLock()) {
                    AppleWebSignIn.complete(context, pending, grant);
                    if (!pending.link()) bind(context, "apple");
                }
                failure = "";
            } catch (Exception | LinkageError rejected) {
                if (AppleWebSignIn.keepPendingAfter(AccountStore.code(rejected))) return;
                failure = "Apple " + SignInFailurePolicy.login(AccountStore.code(rejected));
            }
            AppleWebSignIn.clearPending(context, pending.verifier());
        }
        String result = failure;
        MAIN.post(() -> {
            Consumer<String> waiter = appleWaiter;
            appleWaiter = null;
            if (waiter != null) waiter.accept(result);
        });
    }

    /** 请服务端发一封 6 位验证码。阻塞，不要在主线程调用。 */
    static EmailCode requestEmailCode(Context context, String email, String purpose) {
        String target = TextPolicy.trimmed(email);
        if (!BackendAccount.validEmail(target)) return new EmailCode(null, "邮箱地址看起来不完整");
        try {
            return new EmailCode(new BackendAccount(context).requestEmailCode(target, purpose), "");
        } catch (Exception | LinkageError error) {
            return new EmailCode(null, SignInFailurePolicy.emailCode(AccountStore.code(error)));
        }
    }

    /** 提交验证码完成登录。阻塞。成功返回空字符串，否则返回给人看的一句话。 */
    static String verifyEmailCode(Context context, BackendAccount.EmailChallenge challenge, String code) {
        String credential = TextPolicy.trimmed(code);
        if (!BackendAccount.validEmailCode(credential)) return "验证码是 6 位数字";
        try {
            synchronized (SyncSwitch.bindingLock()) {
                new BackendAccount(context).verifyEmailCode(challenge, credential);
                if (!"link".equals(challenge.purpose())) bind(context, "email");
            }
            return "";
        } catch (Exception | LinkageError error) {
            return SignInFailurePolicy.emailLogin(AccountStore.code(error));
        }
    }

    /** 退出登录：先让 Rust 的云同步清空进度、关掉开关并取消旧账号排队中的词库快照（Rust 的 `sign_out`，要在清会话之前），再删掉本机会话，取消还在进行的流式对话和 AI 皮肤生成，最后请 Credential Manager 清掉它记住的登录状态（尽力而为，见 {@link GoogleSignInFlow#clearCredentialState}）。阻塞；清除会话失败时抛出，界面不能按已退出处理。服务端的会话已经撤销或账号已经注销时（移除本机、注销账号）也走这里，Rust 的 forget 对已经清掉的会话什么也不做。 */
    static void signOut(Context context) {
        synchronized (SyncSwitch.bindingLock()) {
            String accountId = SyncSwitch.accountId(context);
            try {
                RustCloudSync.signOut(context);
            } catch (AccountStore.Failure failure) {
                // 退出登录不能因为同步状态写不进去而失败：Rust 下一次读状态时按会话对齐，看到已经退出就清空进度、关掉开关。
                Log.w("MSIMESignIn", "cloud sync sign-out failed: " + failure.code);
            }
            new BackendAccount(context).signOut();
            cancelPendingSnapshot(context, accountId);
            SyncSwitch.clear(context);
        }
        RustAccount.cancelForSignOut();
        GoogleSignInFlow.clearCredentialState(context);
    }

    /** {@link #logout} 的结果：本机是否已经退出，以及要提示的一句话（完全成功时为空串）。 */
    record Logout(boolean signedOut, String message) {}

    /**
     * 「退出登录」按钮：撤销服务端的这次登录（Rust 的 `logout{all:false}`），再清本机的同步状态。阻塞，不要在主线程调用。
     *
     * <p>Rust 先清本地会话再请求服务端，所以服务端那一步失败（断网）时本机已经退出：照样清掉同步状态，结果是已退出、带一句说明服务端没有撤销的话。登录已经不是页面打开时的那个账号时什么也不动；本机也没退出时是未退出加失败说明。
     */
    static Logout logout(Context context, String expectedUserId) {
        synchronized (SyncSwitch.bindingLock()) {
            String code = "";
            try {
                new DeviceDataApi(context, expectedUserId).logout();
            } catch (AccountStore.Failure failure) {
                if ("account_cancelled".equals(failure.code)) return new Logout(false, "登录已改变，请重新打开页面");
                code = failure.code;
            }
            try {
                if (!RustAccount.userId(context).isEmpty()) return new Logout(false, "没有退出登录，请稍后再试");
                signOut(context);
            } catch (AccountStore.Failure | RuntimeException failure) {
                return new Logout(false, "没有退出登录，请稍后再试");
            }
            return new Logout(true, code.isEmpty() ? "" : AccountFailureText.signedOutLocally(code));
        }
    }

    /** Clears the local login only when it is still the session that a page loaded. */
    static boolean signOut(Context context, String expectedSessionId) {
        synchronized (SyncSwitch.bindingLock()) {
            try {
                if (!expectedSessionId.equals(new BackendAccount(context).sessionId())) return false;
                signOut(context);
                return true;
            } catch (Exception | LinkageError error) {
                return false;
            }
        }
    }


    /** 账号退出或替换时取消仍待激活的云端词库快照（Rust 的快照队列），避免下一位账号继承旧账号的词库。 */
    static void cancelPendingSnapshot(Context context, String accountId) {
        if (accountId == null || accountId.isEmpty()) return;
        try {
            DictionarySnapshotQueue.cancel(DictionarySnapshotQueue.directory(context), accountId);
        } catch (DictionarySnapshotQueue.Failure | IOException ignored) {
            // 退出登录不能因过期的同步队列损坏而失败；下次队列维护会继续处理。
        }
    }

    /**
     * 记下这次登录的账号，再让 Rust 的云同步按会话里的用户 id 对齐一次（换了账号时关闭同步、清空进度，同一个账号重新登录保留进度）。
     *
     * <p>用户 id 读 Rust 刚存下的登录会话（status，不联网），与 Rust 对齐同步时用的是同一个值；读不到时按换账号处理（SyncSwitch 清空账号绑定）。原来这里另发一次 `GET /v1/users/me`，登录刚成功就断网时会读成空、被当成换了账号（迁移差异 M105）。
     */
    private static void bind(Context context, String loginKind) {
        String id;
        try {
            id = DeviceDataApi.currentUserId(context);
        } catch (AccountStore.Failure failure) {
            id = "";
        }
        String previousAccount = SyncSwitch.accountId(context);
        if (!previousAccount.isEmpty() && !previousAccount.equals(id)) {
            cancelPendingSnapshot(context, previousAccount);
        }
        SyncSwitch.bindAccount(context, id, loginKind);
        try {
            RustCloudSync.status(context);
        } catch (AccountStore.Failure failure) {
            // 对齐只是提前做：Rust 的 status、set_enabled、run 每次都会按会话再对齐一次。
            Log.w("MSIMESignIn", "cloud sync alignment after sign-in failed: " + failure.code);
        }
    }

    private static void offMainThread(Runnable work) {
        ThreadPolicy.startNamedThread("msime-sign-in", work);
    }
}

package app.msime.android.home;

import app.msime.android.TextPolicy;
import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Handler;
import androidx.core.content.ContextCompat;
import app.msime.android.AppEdition;
import app.msime.android.AppleWebSignIn;
import app.msime.android.BackendAccount;
import app.msime.android.CloudApi;
import app.msime.android.GoogleSignInFlow;
import app.msime.android.MainThreadPolicy;
import app.msime.android.R;
import app.msime.android.SyncSwitch;
import app.msime.android.DictionarySnapshotQueue;
import app.msime.android.ThreadPolicy;
import java.io.File;
import app.msime.android.TextPolicy;
import java.util.function.Consumer;
import org.json.JSONObject;

/**
 * 登录：Google、Apple（网页授权）与邮箱验证码三种方式，我的页、新手引导和登录面板共用这一份。
 *
 * <p>Whether to offer it and how to run it. The offer depends on the backend accepting at least one provider this build can use -- `/v1/auth/providers` answers false for a provider with no client configured, and a button that is certain to fail is worse than no button. 登录成功后记下账号与登录方式（{@link SyncSwitch#bindAccount}，换账号时它会关闭同步并清空游标），但不触发任何同步；退出登录时调用 {@link #signOut}，同样清空同步状态。{@link BackendAccount} 在设备测试 APK 的编译清单里，不能引用 SyncSwitch，所以这两个时机放在这里。
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
            return new CloudApi(context).providers();
        } catch (CloudApi.Failure | RuntimeException | LinkageError error) {
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
                                    synchronized (SyncSwitch.bindingLock()) {
                                        new BackendAccount(context).login(started, idToken, userAgent(context));
                                        bind(context, "google");
                                    }
                                    failure = "";
                                } catch (Exception | LinkageError error) {
                                    failure = "登录没有完成：" + explain(error);
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
            } catch (CloudApi.Failure error) {
                url = null;
                failure = error.unavailable() ? "Apple 登录暂未开放" : error.network()
                    ? "连不上服务器，请检查网络后再试" : "现在无法开始 Apple 登录，请稍后再试。";
            } catch (RuntimeException | LinkageError error) {
                url = null;
                failure = "现在无法开始 Apple 登录，请稍后再试。";
            }
            String address = url;
            String message = failure;
            activity.runOnUiThread(() -> {
                if (address == null) {
                    if (appleWaiter == finished) appleWaiter = null;
                    finished.accept(message);
                    return;
                }
                try {
                    activity.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(address))
                        .addCategory(Intent.CATEGORY_BROWSABLE));
                } catch (ActivityNotFoundException absent) {
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
     * <p>在锁里读出等待中的流程再兑换，成功、网络错误或 5xx 都按 verifier 删掉它并报告结果；服务端以 400 / 401 / 404 拒绝（grant 不认识、已用过或属于别的 challenge，例如伪造的或旧标签页的回调）时保留它、也不报告失败，真正的回调随后仍能完成。`error=` 回调同样保留、不报告：回调 Activity 是导出的，任何应用或网页都能发一个带 error 的 VIEW，原先它会删掉流程、让正在进行的登录失败；现在流程留到过期，用户也可以自己关掉登录面板。
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
                    AppleWebSignIn.complete(context, pending, grant, userAgent(context));
                    if (!pending.link()) bind(context, "apple");
                }
                failure = "";
            } catch (Exception | LinkageError rejected) {
                if (AppleWebSignIn.keepPendingAfter(BackendAccount.failureStatus(rejected))) return;
                failure = "Apple 登录没有完成：" + explain(rejected);
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
            int status = BackendAccount.failureStatus(error);
            String failure = status == 429 ? "发送太频繁，请稍后再试"
                : status == 503 ? "邮箱登录暂未开放"
                : status == 400 ? "这个邮箱地址不能用来登录"
                : status == 403 ? "需要先重新登录一次"
                : status == 0 ? "连不上服务器，请检查网络后再试"
                : "验证码没有发出，请稍后再试";
            return new EmailCode(null, failure);
        }
    }

    /** 提交验证码完成登录。阻塞。成功返回空字符串，否则返回给人看的一句话。 */
    static String verifyEmailCode(Context context, BackendAccount.EmailChallenge challenge, String code) {
        String credential = TextPolicy.trimmed(code);
        if (!BackendAccount.validEmailCode(credential)) return "验证码是 6 位数字";
        try {
            synchronized (SyncSwitch.bindingLock()) {
                new BackendAccount(context).verifyEmailCode(challenge, credential, userAgent(context));
                if (!"link".equals(challenge.purpose())) bind(context, "email");
            }
            return "";
        } catch (Exception | LinkageError error) {
            int status = BackendAccount.failureStatus(error);
            return status == 400 || status == 401 ? "验证码不对或已过期"
                : status == 429 ? "尝试太多次，请稍后再试"
                : status == 403 ? "需要先重新登录一次"
                : status == 0 ? "连不上服务器，请检查网络后再试"
                : "登录没有完成，请稍后再试";
        }
    }

    /** 退出登录：删掉本机会话并关闭同步、清空同步游标，再请 Credential Manager 清掉它记住的登录状态（尽力而为，见 {@link GoogleSignInFlow#clearCredentialState}）。阻塞；清除会话失败时抛出，界面不能按已退出处理。 */
    static void signOut(Context context) {
        synchronized (SyncSwitch.bindingLock()) {
            String accountId = SyncSwitch.accountId(context);
            new BackendAccount(context).signOut();
            cancelPendingSnapshot(context, accountId);
            SyncSwitch.clear(context);
        }
        GoogleSignInFlow.clearCredentialState(context);
    }


    /** 账号退出或替换时取消仍待激活的云端词库快照，避免下一位账号继承旧账号的词库。 */
    static void cancelPendingSnapshot(Context context, String accountId) {
        if (accountId == null || accountId.isEmpty()) return;
        try {
            File files = context.getFilesDir();
            if (files == null) return;
            DictionarySnapshotQueue queue = new DictionarySnapshotQueue(
                files.toPath(), new File(files, "bootstrap/state/dictionary-snapshots").toPath());
            queue.cancel(accountId);
        } catch (Exception | LinkageError ignored) {
            // 退出登录不能因过期的同步队列损坏而失败；下次队列维护会继续处理。
        }
    }

    /** 记下这次登录的账号；读不到用户 id 时按换账号处理（SyncSwitch 会清空同步状态）。 */
    private static void bind(Context context, String loginKind) {
        String id = "";
        try {
            JSONObject me = new CloudApi(context).json("GET", "/v1/users/me", null, CloudApi.Auth.ACCOUNT);
            JSONObject user = me.optJSONObject("user");
            Object value = user == null ? null : user.opt("id");
            if (value instanceof String text) id = text;
        } catch (CloudApi.Failure | RuntimeException error) {
            id = "";
        }
        String previousAccount = SyncSwitch.accountId(context);
        if (!previousAccount.isEmpty() && !previousAccount.equals(id)) {
            cancelPendingSnapshot(context, previousAccount);
        }
        SyncSwitch.bindAccount(context, id, loginKind);
    }

    private static String userAgent(Context context) {
        return BackendAccount.loginUserAgent(context, AppEdition.current().id());
    }

    private static String explain(Throwable error) {
        int status = BackendAccount.failureStatus(error);
        if (status == 0) return "连不上服务器";
        if (status == 429) return "尝试太频繁，请稍后再试";
        if (status == 503) return "这种登录方式暂未开放";
        return "服务器拒绝了这次登录（" + status + "）";
    }

    private static void offMainThread(Runnable work) {
        ThreadPolicy.startNamedThread("msime-sign-in", work);
    }
}

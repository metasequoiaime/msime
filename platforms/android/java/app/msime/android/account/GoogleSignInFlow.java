package app.msime.android;

import android.app.Activity;
import android.content.Context;
import android.util.Log;
import androidx.credentials.ClearCredentialStateRequest;
import androidx.credentials.Credential;
import androidx.credentials.CredentialManager;
import androidx.credentials.CredentialManagerCallback;
import androidx.credentials.CustomCredential;
import androidx.credentials.GetCredentialRequest;
import androidx.credentials.GetCredentialResponse;
import androidx.credentials.exceptions.ClearCredentialException;
import androidx.credentials.exceptions.GetCredentialException;
import com.google.android.libraries.identity.googleid.GetSignInWithGoogleOption;
import com.google.android.libraries.identity.googleid.GoogleIdTokenCredential;
import java.util.concurrent.Executor;

/**
 * 向 Google 要一枚 ID token，带着后端给的 nonce。
 *
 * <p>Credential Manager rather than the old `GoogleSignInClient`, which Google has deprecated. The
 * nonce is not decoration: the backend generated it for this one attempt, Google signs it into the
 * token, and the backend checks it back -- that is what stops a token minted for another app, or
 * lifted from an earlier sign-in, being presented here.
 *
 * <p>Nothing is offered unless a server client ID is configured. Without one there is no audience
 * for the token and the attempt cannot succeed, so the caller hides the button instead of letting
 * someone press it and read an error.
 */
public final class GoogleSignInFlow {
    private static final String TAG = "MSIMESignIn";

    private GoogleSignInFlow() {}

    /**
     * 把异常说成一句话。
     *
     * <p>取消和配置错误不是一件事：前者是用户的决定，后者是这台设备上这个 app 根本拿不到令牌。原来两者都写成「没有完成」，于是屏幕上那句话对任何一种情况都成立，也就什么都没说——查一次得去翻 logcat。类型和原文都带上，代价是一句长一点的话。
     *
     * <p>取消以外的失败都在 logcat 里留一行异常类型和消息：`NoCredentialException` 在屏幕上只带状态码（{@link GoogleSignInFailure}），完整的 Play services 消息只在这里。消息是 Play services 的状态说明，不含账号、令牌或 nonce。
     */
    private static String explain(GetCredentialException error) {
        if (error instanceof androidx.credentials.exceptions.GetCredentialCancellationException) {
            return "已取消 Google 登录";
        }
        Log.w(TAG, "Google sign-in failed: " + error.getType() + ": " + error.getMessage());
        if (error instanceof androidx.credentials.exceptions.NoCredentialException) {
            return GoogleSignInFailure.noCredential(error.getMessage());
        }
        String detail = error.getMessage();
        return "Google 登录失败：" + error.getType()
            + (detail == null || detail.isEmpty() ? "" : "，" + detail);
    }

    /**
     * 退出登录后告诉 Credential Manager 清掉它为这个 app 记住的登录状态。尽力而为：结果只写日志，不阻塞、不抛出，调用方在本机会话已经删掉之后再调。
     *
     * <p>在 Play services 上它对应 `SignInClient.signOut`，只关掉自动选择账号；这里没有打开自动选择，所以它不影响下一次能否登录（#5979 的原因在 {@link #start} 的选项上）。照 Google 的建议在退出时调用，免得以后打开自动选择时，退出登录后又被悄悄登回同一个账号。
     */
    public static void clearCredentialState(Context context) {
        try {
            CredentialManager.create(context.getApplicationContext()).clearCredentialStateAsync(
                new ClearCredentialStateRequest(), null, Runnable::run,
                new CredentialManagerCallback<Void, ClearCredentialException>() {
                    @Override public void onResult(Void result) {}

                    @Override public void onError(ClearCredentialException error) {
                        Log.w(TAG, "Credential state not cleared: " + error.getType() + ": " + error.getMessage());
                    }
                });
        } catch (RuntimeException | LinkageError error) {
            // 没有凭据提供方或 Play services 版本不对时可能在这里同步抛出；本机已经退出登录，不能因为这一步把退出报成失败。
            Log.w(TAG, "Credential state not cleared", error);
        }
    }

    /** What came back: a token to exchange, or a reason to show. */
    public interface Listener {
        void onToken(String idToken);

        void onFailure(String message);
    }

    /**
     * Run the account chooser and hand back the ID token.
     *
     * @param serverClientId the backend's own Google client ID, which the token is minted for
     * @param nonce          the challenge's nonce, passed through unchanged
     */
    public static void start(Activity activity, String serverClientId, String nonce,
            Executor executor, Listener listener) {
        if (serverClientId == null || serverClientId.isEmpty()) {
            listener.onFailure("这台设备还没有配置 Google 登录");
            return;
        }
        // 用户按的是「通过 Google 登录」按钮，所以走按钮流程 `GetSignInWithGoogleOption`：它弹出完整的账号选择，需要重新验证的账号也在里面，还能当场添加账号。原来的 `GetGoogleIdOption` 是底部弹出的一键登录，Play services 会从里面去掉需要重新验证的账号，用户关掉几次后还会进入冷却期（`28436`），这两种情况都报成 no credential，于是设备上明明有 Google 账号，屏幕上却说没有（#5979）。按钮选项不能和别的选项放在同一个请求里，这里也只放这一个。
        GetSignInWithGoogleOption option = new GetSignInWithGoogleOption.Builder(serverClientId)
            .setNonce(nonce)
            .build();
        GetCredentialRequest request = new GetCredentialRequest.Builder()
            .addCredentialOption(option)
            .build();
        CredentialManager.create(activity).getCredentialAsync(activity, request, null, executor,
            new CredentialManagerCallback<GetCredentialResponse, GetCredentialException>() {
                @Override public void onResult(GetCredentialResponse response) {
                    Credential credential = response.getCredential();
                    if (credential instanceof CustomCredential custom
                            && GoogleIdTokenCredential.TYPE_GOOGLE_ID_TOKEN_CREDENTIAL
                                .equals(custom.getType())) {
                        try {
                            listener.onToken(GoogleIdTokenCredential
                                .createFrom(custom.getData()).getIdToken());
                            return;
                        } catch (RuntimeException error) {
                            listener.onFailure("Google 返回的凭据无法读取");
                            return;
                        }
                    }
                    listener.onFailure("Google 返回了预期之外的凭据");
                }

                @Override public void onError(GetCredentialException error) {
                    listener.onFailure(explain(error));
                }
            });
    }
}

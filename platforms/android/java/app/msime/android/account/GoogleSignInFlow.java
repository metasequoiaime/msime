package app.msime.android;

import android.app.Activity;
import androidx.credentials.Credential;
import androidx.credentials.CredentialManager;
import androidx.credentials.CredentialManagerCallback;
import androidx.credentials.CustomCredential;
import androidx.credentials.GetCredentialRequest;
import androidx.credentials.GetCredentialResponse;
import androidx.credentials.exceptions.GetCredentialException;
import com.google.android.libraries.identity.googleid.GetGoogleIdOption;
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
    private GoogleSignInFlow() {}

    /**
     * 把异常说成一句话。
     *
     * <p>取消和配置错误不是一件事：前者是用户的决定，后者是这台设备上这个 app 根本拿不到令牌。
     * 原来两者都写成「没有完成」，于是屏幕上那句话对任何一种情况都成立，也就什么都没说——查一次
     * 得去翻 logcat。类型和原文都带上，代价是一句长一点的话。
     */
    private static String explain(GetCredentialException error) {
        if (error instanceof androidx.credentials.exceptions.GetCredentialCancellationException) {
            return "已取消 Google 登录";
        }
        if (error instanceof androidx.credentials.exceptions.NoCredentialException) {
            // 如果 Android OAuth client 没有登记这个包名和签名证书，Play services 同样报告为 no credential，只是消息里带有状态码 `28444`；这种情况下设备上通常是有账号的，提示「没有账号」会把人引到错误的方向。
            String detail = error.getMessage();
            if (detail != null && (detail.contains("28444") || detail.contains("Developer console"))) {
                return "Google 登录未配置：这个安装包的签名证书没有登记到 Google Cloud 的 Android OAuth 客户端";
            }
            return "这台设备上没有可用的 Google 账号";
        }
        String detail = error.getMessage();
        return "Google 登录失败：" + error.getType()
            + (detail == null || detail.isEmpty() ? "" : "，" + detail);
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
        GetGoogleIdOption option = new GetGoogleIdOption.Builder()
            // 不限于已授权过的账号：第一次登录时设备上还没有任何授权记录，限住就是一张空列表。
            .setFilterByAuthorizedAccounts(false)
            .setServerClientId(serverClientId)
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

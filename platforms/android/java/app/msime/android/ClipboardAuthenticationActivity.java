package app.msime.android;

import android.content.Context;
import android.content.Intent;
import android.os.Bundle;
import android.os.ResultReceiver;
import androidx.annotation.Nullable;
import androidx.biometric.BiometricManager;
import androidx.biometric.BiometricPrompt;
import androidx.core.content.ContextCompat;
import androidx.fragment.app.FragmentActivity;

/** 由输入法服务启动的认证桥；服务进程没有 Activity 窗口，不能直接创建 BiometricPrompt。 */
public final class ClipboardAuthenticationActivity extends FragmentActivity {
    public static final String EXTRA_RESULT = "app.msime.android.clipboard.AUTH_RESULT";
    public static final String EXTRA_SUBTITLE = "app.msime.android.clipboard.AUTH_SUBTITLE";
    public static final int RESULT_AUTHENTICATED = 2001;
    public static final int RESULT_UNAVAILABLE = 2002;
    private static final int AUTHENTICATORS = SystemAuthenticators.allowed();
    @Nullable private ResultReceiver result;
    private boolean sent;

    public static void launch(Context context, ResultReceiver receiver, String subtitle) {
        Intent intent = new Intent(context, ClipboardAuthenticationActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK | Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS)
            .putExtra(EXTRA_RESULT, receiver)
            .putExtra(EXTRA_SUBTITLE, subtitle);
        context.startActivity(intent);
    }

    @Override protected void onCreate(@Nullable Bundle state) {
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        result = getIntent().getParcelableExtra(EXTRA_RESULT);
        if (BiometricManager.from(this).canAuthenticate(AUTHENTICATORS)
                != BiometricManager.BIOMETRIC_SUCCESS) {
            finishWith(RESULT_UNAVAILABLE);
            return;
        }
        BiometricPrompt prompt = new BiometricPrompt(this, ContextCompat.getMainExecutor(this),
            new BiometricPrompt.AuthenticationCallback() {
                @Override public void onAuthenticationSucceeded(BiometricPrompt.AuthenticationResult value) {
                    finishWith(true);
                }

                @Override public void onAuthenticationError(int code, CharSequence message) {
                    finishWith(false);
                }
            });
        String subtitle = getIntent().getStringExtra(EXTRA_SUBTITLE);
        BiometricPrompt.PromptInfo info = new BiometricPrompt.PromptInfo.Builder()
            .setTitle("验证身份")
            .setSubtitle(subtitle == null ? "剪贴板操作需要系统认证" : subtitle)
            .setAllowedAuthenticators(AUTHENTICATORS)
            .build();
        prompt.authenticate(info);
    }

    @Override public void onBackPressed() { finishWith(false); }

    /** 被系统或任务移除直接结束、没走到 finishWith 时也要回一个取消：发起方（输入法服务的忙标记、设置页开关的等待标记）只在收到结果时才复位，收不到就再也发不起认证。配置变更重建不算结束，新实例会接着认证。 */
    @Override protected void onDestroy() {
        if (!sent && isFinishing()) {
            sent = true;
            if (result != null) result.send(RESULT_CANCELED, null);
        }
        super.onDestroy();
    }

    private void finishWith(boolean authenticated) {
        finishWith(authenticated ? RESULT_AUTHENTICATED : RESULT_CANCELED);
    }

    private void finishWith(int code) {
        if (sent) return;
        sent = true;
        if (result != null) result.send(code, null);
        finish();
    }
}

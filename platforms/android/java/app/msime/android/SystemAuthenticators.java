package app.msime.android;

import android.os.Build;
import androidx.biometric.BiometricManager.Authenticators;

/**
 * 敏感操作的系统认证共用同一组认证方式：强生物识别或锁屏凭据。
 *
 * <p>AndroidX Biometric 在 Android 9、10 上不支持「强生物识别 + 锁屏凭据」这个组合：`canAuthenticate` 固定返回 `BIOMETRIC_ERROR_UNSUPPORTED`，`PromptInfo.Builder.build()` 直接抛异常，设了锁屏密码的设备也永远过不了认证。这两个版本上能带锁屏凭据后备的只有「弱生物识别 + 锁屏凭据」，因此改用它；Android 11 起仍要求强生物识别。
 */
public final class SystemAuthenticators {
    private SystemAuthenticators() {}

    public static int allowed() {
        int sdk = Build.VERSION.SDK_INT;
        int biometric = sdk == Build.VERSION_CODES.P || sdk == Build.VERSION_CODES.Q
            ? Authenticators.BIOMETRIC_WEAK : Authenticators.BIOMETRIC_STRONG;
        return biometric | Authenticators.DEVICE_CREDENTIAL;
    }
}

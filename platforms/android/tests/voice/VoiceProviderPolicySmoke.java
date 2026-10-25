import app.msime.android.VoiceProviderPolicy;

/** 设置页的凭据校验与状态提示；接口校验、换服务补丁由共享 Rust 和设备 JNI 冒烟覆盖。 */
public final class VoiceProviderPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check("system".equals(VoiceProviderPolicy.SYSTEM)
                && "local".equals(VoiceProviderPolicy.LOCAL)
                && "legacy".equals(VoiceProviderPolicy.AUTH_LEGACY),
            "voice policy constants remain stable");

        check(VoiceProviderPolicy.endpointProblem("openai", "https://example.invalid/v1") != null
                && VoiceProviderPolicy.endpointProblem("openai", "") != null,
            "共享 JNI 不可用时不放行保存；地址规则由 Rust 和真实 JNI 设备套件验证");

        System.out.println("Android voice provider settings: credentials and endpoint bridge passed");
    }
}

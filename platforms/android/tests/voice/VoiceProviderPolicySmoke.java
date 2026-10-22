import app.msime.android.VoiceProviderPolicy;

/** 设置页的凭据校验与状态提示；接口校验、换服务补丁由共享 Rust 和设备 JNI 冒烟覆盖。 */
public final class VoiceProviderPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(VoiceProviderPolicy.network("openai") && VoiceProviderPolicy.network("doubao"), "cloud services need a key");
        check(!VoiceProviderPolicy.network("system") && !VoiceProviderPolicy.network("local")
                && !VoiceProviderPolicy.network("") && !VoiceProviderPolicy.network(null),
            "the platform recogniser and the local model send nothing");

        check(VoiceProviderPolicy.legacyAuth("legacy") && VoiceProviderPolicy.legacyAuth(" legacy "),
            "legacy auth needs the App ID");
        check(!VoiceProviderPolicy.legacyAuth("") && !VoiceProviderPolicy.legacyAuth("api_key")
                && !VoiceProviderPolicy.legacyAuth(null),
            "empty means API key, as in the shared auth policy");

        check(VoiceProviderPolicy.endpointProblem("openai", "https://example.invalid/v1") != null
                && VoiceProviderPolicy.endpointProblem("openai", "") != null,
            "共享 JNI 不可用时不放行保存；地址规则由 Rust 和真实 JNI 设备套件验证");

        System.out.println("Android voice provider settings: credentials and endpoint bridge passed");
    }
}

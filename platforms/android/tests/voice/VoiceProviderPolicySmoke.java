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

        check(VoiceProviderPolicy.validCredential("openai", "") && VoiceProviderPolicy.validCredential("openai", "  "),
            "an empty key deletes it");
        check(VoiceProviderPolicy.validCredential("openai", "sk-abc"), "a normal key is accepted");
        check(!VoiceProviderPolicy.validCredential("openai", "sk\nabc"), "a control character cannot reach a header");
        check(VoiceProviderPolicy.validCredential("openai", "密钥") && !VoiceProviderPolicy.validCredential("doubao", "密钥"),
            "Doubao keys travel in handshake headers and must be ASCII");
        check(!VoiceProviderPolicy.validCredential("openai", "x".repeat(16 * 1024 + 1)), "the shared length bound applies");

        check(VoiceProviderPolicy.endpointProblem("openai", "https://example.invalid/v1") != null
                && VoiceProviderPolicy.endpointProblem("openai", "") != null,
            "共享 JNI 不可用时不放行保存；地址规则由 Rust 和真实 JNI 设备套件验证");

        // The shipped default provider is doubao with no key; the row must say the keyboard is really using the platform recogniser.
        check("密钥未填写完整，暂用系统识别".equals(VoiceProviderPolicy.status("doubao", null)),
            "an unconfigured default says what is actually used");
        check("未填写 API Key，暂用系统识别".equals(VoiceProviderPolicy.status("openai", null)),
            "a missing upload key says what is actually used");
        check(VoiceProviderPolicy.status("openai", "openai") == null, "a working configuration needs no remark");
        check("使用设备自带的语音识别服务".equals(VoiceProviderPolicy.status("system", null)),
            "the platform recogniser is explained");
        check("没有可用的系统识别服务，可改用本地模型或豆包".equals(
                VoiceProviderPolicy.status("system", null, false)),
            "a device without a platform recogniser offers an alternative");
        check("密钥未填写完整，系统识别不可用".equals(
                VoiceProviderPolicy.status("doubao", null, false)),
            "an unconfigured cloud provider does not claim an unavailable fallback");
        check("未填写 API Key，暂用系统识别".equals(
                VoiceProviderPolicy.status("openai", null, true)),
            "an available platform recogniser keeps the existing fallback wording");
        check(VoiceProviderPolicy.status("local", "local") == null, "the local model reports through localStatus");
        check("还没有可用的模型，请在下面下载一个".equals(VoiceProviderPolicy.localStatus(false, true))
                && "还没有可用的模型，请在下面下载一个".equals(VoiceProviderPolicy.localStatus(false, false)),
            "a missing model is named first");
        check("还要下载本地语音运行库".equals(VoiceProviderPolicy.localStatus(true, false)),
            "a thin package also needs the runtime");
        check(VoiceProviderPolicy.localStatus(true, true) == null, "a ready local model needs no remark");
        System.out.println("Android voice provider settings: credentials, status and unavailable endpoint bridge passed");
    }
}

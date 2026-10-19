import app.msime.android.VoiceProviderPolicy;
import java.util.LinkedHashMap;
import java.util.Map;

/** 设置页的凭据换槽、接口校验与状态提示；目录展示另由共享快照冒烟覆盖。 */
public final class VoiceProviderPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(VoiceProviderPolicy.network("openai") && VoiceProviderPolicy.network("doubao"), "cloud services need a key");
        check(!VoiceProviderPolicy.network("system") && !VoiceProviderPolicy.network("local")
                && !VoiceProviderPolicy.network("") && !VoiceProviderPolicy.network(null),
            "the platform recogniser and the local model send nothing");

        // Switching parks the current key in the old provider's slot and brings back the new one's, as the shared page's swapTokenSlot does.
        Map<String, String> slots = new LinkedHashMap<>();
        slots.put("openai", "sk-openai");
        VoiceProviderPolicy.Tokens toOpenai = VoiceProviderPolicy.switchTokens("doubao", "openai", "doubao-key", slots);
        check(toOpenai.token().equals("sk-openai"), "the new provider's own key is restored");
        check("doubao-key".equals(toOpenai.tokens().get("doubao")), "the old provider's key is kept in its slot");
        check(slots.size() == 1, "the caller's map is not modified");

        VoiceProviderPolicy.Tokens fresh = VoiceProviderPolicy.switchTokens("doubao", "groq", "doubao-key", slots);
        check(fresh.token().isEmpty(), "a provider without a key starts empty rather than inheriting another's");

        VoiceProviderPolicy.Tokens cleared = VoiceProviderPolicy.switchTokens("openai", "doubao", "", slots);
        check(!cleared.tokens().containsKey("openai"), "an empty box removes the old provider's slot");

        Map<String, String> withSystem = new LinkedHashMap<>();
        withSystem.put("system", "stale");
        VoiceProviderPolicy.Tokens toSystem = VoiceProviderPolicy.switchTokens("openai", "system", "sk", withSystem);
        check(toSystem.token().isEmpty() && !toSystem.tokens().containsKey("system")
                && "sk".equals(toSystem.tokens().get("openai")),
            "the platform recogniser holds no key, and the one being left is kept");
        check(VoiceProviderPolicy.switchTokens("", "openai", "", null).token().isEmpty(),
            "no previous provider and no slots is not an error");

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

        check(VoiceProviderPolicy.endpointProblem("openai", "") == null, "empty means the provider's default");
        check(VoiceProviderPolicy.endpointProblem("openai", "https://proxy.example.com/v1/audio/transcriptions") == null,
            "an https proxy is accepted");
        check(VoiceProviderPolicy.endpointProblem("openai", "http://proxy.example.com/v1") != null,
            "a cleartext endpoint would send the key in the clear");
        check(VoiceProviderPolicy.endpointProblem("openai", "https://user@proxy.example.com/v1") != null,
            "credentials in the authority are refused, as the shared validation does");
        check(VoiceProviderPolicy.endpointProblem("openai", "wss://example.invalid/stream") != null,
            "a Doubao socket address is not an upload endpoint");
        check(VoiceProviderPolicy.endpointProblem("doubao", "wss://example.invalid/stream") == null
                && VoiceProviderPolicy.endpointProblem("doubao", "https://openspeech.bytedance.com/x") != null,
            "Doubao only streams over wss");

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
        System.out.println("Android voice provider settings: provider list, token slots, credentials and endpoints passed");
    }
}

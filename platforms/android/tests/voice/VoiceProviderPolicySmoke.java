import app.msime.android.VoiceProviderPolicy;
import java.util.LinkedHashMap;
import java.util.Map;

/** The settings page's recognition-service rules: what can be picked, how credentials move between slots, which endpoints may be stored. */
public final class VoiceProviderPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // Every id offered must be one the shared document accepts and the shared resolver knows; the shared list is crates/client-core ASR_PROVIDERS.
        String[] accepted = {"doubao", "siliconflow", "openai", "groq", "everyapi", "mistral", "bailian", "system", "local"};
        for (String provider : VoiceProviderPolicy.PROVIDERS) {
            boolean known = false;
            for (String candidate : accepted) known |= candidate.equals(provider);
            check(known, provider + " is accepted by the shared preferences validation");
        }
        check(VoiceProviderPolicy.PROVIDERS[0].equals("system") && VoiceProviderPolicy.PROVIDERS[1].equals("local"),
            "the two services that need no key come first");

        check(VoiceProviderPolicy.label("").equals("系统识别") && VoiceProviderPolicy.label(null).equals("系统识别"),
            "nothing stored reads as the platform recogniser");
        check(VoiceProviderPolicy.label("local").equals("本地模型"), "a model chosen elsewhere is named");
        check(VoiceProviderPolicy.label("bailian").equals("阿里云百炼 · 千问"), "labels match the shared settings page");
        check(VoiceProviderPolicy.label("future").equals("future"), "an unknown id is shown as stored");

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
        check(VoiceProviderPolicy.endpointProblem("openai", VoiceProviderPolicy.DOUBAO_ASYNC) != null,
            "a Doubao socket address is not an upload endpoint");
        check(VoiceProviderPolicy.endpointProblem("doubao", VoiceProviderPolicy.DOUBAO_NOSTREAM) == null
                && VoiceProviderPolicy.endpointProblem("doubao", "https://openspeech.bytedance.com/x") != null,
            "Doubao only streams over wss");

        check(VoiceProviderPolicy.doubaoEndpointLabel(VoiceProviderPolicy.DOUBAO_NOSTREAM).equals("整句流式"),
            "the whole-sentence interface is named");
        check(VoiceProviderPolicy.doubaoEndpointLabel("").equals("双向流式")
                && VoiceProviderPolicy.doubaoEndpointLabel(VoiceProviderPolicy.DOUBAO_ASYNC).equals("双向流式"),
            "empty resolves to the shared default, the bidirectional interface");
        check(VoiceProviderPolicy.doubaoEndpointLabel("wss://example.com/asr").equals("自定义地址"),
            "anything else is a custom address");

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

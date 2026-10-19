package app.msime.android;

import java.util.LinkedHashMap;
import java.util.Map;

/**
 * 设置页「识别服务」的凭据换槽、字段校验与状态文案；公开服务和接口目录由 {@link VoiceProviderCatalog} 读取共享 Rust。
 *
 * <p>这里不推导任何默认接口或模型。键盘经 `msime_client_mobile_voice_configuration` 读共享解析，`asr_endpoint` 和 `asr_model` 留空时由共享层补上该服务的默认值，所以切换服务时把这两项清空就够了，不需要在 Java 里再放一份默认值表。共享设置页切换时会保留不是任何预设的自定义地址；这里不保留，因为那个地址是为上一个服务填的，留着它会把新服务的密钥发到旧服务的主机上。
 *
 * <p>凭据沿用共享设置页的分槽写法（`swapTokenSlot`）：`asr_token` 是当前服务的密钥，`asr_tokens` 按服务 id 存其他服务的，切走时把当前的收进槽里，切回来时取出来，两边读同一份文档不会互相错位。
 */
public final class VoiceProviderPolicy {
    /** 设备自带的语音识别服务；共享解析对它不给配置，键盘因此用系统识别。 */
    public static final String SYSTEM = "system";
    public static final String LOCAL = "local";
    /** 豆包两种鉴权：新版控制台的 API Key，旧版控制台的 App ID + Access Token。空值按 API Key。 */
    public static final String AUTH_API_KEY = "api_key";
    public static final String AUTH_LEGACY = "legacy";
    private VoiceProviderPolicy() {}

    /** 要联网、要密钥的服务：除系统识别和本地模型以外的全部。 */
    public static boolean network(String provider) {
        return provider != null && !provider.isEmpty() && !SYSTEM.equals(provider) && !LOCAL.equals(provider);
    }

    /** 豆包旧版鉴权要另填 App ID；其他模式和服务只要一个密钥。 */
    public static boolean legacyAuth(String mode) {
        return AUTH_LEGACY.equals(mode == null ? "" : mode.trim());
    }

    /** 切换服务后的凭据：`token` 写回 `asr_token`，`tokens` 整个替换 `asr_tokens`。 */
    public record Tokens(String token, Map<String, String> tokens) {}

    /** 与共享设置页 `asrProviderUpdate` 的凭据部分相同：当前密钥收进旧服务的槽（空就删掉那个槽），取出新服务的；系统识别和本地模型不发请求，不留槽也不带密钥。 */
    public static Tokens switchTokens(String from, String to, String token, Map<String, String> slots) {
        Map<String, String> next = new LinkedHashMap<>();
        if (slots != null) next.putAll(slots);
        String current = token == null ? "" : token;
        if (from != null && !from.isEmpty()) {
            if (current.isEmpty()) next.remove(from);
            else next.put(from, current);
        }
        if (!network(to)) {
            next.remove(to);
            return new Tokens("", next);
        }
        String restored = next.get(to);
        return new Tokens(restored == null ? "" : restored, next);
    }

    /** 密钥、App ID 能不能存：留空表示删除；否则不能有控制字符，长度与共享层的上限一致。豆包的凭据放进 WebSocket 握手的请求头，非 ASCII 的值共享层会拒绝，这里提前拦下。 */
    public static boolean validCredential(String provider, String value) {
        String trimmed = value == null ? "" : value.trim();
        if (trimmed.isEmpty()) return true;
        if (TextPolicy.hasControl(trimmed) || !TextPolicy.boundedNonBlank(trimmed, 16 * 1024)) return false;
        return !DoubaoAsrPolicy.isStreaming(provider) || trimmed.chars().allMatch(c -> c < 0x80);
    }

    /** 手填的接口地址为什么不能存；留空（用默认地址）或可以存时为 null。豆包只走 `wss://`，其他只走 `https://`：密钥在请求头里，键盘不会往明文连接上发。 */
    public static String endpointProblem(String provider, String endpoint) {
        String value = endpoint == null ? "" : endpoint.trim();
        if (value.isEmpty()) return null;
        boolean streaming = DoubaoAsrPolicy.isStreaming(provider);
        String scheme = streaming ? "wss://" : "https://";
        if (TextPolicy.validAuthority(value, scheme, AiPolishConfiguration.MAX_ENDPOINT_LENGTH)) return null;
        return streaming ? "豆包的接口地址要以 wss:// 开头" : "接口地址要以 https:// 开头，且不能带用户名或密码";
    }

    /**
     * 选了本地模型时「识别服务」行的说明：还没选用装好的模型，或这个安装包不带、也还没下载语音运行库时，说明缺什么；都齐了为 null。本地识别不会回退到系统识别（录音不该在用户不知道时离开手机），所以这里说的是「还不能用」而不是「暂用系统识别」。
     */
    public static String localStatus(boolean modelReady, boolean runtimeReady) {
        if (!modelReady) return "还没有可用的模型，请在下面下载一个";
        return runtimeReady ? null : "还要下载本地语音运行库";
    }

    /**
     * 「识别服务」行的说明：选了要密钥的服务、共享解析却没给出配置时，说明键盘眼下其实在用系统识别。
     *
     * @param resolved 共享解析给出的服务（`VoiceConfiguration#provider`），没有配置时为 null
     */
    public static String status(String provider, String resolved) {
        return status(provider, resolved, true);
    }

    /**
     * 「识别服务」行的说明，并把系统识别服务是否真的存在纳入回退提示。
     *
     * <p>设置页以前只看共享解析结果：没有云端密钥时写「暂用系统识别」，即使设备没有
     * {@code RecognitionService} 也会这样写。这样的用户按语音键只能得到失败提示，却在
     * 设置页看不出该改用本地模型或豆包。
     */
    public static String status(String provider, String resolved, boolean platformAvailable) {
        if (!network(provider)) return SYSTEM.equals(provider) || provider == null || provider.isEmpty()
            ? platformAvailable ? "使用设备自带的语音识别服务" : "没有可用的系统识别服务，可改用本地模型或豆包" : null;
        if (resolved != null) return null;
        String fallback = platformAvailable ? "暂用系统识别" : "系统识别不可用";
        return DoubaoAsrPolicy.isStreaming(provider) ? "密钥未填写完整，" + fallback : "未填写 API Key，" + fallback;
    }
}

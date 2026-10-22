package app.msime.android;

import org.json.JSONException;
import org.json.JSONObject;

/**
 * 设置页「识别服务」的凭据校验与状态文案；服务目录、接口提示和换服务补丁经原生桥读取共享 Rust。
 *
 * <p>这里不推导任何默认接口或模型。键盘经 `msime_client_mobile_voice_configuration` 读共享解析，`asr_endpoint` 和 `asr_model` 留空时由共享层补上该服务的默认值，所以切换服务时把这两项清空就够了，不需要在 Java 里再放一份默认值表。共享设置页切换时会保留不是任何预设的自定义地址；这里不保留，因为那个地址是为上一个服务填的，留着它会把新服务的密钥发到旧服务的主机上。
 *
 * <p>凭据分槽与地址、模型的清空策略由 {@link VoiceProviderSettings} 调用共享 `provider_settings` 计算。
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

    /** 密钥、App ID 能不能存：规则由共享 Rust 预检，桥接失败时拒绝保存。 */
    public static boolean validCredential(String provider, String value) {
        try {
            String response = NativeClient.voiceCredentialIssue(provider == null ? "" : provider,
                value == null ? "" : value);
            return credentialAccepted(response);
        } catch (IllegalArgumentException | LinkageError error) {
            return false;
        }
    }

    /** 只接受共享层的严格成功信封；不把字符串布尔值或缺字段当成允许保存。 */
    private static boolean credentialAccepted(String response) {
        try {
            JSONObject value = response == null ? null : JsonPolicy.objectValue(new JSONObject(response));
            if (value == null || !value.has("accepted")) return false;
            return Boolean.TRUE.equals(JsonPolicy.strictBoolean(value.opt("accepted")));
        } catch (JSONException error) {
            return false;
        }
    }

    /** 读取共享运行时规则的设置提示；留空用默认值，失败时返回错误提示并阻止保存。 */
    public static String endpointProblem(String provider, String endpoint) {
        try {
            return endpointIssue(NativeClient.voiceEndpointIssue(endpoint == null ? "" : endpoint,
                DoubaoAsrPolicy.isStreaming(provider)));
        } catch (IllegalArgumentException | LinkageError error) {
            return ENDPOINT_UNAVAILABLE;
        }
    }

    private static final String ENDPOINT_UNAVAILABLE = "接口地址校验暂不可用，请稍后重试";

    /** 只接受完整且不矛盾的成功信封，不把缺字段或字符串布尔值当成允许保存。 */
    private static String endpointIssue(String response) {
        try {
            JSONObject value = response == null ? null : JsonPolicy.objectValue(new JSONObject(response));
            if (value == null || !value.has("issue")) return ENDPOINT_UNAVAILABLE;
            Boolean accepted = JsonPolicy.strictBoolean(value.opt("accepted"));
            if (Boolean.TRUE.equals(accepted) && value.isNull("issue")) return null;
            String issue = JsonPolicy.strictString(value.opt("issue"));
            if (Boolean.FALSE.equals(accepted) && issue != null && !issue.isEmpty()
                    && TextPolicy.boundedNonBlank(issue, 512)) return issue;
        } catch (JSONException error) {
            return ENDPOINT_UNAVAILABLE;
        }
        return ENDPOINT_UNAVAILABLE;
    }

    /** 读取共享层计算的本地模型状态文案。 */
    public static String localStatus(boolean modelReady, boolean runtimeReady) {
        return sharedStatus(LOCAL, false, true, modelReady, runtimeReady);
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
        return sharedStatus(provider == null ? "" : provider, resolved != null, platformAvailable,
            true, true);
    }

    /** 只接受共享层完整成功信封和字符串或 null 状态。 */
    private static String sharedStatus(String provider, boolean resolved, boolean platformAvailable,
            boolean modelReady, boolean runtimeReady) {
        try {
            String response = NativeClient.mobileVoiceStatus(provider, resolved, platformAvailable,
                modelReady, runtimeReady);
            return parseStatus(response);
        } catch (IllegalArgumentException | LinkageError error) {
            return null;
        }
    }

    /** 只解析共享层成功信封，拒绝缺字段、错误类型和无界文本。 */
    private static String parseStatus(String response) {
        try {
            JSONObject value = response == null ? null : JsonPolicy.objectValue(new JSONObject(response));
            if (value == null || !value.has("status") || value.isNull("status")) return null;
            String status = JsonPolicy.strictString(value.opt("status"));
            return status != null && TextPolicy.boundedNonBlank(status, 512) ? status : null;
        } catch (JSONException error) {
            return null;
        }
    }
}

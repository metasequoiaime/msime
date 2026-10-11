package app.msime.android;

/**
 * 设置页「润色」组和两个「测试」按钮的规则：可选的润色服务与显示名、提示词方案、自定义方案存在哪个字段、润色眼下为什么不生效。
 *
 * <p>和识别服务一样，这里不放默认接口和模型：键盘经共享解析读 `voice_input.polish_*`，空的接口和模型由共享层补上该服务的默认值（`voice::provider::polish_provider_defaults`），所以切换服务时清空这两项即可；凭据的分槽规则沿用 {@link VoiceProviderPolicy#switchTokens}。服务和方案的 id、显示名与共享设置页的 `POLISH_PROVIDER_OPTIONS`、`POLISH_PRESET_NAMES` 一致。
 */
public final class VoicePolishSettingsPolicy {
    /** 设置页可选的润色服务，按共享设置页的顺序。 */
    public static final String[] PROVIDERS = {"siliconflow", "openai", "deepseek", "groq"};
    private static final String[] PROVIDER_LABELS = {"SiliconFlow", "OpenAI", "DeepSeek", "Groq"};
    /** 四个内置方案和三个自定义槽位；内置方案的正文在共享层（`shared/voice/PolishPrompt.h`）。 */
    public static final String[] PROMPTS = {"cleanup", "faithful", "zh2en", "casual", "custom_1", "custom_2", "custom_3"};
    private static final String[] PROMPT_LABELS = {"精炼整理", "忠实校对", "中翻英", "口语整理", "自定义一", "自定义二", "自定义三"};

    private VoicePolishSettingsPolicy() {}

    /** 润色服务的显示名；没存时按出厂的 SiliconFlow，不认识的 id 原样显示。 */
    public static String providerLabel(String provider) {
        if (provider == null || provider.isEmpty()) return PROVIDER_LABELS[0];
        for (int index = 0; index < PROVIDERS.length; index++) {
            if (PROVIDERS[index].equals(provider)) return PROVIDER_LABELS[index];
        }
        return provider;
    }

    /** 方案的显示名；没存时按出厂的「精炼整理」，不认识的 id 原样显示。 */
    public static String promptLabel(String id) {
        if (id == null || id.isEmpty()) return PROMPT_LABELS[0];
        for (int index = 0; index < PROMPTS.length; index++) {
            if (PROMPTS[index].equals(id)) return PROMPT_LABELS[index];
        }
        return id;
    }

    /** 自定义方案的正文字段（`polish_prompt_custom_1` 等）；内置方案为 null，它们的正文不能改。 */
    public static String customField(String id) {
        return id != null && id.matches("custom_[123]") ? "polish_prompt_" + id : null;
    }

    /**
     * 「润色」开关下的说明：打开了、共享解析却没给出配置时说明润色不会生效。
     *
     * @param configured 共享解析给出了润色配置（`VoiceConfiguration#polish` 不为 null）
     */
    public static String status(boolean enabled, boolean configured, boolean hasToken) {
        if (!enabled || configured) return null;
        return hasToken ? "配置不完整，识别结果不会润色" : "未填写 API Key，识别结果不会润色";
    }

    /** 测试结果对话框的标题。 */
    public static String testTitle(boolean ok) {
        return ok ? "测试通过" : "测试未通过";
    }
}

package app.msime.android;

/**
 * 设置页「润色」组和两个「测试」按钮的规则：提示词方案、自定义方案存在哪个字段、润色眼下为什么不生效。
 *
 * <p>和识别服务一样，这里不放默认接口和模型：键盘经共享解析读 `voice_input.polish_*`，空的接口和模型由共享层补上该服务的默认值（`voice::provider::polish_provider_defaults`），所以切换服务时清空这两项即可；凭据分槽与字段清空由 {@link VoiceProviderSettings} 调用共享 Rust 计算。服务目录由 {@link VoiceProviderCatalog} 读取共享 Rust；内置方案的 id、显示名由 {@link VoicePolishPresetCatalog} 静态导出共享 Rust 目录。
 */
public final class VoicePolishSettingsPolicy {
    /** 共享内置方案和三个自定义槽位；内置方案的正文在共享层（`shared/voice/PolishPrompt.h`）。 */
    public static final String[] PROMPTS = withCustom(VoicePolishPresetCatalog.ids(), "custom_1", "custom_2", "custom_3");
    private static final String[] PROMPT_LABELS = withCustom(VoicePolishPresetCatalog.titles(), "自定义一", "自定义二", "自定义三");

    private VoicePolishSettingsPolicy() {}

    private static String[] withCustom(String[] builtIns, String... custom) {
        String[] choices = java.util.Arrays.copyOf(builtIns, builtIns.length + custom.length);
        System.arraycopy(custom, 0, choices, builtIns.length, custom.length);
        return choices;
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
        try {
            return VoiceProviderPolicy.parseStatus(
                NativeClient.mobileVoicePolishStatus(enabled, configured, hasToken));
        } catch (IllegalArgumentException | LinkageError error) {
            return null;
        }
    }

    /** 测试结果对话框的标题。 */
    public static String testTitle(boolean ok) {
        return ok ? "测试通过" : "测试未通过";
    }
}

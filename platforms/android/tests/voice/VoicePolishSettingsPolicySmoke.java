import app.msime.android.VoicePolishSettingsPolicy;

/** The settings page's polish group: providers and prompt ids shared with the React page, where custom prompts are stored, and why polishing would not run. */
public final class VoicePolishSettingsPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        // The shared preferences accept these polish providers (crates/client-core POLISH_PROVIDERS); every one offered must be among them.
        String[] accepted = {"siliconflow", "openai", "deepseek", "groq", "doubao"};
        for (String provider : VoicePolishSettingsPolicy.PROVIDERS) {
            boolean known = false;
            for (String candidate : accepted) known |= candidate.equals(provider);
            check(known, provider + " is accepted by the shared preferences validation");
        }
        check(VoicePolishSettingsPolicy.providerLabel("").equals("SiliconFlow"), "nothing stored reads as the shipped default");
        check(VoicePolishSettingsPolicy.providerLabel("deepseek").equals("DeepSeek"), "labels match the shared page");
        check(VoicePolishSettingsPolicy.providerLabel("future").equals("future"), "an unknown id is shown as stored");

        check(VoicePolishSettingsPolicy.promptLabel("").equals("精炼整理"), "nothing stored reads as the shipped cleanup preset");
        check(VoicePolishSettingsPolicy.promptLabel("zh2en").equals("中翻英")
                && VoicePolishSettingsPolicy.promptLabel("custom_2").equals("自定义二"), "preset names match the shared page");
        check(VoicePolishSettingsPolicy.promptLabel("other").equals("other"), "an unknown prompt id is shown as stored");

        check("polish_prompt_custom_1".equals(VoicePolishSettingsPolicy.customField("custom_1"))
                && "polish_prompt_custom_3".equals(VoicePolishSettingsPolicy.customField("custom_3")),
            "custom slots map to the fields the keyboard reads");
        check(VoicePolishSettingsPolicy.customField("cleanup") == null && VoicePolishSettingsPolicy.customField("custom_4") == null
                && VoicePolishSettingsPolicy.customField(null) == null, "built-in presets and unknown ids have no editable text");

        check(VoicePolishSettingsPolicy.status(false, false, false) == null, "a switched-off polish needs no remark");
        check(VoicePolishSettingsPolicy.status(true, true, true) == null, "a working configuration needs no remark");
        check("未填写 API Key，识别结果不会润色".equals(VoicePolishSettingsPolicy.status(true, false, false)),
            "a missing key says polishing will not run");
        check("配置不完整，识别结果不会润色".equals(VoicePolishSettingsPolicy.status(true, false, true)),
            "a key with a bad endpoint or model says so");

        check(VoicePolishSettingsPolicy.testTitle(true).equals("测试通过")
                && VoicePolishSettingsPolicy.testTitle(false).equals("测试未通过"), "test dialog titles");
        System.out.println("Android voice polish settings: providers, prompts, custom fields and status passed");
    }
}

import app.msime.android.VoicePolishSettingsPolicy;

/** 润色方案、自定义提示词字段和状态提示；服务名称由共享目录快照冒烟覆盖。 */
public final class VoicePolishSettingsPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
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

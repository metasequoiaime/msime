import app.msime.android.VoicePolishSettingsPolicy;

/** 润色方案、自定义提示词字段和测试标题；状态提示由真实 JNI 设备套件覆盖。 */
public final class VoicePolishSettingsPolicySmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(java.util.Arrays.equals(VoicePolishSettingsPolicy.PROMPTS,
                new String[] {"cleanup", "faithful", "zh2en", "casual", "custom_1", "custom_2", "custom_3"}),
            "方案选择表保持内置目录和三个自定义槽位的既有顺序");
        String[] labels = {"精炼整理", "忠实校对", "中翻英", "口语整理", "自定义一", "自定义二", "自定义三"};
        for (int index = 0; index < labels.length; index++) {
            check(labels[index].equals(VoicePolishSettingsPolicy.promptLabel(VoicePolishSettingsPolicy.PROMPTS[index])),
                "方案显示名保持既有顺序");
        }
        check("精炼整理".equals(VoicePolishSettingsPolicy.promptLabel(null)), "未存方案按第一项显示");
        check(VoicePolishSettingsPolicy.promptLabel("").equals("精炼整理"), "nothing stored reads as the shipped cleanup preset");
        check(VoicePolishSettingsPolicy.promptLabel("zh2en").equals("中翻英")
                && VoicePolishSettingsPolicy.promptLabel("custom_2").equals("自定义二"), "preset names match the shared page");
        check(VoicePolishSettingsPolicy.promptLabel("other").equals("other"), "an unknown prompt id is shown as stored");

        check("polish_prompt_custom_1".equals(VoicePolishSettingsPolicy.customField("custom_1"))
                && "polish_prompt_custom_3".equals(VoicePolishSettingsPolicy.customField("custom_3")),
            "custom slots map to the fields the keyboard reads");
        check(VoicePolishSettingsPolicy.customField("cleanup") == null && VoicePolishSettingsPolicy.customField("custom_4") == null
                && VoicePolishSettingsPolicy.customField(null) == null, "built-in presets and unknown ids have no editable text");

        check(VoicePolishSettingsPolicy.testTitle(true).equals("测试通过")
                && VoicePolishSettingsPolicy.testTitle(false).equals("测试未通过"), "test dialog titles");
        System.out.println("Android voice polish settings: providers, prompts, custom fields and status passed");
    }
}

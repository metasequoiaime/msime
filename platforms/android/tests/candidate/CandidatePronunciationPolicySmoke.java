import app.msime.android.CandidatePronunciationPolicy;

public final class CandidatePronunciationPolicySmoke {
    public static void main(String[] args) {
        String key = CandidatePronunciationPolicy.key("en", "world");
        check(key.equals(CandidatePronunciationPolicy.lookupText("world", "", "en"))
                && key.equals(CandidatePronunciationPolicy.lookupText("world", "", "fr")),
            "英文候选直接读候选词，不看目标语言");
        check(CandidatePronunciationPolicy.key("en", "to love; affection").equals(
                CandidatePronunciationPolicy.lookupText("爱", "to love; affection", "en")),
            "中文候选读英语释义首行，具体取首词组交给共享层");
        check(CandidatePronunciationPolicy.key("en", "(of a person) kind").equals(CandidatePronunciationPolicy.lookupText(
                "善", "(of a person) kind", "en")),
            "括号注解交给共享层按统一规则去除");
        check(CandidatePronunciationPolicy.key("fr", "aimer").equals(CandidatePronunciationPolicy.lookupText("爱", "aimer", "fr")),
            "法语目标按法语读，不误用英语音标");
        check(CandidatePronunciationPolicy.key("ja", "愛する").equals(CandidatePronunciationPolicy.lookupText("爱", "愛する", "ja")),
            "日语释义交给共享层读罗马字");
        check(CandidatePronunciationPolicy.lookupText("爱", "aimer", "zh").isEmpty(),
            "没有读音表的语言不请求");
        check(CandidatePronunciationPolicy.lookupText("123", "", "en").isEmpty(),
            "没有英文词的候选不请求");
        check(CandidatePronunciationPolicy.lookupText("爱", "to love\taffection", "en").isEmpty()
                && CandidatePronunciationPolicy.lookupText("爱", "love\u0007me", "en").isEmpty(),
            "带控制字符的释义不请求，免得共享层拒绝整批");
        check("，音标：[ˈkoʂkə]".equals(CandidatePronunciationPolicy.accessibilitySuffix("[ˈkoʂkə]"))
                && "，读音：neko".equals(CandidatePronunciationPolicy.accessibilitySuffix("neko"))
                && "，读音：goyang-i".equals(CandidatePronunciationPolicy.accessibilitySuffix("goyang-i"))
                && CandidatePronunciationPolicy.accessibilitySuffix("").isEmpty(),
            "读屏念音标或读音");

        // 回答的形状按语言收：英文只收 /…/，其他语言也收 […]，日文和韩文是罗马字。org.json 在 JVM 冒烟里只有桩，请求和回答的拼装由共享层的测试覆盖。
        check(app.msime.android.CandidatePronunciationModel.wellFormed("en", "/həˈləʊ/")
                && !app.msime.android.CandidatePronunciationModel.wellFormed("en", "[x]")
                && app.msime.android.CandidatePronunciationModel.wellFormed("ru", "[ˈkoʂkə]")
                && app.msime.android.CandidatePronunciationModel.wellFormed("fr", "/ʃa/")
                && !app.msime.android.CandidatePronunciationModel.wellFormed("fr", "ʃa")
                && app.msime.android.CandidatePronunciationModel.wellFormed("ja", "neko")
                && !app.msime.android.CandidatePronunciationModel.wellFormed("ja", " ")
                && app.msime.android.CandidatePronunciationModel.wellFormed("ko", "goyang-i")
                && !app.msime.android.CandidatePronunciationModel.wellFormed("ko", " "),
            "按语言校验读音的形状");
        check("注释\n/IPA/".equals(CandidatePronunciationPolicy.withReading("注释", "/IPA/")),
            "音标独占一行");
        check("注释".equals(CandidatePronunciationPolicy.withReading("注释", "")),
            "无音标保持原布局");
        check(!CandidatePronunciationPolicy.visible(false, true)
                && !CandidatePronunciationPolicy.visible(true, false)
                && CandidatePronunciationPolicy.visible(true, true),
            "开关和资源同时可用才显示");
        check("\u00A0".equals(CandidatePronunciationPolicy.suggestionAnnotation(true, ""))
                && "/həˈləʊ/".equals(CandidatePronunciationPolicy.suggestionAnnotation(true, "/həˈləʊ/"))
                && CandidatePronunciationPolicy.suggestionAnnotation(false, "/həˈləʊ/").isEmpty(),
            "英文直输建议查音标前占位、得到音标后显示、关闭后恢复单行");
    }

    private static void check(boolean ok, String message) {
        if (!ok) throw new AssertionError(message);
    }
}

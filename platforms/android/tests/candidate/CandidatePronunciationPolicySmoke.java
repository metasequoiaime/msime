import app.msime.android.CandidatePronunciationPolicy;

public final class CandidatePronunciationPolicySmoke {
    public static void main(String[] args) {
        check("world".equals(CandidatePronunciationPolicy.lookupText("world", "", true)),
            "英文候选直接读候选词");
        check("to love; affection".equals(CandidatePronunciationPolicy.lookupText("爱", "to love; affection", true)),
            "中文候选读英语释义首行，具体取首词组交给共享层");
        check("(of a person) kind".equals(CandidatePronunciationPolicy.lookupText(
                "善", "(of a person) kind", true)),
            "括号注解交给共享层按统一规则去除");
        check(CandidatePronunciationPolicy.lookupText("爱", "aimer", false).isEmpty(),
            "非英语目标不误用英语音标");
        check(CandidatePronunciationPolicy.lookupText("123", "", true).isEmpty(),
            "没有英文词的候选不请求");
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

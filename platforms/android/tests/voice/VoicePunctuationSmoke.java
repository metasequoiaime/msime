package app.msime.android;

/** 语音标点（#10558）：取值和共享层一致，默认不改识别结果，共享层调用不了时不丢内容。 */
public final class VoicePunctuationSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        check(VoicePunctuation.MODES.length == VoicePunctuation.LABELS.length,
            "every punctuation mode has a settings label");
        check(VoicePunctuation.AUTO.equals(VoicePunctuation.MODES[0]),
            "the factory default is listed first, as 智能添加");
        for (String mode : VoicePunctuation.MODES) {
            check(mode.equals(VoicePunctuation.mode(mode)), "a known mode is kept: " + mode);
        }
        check(VoicePunctuation.AUTO.equals(VoicePunctuation.mode(null)),
            "a missing preference reads as auto");
        check(VoicePunctuation.AUTO.equals(VoicePunctuation.mode("from_a_newer_version")),
            "an unknown mode reads as auto, as the shared layer does");
        check(VoicePunctuation.AUTO.equals(VoicePunctuation.mode(1)),
            "a non-string mode is not coerced");
        check("省略句末标点".equals(VoicePunctuation.label("omit_trailing"))
                && "智能添加".equals(VoicePunctuation.label("from_a_newer_version")),
            "the settings row names the stored mode, and auto for anything else");

        String recognized = "你好。";
        // auto 不经过共享层：同一个对象原样返回。
        check(VoicePunctuation.apply(recognized, VoicePunctuation.AUTO) == recognized,
            "auto leaves the recognizer output untouched");
        check(VoicePunctuation.apply(recognized, "from_a_newer_version") == recognized,
            "an unknown mode leaves the recognizer output untouched");
        check(VoicePunctuation.apply(null, "omit_all") == null
                && "".equals(VoicePunctuation.apply("", "omit_all")),
            "no text stays no text");
        // JVM 上没有 org.json 和原生库，共享层调用失败：交回原文，不能把识别内容丢掉。
        check(recognized.equals(VoicePunctuation.apply(recognized, "omit_trailing")),
            "a failed shared call keeps the transcript");
        System.out.println("Android voice punctuation keeps auto and unknown modes untouched");
    }
}

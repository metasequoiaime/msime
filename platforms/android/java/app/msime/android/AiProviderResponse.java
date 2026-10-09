package app.msime.android;

/** Shared validation for provider response fields that must remain plain text. */
final class AiProviderResponse {
    private AiProviderResponse() {}

    /** 统一限制语音转写文本的字符数、控制字符和 Unicode 合法性。 */
    static String boundedText(Object value, int maxCodePoints) {
        String text = JsonPolicy.strictStringOrEmpty(value);
        return TextPolicy.codePointLength(text) <= maxCodePoints
                && !TextPolicy.hasControlExceptWhitespace(text)
                && TextPolicy.validUnicode(text) ? text : "";
    }
}

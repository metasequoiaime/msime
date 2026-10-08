package app.msime.android;

/** Shared validation for provider response fields that must remain plain text. */
final class AiProviderResponse {
    private AiProviderResponse() {}

    static String strictContent(Object value) {
        return JsonPolicy.strictStringOrEmpty(value);
    }

    static String strictText(Object value) {
        return JsonPolicy.strictStringOrEmpty(value);
    }
}

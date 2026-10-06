package app.msime.android;

/** Shared validation for provider response fields that must remain plain text. */
final class AiProviderResponse {
    private AiProviderResponse() {}

    static String strictContent(Object value) {
        return value instanceof String ? (String) value : "";
    }

    static String strictText(Object value) {
        return value instanceof String ? (String) value : "";
    }
}

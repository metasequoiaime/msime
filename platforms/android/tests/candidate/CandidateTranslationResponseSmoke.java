package app.msime.android;

import java.math.BigDecimal;
import java.util.Arrays;
import java.util.List;

public final class CandidateTranslationResponseSmoke {
    public static void main(String[] args) throws Exception {
        check(!BackendTranslationClient.successStatusCode(200.5),
            "fractional translation status codes must reject the response");
        check(!BackendTranslationClient.successStatusCode(new BigDecimal("200.0000000000000000001")),
            "precise fractional translation status codes must reject the response");
        check(!BackendTranslationClient.successStatusCode(200.0),
            "rounded fractional JSON translation status codes must reject the response");
        check(!BackendTranslationClient.successStatusCode(true),
            "boolean translation status codes must reject the response");
        check(BackendTranslationClient.successStatusCode(200),
            "integer translation status codes must be accepted");

        List<String> translations = BackendTranslationClient.parseValues(
            Arrays.asList("你好", null), 2);
        check(translations == null, "JSON null translation must reject the response");

        translations = BackendTranslationClient.parseValues(
            Arrays.asList("你好", 42), 2);
        check(translations == null, "non-string translation must reject the response");

        translations = BackendTranslationClient.parseValues(
            Arrays.asList("你好", "hello"), 2);
        check(translations != null && translations.equals(List.of("你好", "hello")),
            "string translations must be returned unchanged");
        System.out.println("candidate translation response types passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

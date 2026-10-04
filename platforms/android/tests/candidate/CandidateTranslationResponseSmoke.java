package app.msime.android;

import java.util.Arrays;
import java.util.List;

public final class CandidateTranslationResponseSmoke {
    public static void main(String[] args) throws Exception {
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

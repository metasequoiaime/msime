package app.msime.android;

import java.net.URI;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

/** Model-directory URLs preserve provider query parameters and encode cursors exactly once. */
public final class AiPolishModelCatalogSmoke {
    static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] arguments) throws Exception {
        URI endpoint = new URI(
            "https://api.anthropic.com/v1/chat/completions?tenant=acme%26west&limit=50");
        URI base = AiPolishModelCatalog.modelsUri(endpoint);
        check(base.toASCIIString().equals(
            "https://api.anthropic.com/v1/models?tenant=acme%26west&limit=50"),
            "model URL must preserve encoded provider query parameters");

        String cursor = URLEncoder.encode("cursor/a+b c", StandardCharsets.UTF_8.name());
        URI page = AiPolishModelCatalog.withQuery(base, "limit=1000&after_id=" + cursor);
        check(page.toASCIIString().equals(
            "https://api.anthropic.com/v1/models?limit=1000&after_id=cursor%2Fa%2Bb+c"),
            "pagination cursor must be encoded exactly once");
        URI anthropicPage = AiPolishModelCatalog.withAnthropicQuery(base, "cursor/a+b c");
        check(anthropicPage.toASCIIString().equals(
            "https://api.anthropic.com/v1/models?tenant=acme%26west&limit=1000&after_id=cursor%2Fa%2Bb+c"),
            "Anthropic pagination must retain unrelated provider query parameters");
        java.lang.reflect.Method strictString = AiPolishModelCatalog.class.getDeclaredMethod(
            "strictString", Object.class);
        strictString.setAccessible(true);
        check("synthetic".equals(strictString.invoke(null, "synthetic")),
            "AI model metadata accepts JSON strings");
        check(strictString.invoke(null, 7) == null,
            "AI model metadata rejects numbers instead of coercing them");
        java.lang.reflect.Method strictBoolean = AiPolishModelCatalog.class.getDeclaredMethod(
            "strictBoolean", Object.class);
        strictBoolean.setAccessible(true);
        check(Boolean.TRUE.equals(strictBoolean.invoke(null, Boolean.TRUE)),
            "AI model metadata accepts JSON booleans");
        check(strictBoolean.invoke(null, "true") == null,
            "AI model metadata rejects boolean strings instead of coercing them");
        System.out.println("Android AI model catalog URL handling passed");
    }
}

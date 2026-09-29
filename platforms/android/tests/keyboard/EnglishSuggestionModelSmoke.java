import app.msime.android.candidate.EnglishSuggestionModel;

public final class EnglishSuggestionModelSmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) throws Exception {
        var result = EnglishSuggestionModel.decode(
            "{\"ok\":true,\"value\":{\"prefix\":\"iph\",\"items\":[\"iPhone\",\"iphone\"]}}");
        check(result.prefix().equals("iph"), "prefix");
        check(result.items().size() == 2 && result.items().get(0).equals("iPhone"), "items");
        boolean rejected = false;
        try {
            EnglishSuggestionModel.decode("{\"ok\":false,\"error\":\"failed\"}");
        } catch (IllegalArgumentException expected) {
            rejected = true;
        }
        check(rejected, "failed response");
        System.out.println("Android English suggestion response model: bounds passed");
    }
}

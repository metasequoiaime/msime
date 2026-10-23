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
        var original = new EnglishSuggestionModel.Result("eng", java.util.List.of("eng"));
        var words = new EnglishSuggestionModel.Result("eng", java.util.List.of("English", "engine", "engineering"));
        check(EnglishSuggestionModel.withCompletions(original, words).items()
            .equals(java.util.List.of("eng", "English", "engine", "engineering")),
            "三字母前缀列出已有词库的补全，原文仍为首项");
        var correction = new EnglishSuggestionModel.Result("eng", java.util.List.of("eng", "Eng"));
        var duplicates = new EnglishSuggestionModel.Result("eng", java.util.List.of("eng", "Eng", "English", "english"));
        check(EnglishSuggestionModel.withCompletions(correction, duplicates).items()
            .equals(java.util.List.of("eng", "Eng", "English")),
            "保留原有纠错项，新增补全按忽略大小写去重");
        check(EnglishSuggestionModel.withCompletions(original,
            new EnglishSuggestionModel.Result("hel", java.util.List.of("hello"))).equals(original),
            "不同前缀的结果不能合并");
        var many = new java.util.ArrayList<String>();
        for (int index = 0; index < 32; index++) many.add("eng" + index);
        check(EnglishSuggestionModel.withCompletions(correction,
            new EnglishSuggestionModel.Result("eng", many)).items().size() == 32,
            "合并结果不超过响应上限");
        System.out.println("Android English suggestion response model: bounds passed");
    }
}

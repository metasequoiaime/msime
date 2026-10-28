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
        check(!result.corrected(), "不带 completions 的响应没有纠错标记");
        var correction = EnglishSuggestionModel.decode(
            "{\"ok\":true,\"value\":{\"corrected\":true,\"items\":[\"teh\",\"the\"],\"prefix\":\"teh\"}}");
        check(correction.corrected() && correction.items().equals(java.util.List.of("teh", "the")),
            "合并展示的第二行是纠错");
        var completions = EnglishSuggestionModel.decode(
            "{\"ok\":true,\"value\":{\"corrected\":false,\"items\":[\"eng\",\"English\",\"engine\"],\"prefix\":\"eng\"}}");
        check(!completions.corrected() && completions.items().equals(java.util.List.of("eng", "English", "engine")),
            "没有纠错时原文后接前缀补全，按引擎顺序展示");
        boolean malformed = false;
        try {
            EnglishSuggestionModel.decode(
                "{\"ok\":true,\"value\":{\"corrected\":true,\"items\":[\"teh\"],\"prefix\":\"teh\"}}");
        } catch (IllegalArgumentException expected) {
            malformed = true;
        }
        check(malformed, "标为纠错却没有恰好两行的响应不可信");
        System.out.println("Android English suggestion response model: bounds and correction flag passed");
    }
}

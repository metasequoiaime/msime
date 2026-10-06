import app.msime.android.OnlineCandidatePolicy;
import java.util.Arrays;
import java.util.List;
import java.net.URL;

public final class OnlineCandidatePolicySmoke {
    static void check(boolean condition) { if (!condition) throw new AssertionError(); }

    private static String signature(boolean cloud, String assistant) {
        return OnlineCandidatePolicy.signature(7, "ni'hao", "fixture", cloud, assistant);
    }

    public static void main(String[] args) throws Exception {
        check(OnlineCandidatePolicy.sessionId(7L, -1) == 7);
        check(OnlineCandidatePolicy.sessionId(7.5, -1) == -1);
        check(OnlineCandidatePolicy.sessionId(true, -1) == -1);
        check(OnlineCandidatePolicy.sessionId("7", -1) == -1);
        check(OnlineCandidatePolicy.sessionId(0L, -1) == -1);
        // Only a query the provider could actually answer is asked at all.
        check(OnlineCandidatePolicy.requestsCloud(true, true));
        check(!OnlineCandidatePolicy.requestsCloud(false, true));
        check(!OnlineCandidatePolicy.requestsCloud(true, false));
        check(OnlineCandidatePolicy.requestsAi(true, true));
        check(!OnlineCandidatePolicy.requestsAi(false, true));
        check(!OnlineCandidatePolicy.requestsAi(true, false));

        check(!OnlineCandidatePolicy.validURL(new URL("https://user:password@example.invalid/translate")));
        check(!OnlineCandidatePolicy.validURL(new URL("https://example.invalid/translate#fragment")));
        check(!OnlineCandidatePolicy.validURL(new URL("https:///translate")));
        check(OnlineCandidatePolicy.validURL(new URL("https://example.invalid/translate")));

        check(OnlineCandidatePolicy.aiCandidateLimit(3) == 3);
        check(OnlineCandidatePolicy.aiCandidateLimit(1) == 1);
        check(OnlineCandidatePolicy.aiCandidateLimit(10) == 10);
        check(OnlineCandidatePolicy.aiCandidateLimit(0) == 0);
        check(OnlineCandidatePolicy.aiCandidateLimit(-1) == 0);
        check(OnlineCandidatePolicy.aiCandidateLimit(11) == 0);

        // The same composition is asked about once; a changed AI configuration asks again.
        check(signature(true, "{\"model\":\"a\"}").equals(signature(true, "{\"model\":\"a\"}")));
        check(!signature(true, "{\"model\":\"a\"}").equals(signature(true, "{\"model\":\"b\"}")));
        check(!signature(true, "{\"model\":\"a\"}").equals(signature(false, "{\"model\":\"a\"}")));
        // A disabled assistant contributes nothing, so its settings must not re-ask on their own.
        check(signature(true, "").equals(signature(true, null)));
        check(!OnlineCandidatePolicy.signature(7, "ni'hao", "fixture", true, "")
            .equals(OnlineCandidatePolicy.signature(8, "ni'hao", "fixture", true, "")));
        check(!OnlineCandidatePolicy.signature(7, "ni'hao", "fixture", true, "")
            .equals(OnlineCandidatePolicy.signature(7, "ni'hao'ma", "fixture", true, "")));
        check(!OnlineCandidatePolicy.signature(7, "ni'hao", "fixture", true, "")
            .equals(OnlineCandidatePolicy.signature(7, "ni'hao", "other", true, "")));
        check(!OnlineCandidatePolicy.signature(7, "a:b", "c", true, "")
            .equals(OnlineCandidatePolicy.signature(7, "a", "b:c", true, "")));

        // 失败请求只有仍属于当前会话和代次时才能重试；迟到失败不能清除新请求的签名。
        String failed = signature(true, "");
        check(OnlineCandidatePolicy.shouldReleaseAfterFailure(failed, failed, 4, 4, 7, 7));
        check(!OnlineCandidatePolicy.shouldReleaseAfterFailure(failed,
            signature(true, "{\"model\":\"new\"}"), 4, 4, 7, 7));
        check(!OnlineCandidatePolicy.shouldReleaseAfterFailure(failed, failed, 3, 4, 7, 7));
        check(!OnlineCandidatePolicy.shouldReleaseAfterFailure(failed, failed, 4, 4, 8, 7));

        // Provider order is kept, duplicates drop out, and the limit caps the result.
        check(OnlineCandidatePolicy.aiCandidates(
            Arrays.asList("你好", "您好", "你好", "哈喽", "嗨"), 3).equals(List.of("你好", "您好", "哈喽")));
        // One unusable entry is skipped rather than discarding the usable ones beside it.
        check(OnlineCandidatePolicy.aiCandidates(
            Arrays.asList("  ", "坏\u0007的", null, "好的"), 3).equals(List.of("好的")));
        check(OnlineCandidatePolicy.aiCandidates(
            Arrays.asList("x".repeat(4097), "短"), 3).equals(List.of("短")));
        check(OnlineCandidatePolicy.aiCandidates(List.of(), 3).isEmpty());
        check(OnlineCandidatePolicy.aiCandidates(null, 3).isEmpty());
        // A limit the shared host would reject contributes nothing at all.
        check(OnlineCandidatePolicy.aiCandidates(List.of("你好"), 0).isEmpty());
        check(OnlineCandidatePolicy.aiCandidates(List.of("你好"), 11).isEmpty());

        check(OnlineCandidatePolicy.acceptsCloudBody("{\"result\":[]}"));
        check(!OnlineCandidatePolicy.acceptsCloudBody(""));
        check(!OnlineCandidatePolicy.acceptsCloudBody(null));
        check(!OnlineCandidatePolicy.acceptsCloudBody(
            "x".repeat(OnlineCandidatePolicy.MAX_CLOUD_RESPONSE_BYTES + 1)));
        check(OnlineCandidatePolicy.acceptsAiBody("{}"));
        check(!OnlineCandidatePolicy.acceptsAiBody(null));
        check(!OnlineCandidatePolicy.acceptsAiBody(
            "x".repeat(OnlineCandidatePolicy.MAX_AI_RESPONSE_BYTES + 1)));
        check(OnlineCandidatePolicy.acceptsAiContent("{\"candidates\":[]}"));
        check(!OnlineCandidatePolicy.acceptsAiContent(""));
        check(!OnlineCandidatePolicy.acceptsAiContent(
            "x".repeat(OnlineCandidatePolicy.MAX_AI_CONTENT_BYTES + 1)));
        // A multi-byte scalar counts its UTF-8 bytes, not its UTF-16 units.
        check(!OnlineCandidatePolicy.acceptsAiContent(
            "好".repeat(OnlineCandidatePolicy.MAX_AI_CONTENT_BYTES / 3 + 1)));

        check(OnlineCandidatePolicy.QUIET_INTERVAL_MILLIS == 350);
        System.out.println(
            "Android online candidates: identity, eligibility and response bounds passed");
    }
}

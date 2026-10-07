import app.msime.android.CloudApi;
import app.msime.android.SkinJobsApi;
import java.util.List;
import java.util.concurrent.FutureTask;
import java.util.concurrent.atomic.AtomicBoolean;

/** AI 设计皮肤的失败分类与提示文案；请求本身走 org.json，在 check-host 的桩 classpath 下跑不了，留给设备上验证。 */
public final class SkinJobsApiSmoke {
    public static void main(String[] arguments) throws Exception {
        try {
            java.lang.reflect.Method strictBoolean = SkinJobsApi.class.getDeclaredMethod(
                "strictBoolean", Object.class);
            strictBoolean.setAccessible(true);
            check(Boolean.TRUE.equals(strictBoolean.invoke(null, Boolean.TRUE)),
                "skin jobs responses accept JSON booleans");
            check(strictBoolean.invoke(null, "true") == null,
                "skin jobs responses reject boolean strings instead of coercing them");
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("skin jobs response policy missing", error);
        }
        check(SkinJobsApi.strictArtworkDimension(512L) == 512,
            "artwork dimensions accept JSON integers");
        check(SkinJobsApi.strictArtworkDimension(1.5d) == null,
            "artwork dimensions reject fractional JSON numbers");
        CloudApi.Failure quota = new CloudApi.Failure(429, "rate_limit_exceeded", "", 7200);
        check(SkinJobsApi.quotaExhausted(quota), "429 is the daily quota");
        check(SkinJobsApi.message(quota).startsWith("今天的生成次数已用完"), "quota wording");
        check(SkinJobsApi.message(quota).contains("2 小时"), "retry-after rounds up to hours");
        check(SkinJobsApi.message(new CloudApi.Failure(429, "rate_limit_exceeded", "", 0)).equals("今天的生成次数已用完"),
            "quota without retry-after");

        CloudApi.Failure disabled = new CloudApi.Failure(503, "service_disabled", "", 0);
        check(SkinJobsApi.unavailable(disabled), "503 hides the entry");
        check(SkinJobsApi.unavailable(new CloudApi.Failure(503, "", "", 0)), "any 503 hides the entry");
        check(!SkinJobsApi.unavailable(quota), "quota is not unavailable");

        check(SkinJobsApi.validPlanCount(1), "one generated plan is accepted");
        check(SkinJobsApi.validPlanCount(SkinJobsApi.MAX_DESIGNS),
            "the design limit is accepted");
        check(!SkinJobsApi.validPlanCount(0), "an empty plan list is refused");
        check(!SkinJobsApi.validPlanCount(SkinJobsApi.MAX_DESIGNS + 1),
            "plans beyond the design limit are refused");

        check(SkinJobsApi.message(new CloudApi.Failure(0, "cancelled", "", 0)).equals("已取消"),
            "local cancellation is not a network failure");
        check(SkinJobsApi.message(new CloudApi.Failure(0, "ai_skin_response", "", 0)).contains("不合格"),
            "refused plans");
        check(SkinJobsApi.message(new CloudApi.Failure(0, "network", "", 0)).startsWith("连不上服务器"),
            "network failure");
        check(SkinJobsApi.message(new CloudApi.Failure(401, "signed_out", "", 0)).equals("请先登录后再生成"),
            "signed out");

        // If a later plan is malformed after earlier illustrations were submitted, the
        // in-flight jobs must enter cancellation so their finally blocks release them.
        AtomicBoolean cancelled = new AtomicBoolean(false);
        FutureTask<Void> submitted = new FutureTask<>(() -> null);
        try {
            java.lang.reflect.Method cancel = SkinJobsApi.class.getDeclaredMethod(
                "cancelSubmitted", AtomicBoolean.class, List.class);
            cancel.setAccessible(true);
            cancel.invoke(null, cancelled, List.of(submitted));
        } catch (ReflectiveOperationException error) {
            throw new AssertionError("submitted illustrations need cancellation cleanup", error);
        }
        check(cancelled.get() && submitted.isCancelled(),
            "malformed plan cancels submitted illustrations");
        System.out.println("SkinJobsApiSmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

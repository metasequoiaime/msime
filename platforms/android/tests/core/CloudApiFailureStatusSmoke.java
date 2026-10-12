import app.msime.android.CloudApi;

public final class CloudApiFailureStatusSmoke {
    public static void main(String[] arguments) {
        check(new CloudApi.Failure(429, "rate_limit_exceeded", "", 0).rateLimited(),
            "HTTP 429 is rate limited");
        check(!new CloudApi.Failure(500, "rate_limit_exceeded", "", 0).rateLimited(),
            "HTTP 500 is not rate limited");
        check(!new CloudApi.Failure(0, "network", "", 0).rateLimited(),
            "network failures are not rate limited");
        check(new CloudApi.Failure(413, "payload_too_large", "", 0).payloadTooLarge(),
            "HTTP 413 is payload too large");
        check(new CloudApi.Failure(415, "unsupported_media_type", "", 0).unsupportedMediaType(),
            "HTTP 415 is unsupported media type");
        check(!new CloudApi.Failure(400, "invalid_avatar", "", 0).payloadTooLarge(),
            "HTTP 400 is not payload too large");
        System.out.println("CloudApiFailureStatusSmoke ok");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

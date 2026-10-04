package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * Bounds and identifies asynchronous cloud and AI results before they return to Engine.
 *
 * <p>Both providers answer a query Engine has usually moved past by the time the network replies.
 * The signature is what stops a second request for a state already asked about, and the byte and
 * shape limits are what stop a service reply from reaching Engine as anything other than a short
 * list of plain candidate strings.
 *
 * <p>The rules live here without any JSON or Android dependency so the host test suite can run
 * them; reading the service envelopes stays in the input service, where JSON already lives.
 */
public final class OnlineCandidatePolicy {
    /** How long a composition has to hold still before either provider is asked. */
    public static final long QUIET_INTERVAL_MILLIS = 350;
    public static final int MAX_CLOUD_RESPONSE_BYTES = 256 * 1024;
    public static final int MAX_AI_RESPONSE_BYTES = 1024 * 1024;
    public static final int MAX_AI_CONTENT_BYTES = 64 * 1024;
    private static final int MAX_CANDIDATE_BYTES = 4096;
    private static final int MAX_CANDIDATE_LIMIT = 10;

    private OnlineCandidatePolicy() {}

    /** Read the positive host session id without JSONObject's lossy numeric conversions. */
    public static long sessionId(Object raw, long fallback) {
        long value = KeyboardGeometry.strictLong(raw, fallback);
        return value > 0 ? value : fallback;
    }

    /**
     * Identity of one online request.
     *
     * <p>{@code assistant} is the AI configuration document when the assistant is enabled and
     * empty otherwise: changing model or prompt has to ask again for the same composition, while a
     * query that only advanced its generation without changing the cache key must not, and a
     * disabled assistant's settings are not going to be consulted either way.
     */
    public static String signature(long sessionId, String cacheKey, String identity,
            boolean cloudCandidates, String assistant) {
        return "session=" + sessionId + "|cache=" + field(cacheKey)
            + "|identity=" + field(identity) + "|cloud=" + cloudCandidates
            + "|assistant=" + field(assistant);
    }

    /** 判断失败请求是否仍可释放当前签名，让同一输入在下一次渲染时重试。 */
    public static boolean shouldReleaseAfterFailure(String requestSignature,
            String currentSignature, long requestEpoch, long currentEpoch,
            long targetSession, long currentSession) {
        return requestEpoch == currentEpoch && targetSession == currentSession
            && requestSignature != null && requestSignature.equals(currentSignature);
    }

    /** Whether the cloud provider should be asked for this query. */
    public static boolean requestsCloud(boolean cloudCandidates, boolean cloudEligible) {
        return cloudCandidates && cloudEligible;
    }

    /** Whether the AI provider should be asked for this query. */
    public static boolean requestsAi(boolean aiEligible, boolean assistantEnabled) {
        return aiEligible && assistantEnabled;
    }

    /** The configured candidate limit, or zero when it is outside what the shared host accepts. */
    public static int aiCandidateLimit(int limit) {
        return limit >= 1 && limit <= MAX_CANDIDATE_LIMIT ? limit : 0;
    }

    /** Whether a cloud body is small enough to hand to the shared parser. */
    public static boolean acceptsCloudBody(String body) {
        return body != null && !body.isEmpty() && TextPolicy.utf8Length(body) <= MAX_CLOUD_RESPONSE_BYTES;
    }

    /** Whether an AI reply is small enough to parse. */
    public static boolean acceptsAiBody(String body) {
        return body != null && !body.isEmpty() && TextPolicy.utf8Length(body) <= MAX_AI_RESPONSE_BYTES;
    }

    /** Whether the JSON-mode content inside an AI reply is small enough to parse. */
    public static boolean acceptsAiContent(String content) {
        return content != null && !content.isEmpty() && TextPolicy.utf8Length(content) <= MAX_AI_CONTENT_BYTES;
    }

    /**
     * The candidates a reply may contribute: provider order, no duplicates, capped at the limit.
     *
     * <p>Blank, control-bearing and over-long entries are skipped rather than failing the batch,
     * matching the shared parser: one unusable entry does not discard the usable ones beside it.
     */
    public static List<String> aiCandidates(List<String> texts, int limit) {
        List<String> result = new ArrayList<>();
        if (texts == null || aiCandidateLimit(limit) == 0) return result;
        for (String text : texts) {
            if (result.size() == limit) break;
            if (text == null || text.trim().isEmpty() || TextPolicy.utf8Length(text) > MAX_CANDIDATE_BYTES
                    || TextPolicy.hasControl(text) || result.contains(text)) {
                continue;
            }
            result.add(text);
        }
        return result;
    }

    private static String text(String value) { return value == null ? "" : value; }

    private static String field(String value) {
        value = text(value);
        return value.length() + ":" + value;
    }

}

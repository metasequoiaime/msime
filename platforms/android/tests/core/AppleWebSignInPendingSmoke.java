package app.msime.android;

import java.security.SecureRandom;

public final class AppleWebSignInPendingSmoke {
    public static void main(String[] arguments) {
        String grant = AppleWebSignIn.newVerifier(new SecureRandom());
        check(AppleWebSignIn.acceptableCallback(grant, null), "a well-shaped grant is accepted");
        check(AppleWebSignIn.acceptableCallback(null, "user_cancelled_authorize"), "a short error code is accepted");
        check(!AppleWebSignIn.acceptableCallback(null, null), "neither grant nor error is dropped");
        check(!AppleWebSignIn.acceptableCallback("", ""), "empty grant and error are dropped");
        check(!AppleWebSignIn.acceptableCallback(grant, "access_denied"), "both grant and error are dropped");
        check(!AppleWebSignIn.acceptableCallback("junk", null), "a malformed grant is dropped");
        check(!AppleWebSignIn.acceptableCallback(null, "Access-Denied"), "an error outside [a-z_] is dropped");
        check(!AppleWebSignIn.acceptableCallback(null, "a".repeat(65)), "an overlong error is dropped");

        check(AppleWebSignIn.keepPendingAfter(400) && AppleWebSignIn.keepPendingAfter(401)
            && AppleWebSignIn.keepPendingAfter(404), "an unknown, used or foreign grant keeps the flow");
        check(!AppleWebSignIn.keepPendingAfter(0) && !AppleWebSignIn.keepPendingAfter(500)
            && !AppleWebSignIn.keepPendingAfter(429), "network, server and rate-limit failures end the flow");

        MemoryPreferences store = new MemoryPreferences();
        long now = 1_700_000_000_000L;
        String verifier = AppleWebSignIn.newVerifier(new SecureRandom());
        store.edit().putString(AppleWebSignIn.KEY_VERIFIER, verifier).putString(AppleWebSignIn.KEY_PURPOSE, "login")
            .putLong(AppleWebSignIn.KEY_CREATED_AT, now - 1_000L).commit();

        // 形状不对的回调在读记录之前就被丢弃（AuthRedirectActivity 先调 acceptableCallback）；读记录本身也不删除它。
        AppleWebSignIn.Pending first = AppleWebSignIn.peekPending(store, now);
        check(first != null && verifier.equals(first.verifier()), "peek returns the live flow");
        check(AppleWebSignIn.peekPending(store, now) != null, "peek leaves the flow in place");

        // 伪造或旧标签页的 grant 被服务端以 401 拒绝：不清记录，下一次真正的回调仍能拿到同一个 verifier。
        if (!AppleWebSignIn.keepPendingAfter(401)) AppleWebSignIn.clearPending(store, first.verifier());
        AppleWebSignIn.Pending genuine = AppleWebSignIn.peekPending(store, now);
        check(genuine != null && verifier.equals(genuine.verifier()), "a rejected grant leaves the flow for the real callback");

        // 只按 verifier 清：期间开始了新流程，旧兑换的收尾不能把新流程删掉。
        AppleWebSignIn.clearPending(store, "someone-else");
        check(AppleWebSignIn.peekPending(store, now) != null, "a different verifier does not clear the flow");
        AppleWebSignIn.clearPending(store, verifier);
        check(AppleWebSignIn.peekPending(store, now) == null, "the matching verifier clears the flow");

        store.edit().putString(AppleWebSignIn.KEY_VERIFIER, verifier)
            .putString(AppleWebSignIn.KEY_PURPOSE, "link")
            .putString(AppleWebSignIn.KEY_SESSION_ID, "session-a")
            .putLong(AppleWebSignIn.KEY_CREATED_AT, now - 1_000L).commit();
        AppleWebSignIn.Pending linked = AppleWebSignIn.peekPending(store, now);
        check(linked != null && linked.link() && "session-a".equals(linked.sessionId()),
            "a link flow keeps the account session that started it");
        store.edit().remove(AppleWebSignIn.KEY_SESSION_ID).commit();
        check(AppleWebSignIn.peekPending(store, now) == null,
            "a link flow without an account session is discarded");

        store.edit().putString(AppleWebSignIn.KEY_VERIFIER, verifier).putString(AppleWebSignIn.KEY_PURPOSE, "link")
            .putLong(AppleWebSignIn.KEY_CREATED_AT, now - AppleWebSignIn.PENDING_MILLIS - 1L).commit();
        check(AppleWebSignIn.peekPending(store, now) == null, "an expired flow is not returned");
        check(!store.contains(AppleWebSignIn.KEY_VERIFIER), "an expired flow is cleared on peek");
        System.out.println("Android Apple web sign-in pending flow passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

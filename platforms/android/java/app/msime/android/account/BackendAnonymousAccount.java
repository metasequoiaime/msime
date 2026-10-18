package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import org.json.JSONObject;

/**
 * 本机匿名账号的令牌：身份、会话、注册和刷新都由 Rust 持有（{@link AccountStore}，#8201 迁移计划 5b）。
 *
 * <p>Rust 只给「登录的优先、否则匿名」的令牌（{@code msime_client_account_access_token}），没有只要匿名令牌的取法，所以这里回答的也是这一种：没有登录时就是匿名账号的令牌。只用匿名账号的语音贡献改由主进程带着存储调 Rust（{@link VoiceContributionApi}）。`:ime` 进程经 {@link AccountSessionProvider} 向主进程要。
 */
final class BackendAnonymousAccount {
    /**
     * The login endpoint is rate limiting; this is not a missing account.
     *
     * <p>Separate from the generic failure because the two want opposite handling: a rate limit
     * clears on its own and must be waited out, while everything else is worth reporting.
     */
    static final class RateLimited extends IllegalStateException {
        private static final long serialVersionUID = 1L;
        private final long retryAfterMillis;

        RateLimited(long retryAfterMillis) {
            super("anonymous login rate limited");
            this.retryAfterMillis = BoundsPolicy.nonNegative(retryAfterMillis);
        }

        long retryAfterMillis() { return retryAfterMillis; }
    }

    private static final Object LOCK = new Object();
    /**
     * 登录端点什么时候可以再问。
     *
     * <p>429 带着 `Retry-After`，遵守它不是客气：目录的每一页、每次搜索、每次切换标签都要令牌，没有这道闸时宿主一遇到限流就立刻再花一次注册去撞它，限流窗口永远清不掉。用静态字段是因为这道闸属于端点，不属于这个类的某个实例。
     */
    private static long nextAttemptAtMillis;
    private final Context application;

    BackendAnonymousAccount(Context context) {
        application = context.getApplicationContext();
    }

    String accessToken() throws Exception {
        return accessToken(null);
    }

    /** 登录的优先、否则匿名账号的令牌；`rejectedToken` 是服务端刚拒绝的令牌，给了就强制刷新。还没有匿名会话时先注册（受 Retry-After 约束）。 */
    String accessToken(String rejectedToken) throws Exception {
        if (!AccountSessionRoutingPolicy.ownsSession(
                Application.getProcessName(), application.getPackageName())) {
            return ownerToken(rejectedToken);
        }
        try {
            return token(AccountStore.accessToken(application, rejectedToken));
        } catch (AccountStore.Failure failure) {
            // 两种会话都没有（从没注册过，或刷新被拒、Rust 清掉了匿名会话）：注册匿名账号以后再取一次。
            if (!"account_unauthorized".equals(failure.code)) throw failure;
        }
        ensureRegistered();
        return token(AccountStore.accessToken(application, null));
    }

    private static String token(JSONObject answer) {
        String token = JsonPolicy.strictStringOrEmpty(answer.opt("access_token"));
        if (!AccountTokenPolicy.validToken(token)) throw new IllegalStateException("anonymous account unavailable");
        return token;
    }

    /** 从主进程取令牌，避免跨进程 SharedPreferences 缓存和 refresh rotation 竞态。 */
    private String ownerToken(String rejectedToken) throws Exception {
        Uri uri = Uri.parse("content://"
            + AccountSessionRoutingPolicy.authority(application.getPackageName()));
        Bundle extras = null;
        if (AccountTokenPolicy.validToken(rejectedToken)) {
            extras = new Bundle();
            extras.putString(AccountSessionRoutingPolicy.KEY_REJECTED_ACCESS_TOKEN, rejectedToken);
        }
        Bundle reply = application.getContentResolver().call(
            uri, AccountSessionRoutingPolicy.METHOD_SHARED_ACCESS_TOKEN, null, extras);
        if (reply == null) throw new IllegalStateException("anonymous account unavailable");
        return AccountSessionRoutingPolicy.sharedTokenFromReply(
            reply.getString(AccountSessionRoutingPolicy.KEY_STATE),
            reply.getString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN));
    }

    /**
     * Registers the device's anonymous account with the backend unless a session is already saved, as the app does on first launch (Rust 的 ensure_anonymous).
     *
     * <p>An expired saved session is enough: registration has happened, and the first request that needs a token refreshes it. 服务端限流时记下 Retry-After，到点之前不再去注册。只在主进程里做。
     */
    void ensureRegistered() throws Exception {
        if (!AccountSessionRoutingPolicy.ownsSession(Application.getProcessName(), application.getPackageName()))
            throw new IllegalStateException("account session owner");
        synchronized (LOCK) {
            long now = System.currentTimeMillis();
            if (now < nextAttemptAtMillis) throw new RateLimited(nextAttemptAtMillis - now);
            try {
                AccountStore.account(application, new JSONObject().put("operation", "ensure_anonymous"));
            } catch (AccountStore.Failure failure) {
                if (!"account_rate_limited".equals(failure.code)) throw failure;
                // 服务端给了重试时间就按它来；没给就退一分钟，别把这件事变成一个忙等的循环。
                long seconds = BoundsPolicy.bounded(failure.retryAfterSeconds == 0 ? 60 : failure.retryAfterSeconds,
                    1, 3600);
                nextAttemptAtMillis = System.currentTimeMillis() + seconds * 1000L;
                throw new RateLimited(seconds * 1000L);
            }
        }
    }
}

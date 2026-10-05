package app.msime.android;

import android.app.Activity;
import android.app.Instrumentation;
import android.os.Bundle;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import org.json.JSONObject;

/** Regression coverage for refresh-token rotation in the native Android account client. */
public final class BackendAccountRefreshDeviceSmoke extends Instrumentation {
    private static final String ACCESS = "a".repeat(64);
    private static final String REFRESH = "b".repeat(64);
    private static final String NEXT_ACCESS = "c".repeat(64);
    private static final String NEXT_REFRESH = "d".repeat(64);

    private static final class MemoryStore implements BackendAccount.SessionStore {
        private String value;

        MemoryStore(String value) { this.value = value; }

        @Override public synchronized String load() { return value; }
        @Override public synchronized void save(String next) { value = next; }
        @Override public synchronized void clear() { value = null; }
    }

    private static JSONObject tokens(String access, String refresh) throws Exception {
        return new JSONObject().put("access_token", access).put("refresh_token", refresh)
            .put("token_type", "Bearer").put("expires_in", 900);
    }

    private static String expiredSession() throws Exception {
        return new JSONObject().put("tokens", tokens(ACCESS, REFRESH))
            .put("expires_at_unix_ms", System.currentTimeMillis() - 1_000L).toString();
    }

    private static String activeSession() throws Exception {
        return new JSONObject().put("tokens", tokens(ACCESS, REFRESH))
            .put("expires_at_unix_ms", System.currentTimeMillis() + 900_000L).toString();
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    private static void refreshesExpiredSessionAndRotatesCredentials() throws Exception {
        MemoryStore store = new MemoryStore(expiredSession());
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            calls.incrementAndGet();
            check("POST".equals(method) && "/v1/auth/refresh".equals(path), "refresh endpoint");
            check(REFRESH.equals(body.optString("refresh_token")), "refresh token sent");
            return tokens(NEXT_ACCESS, NEXT_REFRESH);
        });

        check(NEXT_ACCESS.equals(account.accessToken()), "refreshed access token returned");
        JSONObject saved = new JSONObject(store.load());
        JSONObject savedTokens = saved.getJSONObject("tokens");
        check(NEXT_ACCESS.equals(savedTokens.optString("access_token")), "access token rotated");
        check(NEXT_REFRESH.equals(savedTokens.optString("refresh_token")), "refresh token rotated");
        check(saved.optLong("expires_at_unix_ms") > System.currentTimeMillis(), "expiry persisted");
        check(calls.get() == 1, "one refresh request");
    }

    private static void concurrentCallersShareOneRefresh() throws Exception {
        MemoryStore store = new MemoryStore(expiredSession());
        CountDownLatch started = new CountDownLatch(1);
        CountDownLatch release = new CountDownLatch(1);
        AtomicInteger calls = new AtomicInteger();
        BackendAccount.Requester requester = (method, path, body, token) -> {
            calls.incrementAndGet();
            started.countDown();
            check(release.await(5, TimeUnit.SECONDS), "refresh released");
            return tokens(NEXT_ACCESS, NEXT_REFRESH);
        };
        ExecutorService executor = Executors.newFixedThreadPool(2);
        try {
            Future<String> first = executor.submit(() -> new BackendAccount(store, requester).accessToken());
            check(started.await(5, TimeUnit.SECONDS), "refresh started");
            Future<String> second = executor.submit(() -> new BackendAccount(store, requester).accessToken());
            release.countDown();
            check(NEXT_ACCESS.equals(first.get(5, TimeUnit.SECONDS)), "first caller token");
            check(NEXT_ACCESS.equals(second.get(5, TimeUnit.SECONDS)), "second caller token");
            check(calls.get() == 1, "single-flight refresh");
        } finally {
            executor.shutdownNow();
        }
    }

    private static void refreshesAnUnexpiredRejectedToken() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            calls.incrementAndGet();
            check("POST".equals(method) && "/v1/auth/refresh".equals(path), "rejected token refresh endpoint");
            check(REFRESH.equals(body.optString("refresh_token")), "rejected token refresh credential");
            return tokens(NEXT_ACCESS, NEXT_REFRESH);
        });

        check(NEXT_ACCESS.equals(account.currentAccessToken(ACCESS)),
            "an unexpired rejected token must be refreshed");
        check(calls.get() == 1, "a rejected token triggers one refresh");
    }

    private static void retriesAccountRequestsAfter401() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            int call = calls.incrementAndGet();
            if ("/v1/auth/refresh".equals(path)) return tokens(NEXT_ACCESS, NEXT_REFRESH);
            check("/v1/models".equals(path), "account retry keeps the original endpoint");
            if (call == 1) throw new BackendAccount.RequestException(401);
            check(NEXT_ACCESS.equals(token), "account retry uses the refreshed access token");
            return new JSONObject().put("data", new org.json.JSONArray()
                .put(new JSONObject().put("id", "synthetic-model")));
        });

        check(account.chatModels().size() == 1, "an account 401 retries after refresh");
        check(calls.get() == 3, "account request, refresh and retry are each issued once");
    }

    private static void unauthorizedRefreshClearsSession() throws Exception {
        MemoryStore store = new MemoryStore(expiredSession());
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            throw new BackendAccount.RequestException(401);
        });
        check(account.accessToken().isEmpty(), "unauthorized refresh has no token");
        check(store.load() == null, "unauthorized refresh clears session");
    }

    private static void unboundedPersistedExpiryIsRejected() throws Exception {
        MemoryStore store = new MemoryStore(new JSONObject().put("tokens", tokens(ACCESS, REFRESH))
            .put("expires_at_unix_ms", Long.MAX_VALUE).toString());
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            calls.incrementAndGet();
            return tokens(NEXT_ACCESS, NEXT_REFRESH);
        });
        check(account.accessToken().isEmpty(), "unbounded persisted expiry rejected");
        check(calls.get() == 0, "unbounded expiry does not use token or refresh");
    }

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        try {
            refreshesExpiredSessionAndRotatesCredentials();
            concurrentCallersShareOneRefresh();
            refreshesAnUnexpiredRejectedToken();
            retriesAccountRequestsAfter401();
            unauthorizedRefreshClearsSession();
            unboundedPersistedExpiryIsRejected();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: account refresh rotation, single-flight and unauthorized clearing\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: account refresh ("
                + error.getClass().getSimpleName() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
}

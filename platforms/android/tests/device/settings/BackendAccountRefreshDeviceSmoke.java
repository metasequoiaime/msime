package app.msime.android;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.content.ContextWrapper;
import android.content.SharedPreferences;
import android.os.Bundle;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
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
        check(!saved.optString("session_id").isEmpty(), "legacy session receives a login identity");
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
                .put(new JSONObject().put("id", "synthetic-model")))
                .put("default_model", "synthetic-model");
        });

        check(account.chatModels().size() == 1, "an account 401 retries after refresh");
        check(calls.get() == 3, "account request, refresh and retry are each issued once");
        check(!new JSONObject(store.load()).optString("session_id").isEmpty(),
            "refresh retains a persisted login identity");
    }

    private static JSONObject models() throws Exception {
        return new JSONObject().put("data", new org.json.JSONArray()
            .put(new JSONObject().put("id", "synthetic-model")))
            .put("default_model", "synthetic-model");
    }

    private static void oldRequestDoesNotRetryAfterNewLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        String oldId = new BackendAccount(store, (method, path, body, token) -> models()).currentSession().sessionId();
        BackendAccount replacement = new BackendAccount(store,
            (method, path, body, token) -> tokens(NEXT_ACCESS, NEXT_REFRESH));
        AtomicInteger modelCalls = new AtomicInteger();
        BackendAccount old = new BackendAccount(store, (method, path, body, token) -> {
            if ("/v1/models".equals(path)) {
                if (modelCalls.incrementAndGet() == 1) {
                    replacement.login(new BackendAccount.Challenge("synthetic-challenge", "synthetic-nonce"),
                        "synthetic-credential");
                    throw new BackendAccount.RequestException(401);
                }
                return models();
            }
            throw new AssertionError("unexpected account request");
        });

        boolean cancelled = false;
        try {
            old.chatModels();
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "a rejected old request must not retry with a new login");
        check(modelCalls.get() == 1, "the new login must not receive the old request");
        check(!oldId.equals(new JSONObject(store.load()).optString("session_id")),
            "new login has a new identity");
    }

    private static void oldRequestDiscardsSuccessAfterNewLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        BackendAccount replacement = new BackendAccount(store,
            (method, path, body, token) -> tokens(NEXT_ACCESS, NEXT_REFRESH));
        BackendAccount old = new BackendAccount(store, (method, path, body, token) -> {
            check("/v1/models".equals(path), "models endpoint");
            replacement.login(new BackendAccount.Challenge("synthetic-challenge", "synthetic-nonce"),
                "synthetic-credential");
            return models();
        });

        boolean cancelled = false;
        try {
            old.chatModels();
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "an old response must not be delivered after a new login");
    }

    private static void secondaryProcessRejectsChangedOwnerLogin() throws Exception {
        AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
        BackendAccount.TokenSource owner = new BackendAccount.TokenSource() {
            @Override public String accessToken() { return ACCESS; }
            @Override public BackendAccount.SessionCredential session(String rejectedToken) {
                return new BackendAccount.SessionCredential(ACCESS, login.get());
            }
        };
        AtomicInteger calls = new AtomicInteger();
        BackendAccount secondary = new BackendAccount(new MemoryStore(null), (method, path, body, token) -> {
            calls.incrementAndGet();
            login.set("synthetic-login-b");
            throw new BackendAccount.RequestException(401);
        }, owner);

        boolean cancelled = false;
        try {
            secondary.chatModels();
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "a secondary process must reject an owner login replacement");
        check(calls.get() == 1, "a secondary process must not replay an old request");
    }

    private static void oldRequestDiscardsSuccessAfterSignOut() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            check("/v1/models".equals(path), "models endpoint");
            new BackendAccount(store, (m, p, b, t) -> models()).signOut();
            return models();
        });

        boolean cancelled = false;
        try {
            account.chatModels();
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "a signed-out request must not deliver its old response");
    }

    private static List<BackendAccount.ChatMessage> chatRequest() {
        return java.util.List.of(new BackendAccount.ChatMessage("user", "synthetic prompt"));
    }

    private static void oldChatStreamDoesNotRetryAfterNewLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        BackendAccount replacement = new BackendAccount(store,
            (method, path, body, token) -> tokens(NEXT_ACCESS, NEXT_REFRESH));
        AtomicInteger calls = new AtomicInteger();
        BackendAccount old = new BackendAccount(store, (method, path, body, token) -> models(), null,
            (body, token, call, listener) -> {
                calls.incrementAndGet();
                replacement.login(new BackendAccount.Challenge("synthetic-challenge", "synthetic-nonce"),
                    "synthetic-credential");
                throw new BackendAccount.RequestException(401);
            });
        boolean cancelled = false;
        try {
            old.chatStream(chatRequest(), "synthetic-model", new BackendAccount.ChatCall(), delta -> {});
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "an old chat stream must not retry after a new login");
        check(calls.get() == 1, "a new login receives no old chat stream retry");
    }

    private static void oldChatStreamDiscardsDeltaAfterNewLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        BackendAccount replacement = new BackendAccount(store,
            (method, path, body, token) -> tokens(NEXT_ACCESS, NEXT_REFRESH));
        AtomicInteger deltas = new AtomicInteger();
        BackendAccount old = new BackendAccount(store, (method, path, body, token) -> models(), null,
            (body, token, call, listener) -> {
                replacement.login(new BackendAccount.Challenge("synthetic-challenge", "synthetic-nonce"),
                    "synthetic-credential");
                listener.onDelta("old delta");
                return "old delta";
            });
        boolean cancelled = false;
        try {
            old.chatStream(chatRequest(), "synthetic-model", new BackendAccount.ChatCall(),
                delta -> deltas.incrementAndGet());
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "an old chat stream must stop after a new login");
        check(deltas.get() == 0, "an old chat delta must not reach the listener");
    }

    private static void oldChatStreamDoesNotFallbackAfterNewLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        BackendAccount replacement = new BackendAccount(store,
            (method, path, body, token) -> tokens(NEXT_ACCESS, NEXT_REFRESH));
        AtomicInteger fallbackCalls = new AtomicInteger();
        BackendAccount old = new BackendAccount(store, (method, path, body, token) -> {
            fallbackCalls.incrementAndGet();
            return models();
        }, null, (body, token, call, listener) -> {
            replacement.login(new BackendAccount.Challenge("synthetic-challenge", "synthetic-nonce"),
                "synthetic-credential");
            throw new BackendAccount.RequestException(400);
        });
        boolean cancelled = false;
        try {
            old.chatStream(chatRequest(), "synthetic-model", new BackendAccount.ChatCall(), delta -> {});
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "an old chat stream must not fallback under a new login");
        check(fallbackCalls.get() == 0, "a new login receives no old fallback request");
    }

    private static void chatStreamRetriesWithinTheSameLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            check("/v1/auth/refresh".equals(path), "chat stream refresh endpoint");
            return tokens(NEXT_ACCESS, NEXT_REFRESH);
        }, null, (body, token, call, listener) -> {
            calls.incrementAndGet();
            if (ACCESS.equals(token)) throw new BackendAccount.RequestException(401);
            check(NEXT_ACCESS.equals(token), "chat stream retry uses refreshed token");
            listener.onDelta("synthetic reply");
            return "synthetic reply";
        });
        AtomicInteger deltas = new AtomicInteger();
        String reply = account.chatStream(chatRequest(), "synthetic-model", new BackendAccount.ChatCall(),
            delta -> deltas.incrementAndGet());
        check("synthetic reply".equals(reply), "same login chat stream returns reply");
        check(calls.get() == 2 && deltas.get() == 1, "same login chat stream retries once and delivers delta");
    }

    private static void chatStreamFallsBackWithinTheSameLogin() throws Exception {
        MemoryStore store = new MemoryStore(activeSession());
        BackendAccount account = new BackendAccount(store, (method, path, body, token) -> {
            check(ACCESS.equals(token) && "/v1/chat/completions".equals(path), "fallback keeps request identity");
            check(!body.optBoolean("stream", true), "fallback disables streaming");
            return new JSONObject().put("choices", new org.json.JSONArray().put(new JSONObject()
                .put("message", new JSONObject().put("role", "assistant").put("content", "synthetic reply"))));
        }, null, (body, token, call, listener) -> {
            throw new BackendAccount.RequestException(400);
        });
        AtomicInteger deltas = new AtomicInteger();
        String reply = account.chatStream(chatRequest(), "synthetic-model", new BackendAccount.ChatCall(),
            delta -> deltas.incrementAndGet());
        check("synthetic reply".equals(reply) && deltas.get() == 1,
            "same login fallback delivers one complete reply");
    }

    private static CommunityCatalog.Item syntheticSkin() throws Exception {
        return new CommunityCatalog.Item("10000000-0000-4000-8000-000000000001",
            CommunityRequest.Kind.SKIN, "Synthetic", "", "Synthetic author", 0, 0, 0.0,
            new JSONObject(), CommunityRequest.Category.OTHER, true, 0, new JSONObject());
    }

    private static byte[] updatedSkin() {
        try {
            return new JSONObject().put("id", syntheticSkin().id()).put("name", "Synthetic")
                .put("description", "").put("author", "Synthetic author").put("design", new JSONObject())
                .put("saves", 0).put("rating_count", 0).put("rating_average", 0.0)
                .put("downloads", 0).put("category", "tech").put("owned", true)
                .toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
        } catch (Exception error) {
            throw new AssertionError(error);
        }
    }

    private void communityCategoryRejectsNewLogin() throws Exception {
        AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) {
                return "synthetic-login-a".equals(login.get()) ? ACCESS : NEXT_ACCESS;
            }
            @Override public String sessionId() { return login.get(); }
        };
        AtomicInteger rejectedCalls = new AtomicInteger();
        CloudApi rejected = new CloudApi((method, path, headers, body) -> {
            rejectedCalls.incrementAndGet();
            login.set("synthetic-login-b");
            return new CloudApi.Exchange(401, "application/json", null, new byte[0]);
        }, account, ignored -> "");
        CommunityCatalog.Update rejectedUpdate = new CommunityCatalog(getTargetContext(), rejected)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(rejectedCalls.get() == 1 && rejectedUpdate.failed()
                && rejectedUpdate.failure().contains("登录已切换"),
            "old category update must not retry under a new login");

        login.set("synthetic-login-a");
        AtomicInteger acceptedCalls = new AtomicInteger();
        CloudApi accepted = new CloudApi((method, path, headers, body) -> {
            acceptedCalls.incrementAndGet();
            login.set("synthetic-login-b");
            return new CloudApi.Exchange(200, "application/json", null, updatedSkin());
        }, account, ignored -> "");
        CommunityCatalog.Update oldSuccess = new CommunityCatalog(getTargetContext(), accepted)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(acceptedCalls.get() == 1 && oldSuccess.failed()
                && oldSuccess.failure().contains("登录已切换"),
            "old category response must not be accepted after a new login");
    }

    private void communityCategoryRetriesWithinTheSameLogin() throws Exception {
        AtomicReference<String> token = new AtomicReference<>(ACCESS);
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) {
                if (rejected != null) token.set(NEXT_ACCESS);
                return token.get();
            }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        AtomicInteger calls = new AtomicInteger();
        CloudApi cloud = new CloudApi((method, path, headers, body) -> {
            calls.incrementAndGet();
            boolean refreshed = ("Bearer " + NEXT_ACCESS).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(refreshed ? 200 : 401, "application/json", null,
                refreshed ? updatedSkin() : new byte[0]);
        }, account, ignored -> "");
        CommunityCatalog.Update update = new CommunityCatalog(getTargetContext(), cloud)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(!update.failed() && update.item().category() == CommunityRequest.Category.TECH,
            "same login category update accepts a refreshed token");
        check(calls.get() == 2, "same login category update retries once");

        CloudApi signedOut = new CloudApi((method, path, headers, body) -> {
            throw new AssertionError("signed-out category update must not send a request");
        }, rejected -> "", rejected -> "");
        CommunityCatalog.Update missing = new CommunityCatalog(getTargetContext(), signedOut)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(missing.failed() && missing.failure().contains("请先登录水杉账号"),
            "signed-out category update keeps its sign-in guidance");
    }

    /** Keep the old direct transport from contacting a real account during the red test. */
    private Context communityTestContext() {
        return new ContextWrapper(getTargetContext()) {
            @Override public Context getApplicationContext() { return this; }
            @Override public SharedPreferences getSharedPreferences(String name, int mode) {
                return (SharedPreferences) java.lang.reflect.Proxy.newProxyInstance(
                    getClass().getClassLoader(), new Class<?>[] {SharedPreferences.class},
                    (proxy, method, arguments) -> {
                        if ("getString".equals(method.getName())
                                && !name.contains("anonymous")) return null;
                        throw new IllegalStateException("synthetic account storage only");
                    });
            }
        };
    }

    private void communityWritesRejectNewLogin() throws Exception {
        for (int status : new int[] {401, 200}) {
            AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
            CloudApi.Tokens account = new CloudApi.Tokens() {
                @Override public String token(String rejected) {
                    return "synthetic-login-a".equals(login.get()) ? ACCESS : NEXT_ACCESS;
                }
                @Override public String sessionId() { return login.get(); }
            };
            AtomicInteger reportCalls = new AtomicInteger();
            CloudApi reportCloud = new CloudApi((method, path, headers, body) -> {
                reportCalls.incrementAndGet();
                check("POST".equals(method) && CommunityRequest.REPORT_PATH.equals(path),
                    "report endpoint");
                login.set("synthetic-login-b");
                return new CloudApi.Exchange(status, "application/json", null,
                    "{\"reported\":true}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
            }, account, ignored -> "");
            String failure = new CommunityCatalog(communityTestContext(), reportCloud)
                .report(syntheticSkin(), CommunityRequest.REPORT_REASONS.get(0), "");
            check(reportCalls.get() == 1 && failure.contains("登录已切换"),
                "old report must not retry or accept a response under a new login: " + status);

            login.set("synthetic-login-a");
            AtomicInteger downloadCalls = new AtomicInteger();
            String downloadPath = CommunityRequest.skinDownloadPath(syntheticSkin().id());
            CloudApi downloadCloud = new CloudApi((method, path, headers, body) -> {
                downloadCalls.incrementAndGet();
                check("POST".equals(method) && downloadPath.equals(path),
                    "download endpoint");
                login.set("synthetic-login-b");
                return new CloudApi.Exchange(status, "application/json", null, "{}".getBytes());
            }, account, ignored -> "");
            check(!new CommunityCatalog(communityTestContext(), downloadCloud)
                    .recordDownload(syntheticSkin()) && downloadCalls.get() == 1,
                "old download must not retry or accept a response under a new login: " + status);
        }
    }

    private void communityWritesRetryWithinTheSameLogin() throws Exception {
        AtomicReference<String> token = new AtomicReference<>(ACCESS);
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) {
                if (rejected != null) token.set(NEXT_ACCESS);
                return token.get();
            }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        AtomicInteger reportCalls = new AtomicInteger();
        CloudApi reportCloud = new CloudApi((method, path, headers, body) -> {
            reportCalls.incrementAndGet();
            boolean refreshed = ("Bearer " + NEXT_ACCESS).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(refreshed ? 201 : 401, "application/json", null,
                refreshed ? "{\"reported\":true}".getBytes() : new byte[0]);
        }, account, ignored -> "");
        check(new CommunityCatalog(communityTestContext(), reportCloud)
                .report(syntheticSkin(), CommunityRequest.REPORT_REASONS.get(0), "").isEmpty()
                && reportCalls.get() == 2,
            "same login report retries once with refreshed token");

        token.set(ACCESS);
        AtomicInteger downloadCalls = new AtomicInteger();
        CloudApi downloadCloud = new CloudApi((method, path, headers, body) -> {
            downloadCalls.incrementAndGet();
            boolean refreshed = ("Bearer " + NEXT_ACCESS).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(refreshed ? 200 : 401, "application/json", null,
                refreshed ? "{}".getBytes() : new byte[0]);
        }, account, ignored -> "");
        check(new CommunityCatalog(communityTestContext(), downloadCloud)
                .recordDownload(syntheticSkin()) && downloadCalls.get() == 2,
            "same login download retries once with refreshed token");
    }

    private void communityReportKeepsAnonymousRateLimit() throws Exception {
        CloudApi cloud = new CloudApi((method, path, headers, body) -> {
            throw new AssertionError("rate-limited identity must not send a report");
        }, ignored -> "", rejected -> {
            throw new BackendAnonymousAccount.RateLimited(60_000L);
        });
        String failure = new CommunityCatalog(communityTestContext(), cloud)
            .report(syntheticSkin(), CommunityRequest.REPORT_REASONS.get(0), "");
        check(failure.contains("操作较频繁"), "anonymous report retains rate-limit guidance");
    }

    private static byte[] syntheticListingPage() throws Exception {
        JSONObject root = new JSONObject().put("skins", new org.json.JSONArray()
            .put(new JSONObject(new String(updatedSkin(), java.nio.charset.StandardCharsets.UTF_8))))
            .put("has_more", false);
        return root.toString().getBytes(java.nio.charset.StandardCharsets.UTF_8);
    }

    private CommunityCatalog syntheticListingCatalog(CloudApi.Tokens account,
            CloudApi.Tokens anonymous, CloudApi.Transport listing) throws Exception {
        CloudApi cloud = new CloudApi((method, path, headers, body) -> {
            throw new AssertionError("listing must use its bounded transport");
        }, account, anonymous);
        return new CommunityCatalog(getTargetContext(), cloud, account, anonymous, listing);
    }

    private void communityListingUsesSyntheticTransport() throws Exception {
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) { return ACCESS; }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        CloudApi.Tokens anonymous = rejected -> "";
        byte[] pageBody = syntheticListingPage();
        AtomicInteger calls = new AtomicInteger();
        CloudApi.Transport listing = (method, path, headers, body) -> {
            calls.incrementAndGet();
            check("GET".equals(method) && path.contains("include=category")
                    && ("Bearer " + ACCESS).equals(headers.get("Authorization")),
                "listing uses the selected account and category path");
            return new CloudApi.Exchange(200, "application/json", null, pageBody);
        };
        CommunityCatalog catalog = syntheticListingCatalog(account, anonymous, listing);
        CommunityCatalog.Page page = catalog.list(CommunityRequest.Kind.SKIN, "", 0, null);
        check(!page.failed() && page.items().size() == 1 && calls.get() == 1,
            "synthetic catalogue page is returned without contacting the backend");
    }

    private void communityListingRejectsNewLogin() throws Exception {
        byte[] pageBody = syntheticListingPage();
        for (int status : new int[] {401, 200}) {
            AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
            CloudApi.Tokens account = new CloudApi.Tokens() {
                @Override public String token(String rejected) {
                    return "synthetic-login-a".equals(login.get()) ? ACCESS : NEXT_ACCESS;
                }
                @Override public String sessionId() { return login.get(); }
            };
            AtomicInteger calls = new AtomicInteger();
            CloudApi.Transport listing = (method, path, headers, body) -> {
                calls.incrementAndGet();
                login.set("synthetic-login-b");
                return new CloudApi.Exchange(status, "application/json", null,
                    status == 200 ? pageBody : new byte[0]);
            };
            CommunityCatalog.Page page = syntheticListingCatalog(account, ignored -> "", listing)
                .list(CommunityRequest.Kind.SKIN, "", 0, null);
            check(page.failed() && page.failure().contains("登录已切换") && calls.get() == 1,
                "old catalogue page must not retry or display after a new login: " + status);
        }
    }

    private void communityListingRetainsSameLoginAndPublicFallback() throws Exception {
        byte[] pageBody = syntheticListingPage();
        AtomicReference<String> token = new AtomicReference<>(ACCESS);
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) {
                if (rejected != null) token.set(NEXT_ACCESS);
                return token.get();
            }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        AtomicInteger calls = new AtomicInteger();
        CloudApi.Transport listing = (method, path, headers, body) -> {
            calls.incrementAndGet();
            boolean refreshed = ("Bearer " + NEXT_ACCESS).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(refreshed ? 200 : 401, "application/json", null,
                refreshed ? pageBody : new byte[0]);
        };
        CommunityCatalog.Page refreshed = syntheticListingCatalog(account, ignored -> "", listing)
            .list(CommunityRequest.Kind.SKIN, "", 0, null);
        check(!refreshed.failed() && refreshed.items().size() == 1 && calls.get() == 2,
            "same login listing retries once with refreshed token");

        CloudApi.Tokens unavailable = rejected -> {
            throw new IllegalStateException("synthetic identity unavailable");
        };
        CloudApi.Transport publicListing = (method, path, headers, body) -> {
            check(!headers.containsKey("Authorization"), "public listing omits bearer token");
            return new CloudApi.Exchange(200, "application/json", null, pageBody);
        };
        CommunityCatalog.Page publicPage = syntheticListingCatalog(unavailable, unavailable, publicListing)
            .list(CommunityRequest.Kind.SKIN, "", 0, null);
        check(!publicPage.failed() && publicPage.items().size() == 1,
            "catalogue remains readable without identity endpoints");
    }

    private void candidateTranslationUsesSyntheticTransport() throws Exception {
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) { return ACCESS; }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        AtomicInteger calls = new AtomicInteger();
        CloudApi cloud = new CloudApi((method, path, headers, body) -> {
            calls.incrementAndGet();
            check("POST".equals(method) && "/v1/translate".equals(path)
                    && ("Bearer " + ACCESS).equals(headers.get("Authorization"))
                    && body != null,
                "translation uses the selected account and endpoint");
            return new CloudApi.Exchange(200, "application/json", null,
                "{\"code\":200,\"data\":[\"synthetic-gloss\"]}".getBytes());
        }, account, ignored -> "");
        List<String> translated = new BackendTranslationClient(cloud).translate(List.of("合成词"), "EN");
        check(translated.equals(List.of("synthetic-gloss")) && calls.get() == 1,
            "synthetic translation returns without contacting the backend");
    }

    private void candidateTranslationRejectsNewLogin() throws Exception {
        for (int status : new int[] {401, 200}) {
            AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
            CloudApi.Tokens account = new CloudApi.Tokens() {
                @Override public String token(String rejected) {
                    return "synthetic-login-a".equals(login.get()) ? ACCESS : NEXT_ACCESS;
                }
                @Override public String sessionId() { return login.get(); }
            };
            AtomicInteger calls = new AtomicInteger();
            CloudApi cloud = new CloudApi((method, path, headers, body) -> {
                calls.incrementAndGet();
                login.set("synthetic-login-b");
                return new CloudApi.Exchange(status, "application/json", null,
                    status == 200 ? "{\"code\":200,\"data\":[\"old-gloss\"]}".getBytes()
                        : new byte[0]);
            }, account, ignored -> "");
            boolean changed = false;
            try {
                new BackendTranslationClient(cloud).translate(List.of("合成词"), "EN");
            } catch (CloudApi.Failure failure) {
                changed = "session_changed".equals(failure.code);
            }
            check(changed && calls.get() == 1,
                "old translation must not retry or publish after a new login: " + status);
        }
    }

    private void candidateTranslationRetainsSameLoginAndAnonymousFallback() throws Exception {
        AtomicReference<String> token = new AtomicReference<>(ACCESS);
        CloudApi.Tokens account = new CloudApi.Tokens() {
            @Override public String token(String rejected) {
                if (rejected != null) token.set(NEXT_ACCESS);
                return token.get();
            }
            @Override public String sessionId() { return "synthetic-login-a"; }
        };
        AtomicInteger calls = new AtomicInteger();
        CloudApi cloud = new CloudApi((method, path, headers, body) -> {
            calls.incrementAndGet();
            boolean refreshed = ("Bearer " + NEXT_ACCESS).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(refreshed ? 200 : 401, "application/json", null,
                refreshed ? "{\"code\":200,\"data\":[\"fresh-gloss\"]}".getBytes()
                    : new byte[0]);
        }, account, ignored -> "");
        check(new BackendTranslationClient(cloud).translate(List.of("合成词"), "EN")
                .equals(List.of("fresh-gloss")) && calls.get() == 2,
            "same login translation retries once with refreshed token");

        CloudApi anonymous = new CloudApi((method, path, headers, body) -> {
            check(("Bearer " + REFRESH).equals(headers.get("Authorization")),
                "anonymous translation retains device identity");
            return new CloudApi.Exchange(200, "application/json", null,
                "{\"code\":200,\"data\":[\"anonymous-gloss\"]}".getBytes());
        }, ignored -> "", ignored -> REFRESH);
        check(new BackendTranslationClient(anonymous).translate(List.of("合成词"), "EN")
                .equals(List.of("anonymous-gloss")),
            "signed-out translation uses the anonymous identity");
    }

    private void candidateTranslationKeepsResponseBound() throws Exception {
        CloudApi cloud = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(200, "application/json", null, new byte[256 * 1024 + 1]),
            new CloudApi.Tokens() {
                @Override public String token(String rejected) { return ACCESS; }
                @Override public String sessionId() { return "synthetic-login-a"; }
            }, ignored -> "");
        boolean tooLarge = false;
        try {
            new BackendTranslationClient(cloud).translate(List.of("合成词"), "EN");
        } catch (IllegalStateException bounded) {
            tooLarge = bounded.getMessage().contains("too large");
        }
        check(tooLarge, "translation retains its 256 KiB response bound");
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
        String stage = "refresh";
        try {
            refreshesExpiredSessionAndRotatesCredentials();
            concurrentCallersShareOneRefresh();
            refreshesAnUnexpiredRejectedToken();
            retriesAccountRequestsAfter401();
            oldRequestDoesNotRetryAfterNewLogin();
            oldRequestDiscardsSuccessAfterNewLogin();
            secondaryProcessRejectsChangedOwnerLogin();
            oldRequestDiscardsSuccessAfterSignOut();
            stage = "old stream retry";
            oldChatStreamDoesNotRetryAfterNewLogin();
            stage = "old stream delta";
            oldChatStreamDiscardsDeltaAfterNewLogin();
            stage = "old stream fallback";
            oldChatStreamDoesNotFallbackAfterNewLogin();
            stage = "same login stream refresh";
            chatStreamRetriesWithinTheSameLogin();
            stage = "same login stream fallback";
            chatStreamFallsBackWithinTheSameLogin();
            stage = "community category old login";
            communityCategoryRejectsNewLogin();
            stage = "community category same login";
            communityCategoryRetriesWithinTheSameLogin();
            stage = "community writes old login";
            communityWritesRejectNewLogin();
            stage = "community writes same login";
            communityWritesRetryWithinTheSameLogin();
            stage = "community report rate limit";
            communityReportKeepsAnonymousRateLimit();
            stage = "community listing synthetic transport";
            communityListingUsesSyntheticTransport();
            stage = "community listing old login";
            communityListingRejectsNewLogin();
            stage = "community listing same login and public";
            communityListingRetainsSameLoginAndPublicFallback();
            stage = "candidate translation synthetic transport";
            candidateTranslationUsesSyntheticTransport();
            stage = "candidate translation old login";
            candidateTranslationRejectsNewLogin();
            stage = "candidate translation same login and anonymous";
            candidateTranslationRetainsSameLoginAndAnonymousFallback();
            stage = "candidate translation response bound";
            candidateTranslationKeepsResponseBound();
            unauthorizedRefreshClearsSession();
            unboundedPersistedExpiryIsRejected();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: account refresh, login lineage, chat stream, community and translation\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: account refresh " + stage + " ("
                + error.getClass().getSimpleName() + ": " + error.getMessage() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
}

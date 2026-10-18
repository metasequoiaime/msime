package app.msime.android;

import android.app.Activity;
import android.app.Instrumentation;
import android.content.Context;
import android.os.Bundle;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;
import org.json.JSONObject;

/**
 * 原生 Android 账号客户端里的请求（键盘的云剪贴板、社区、释义）怎样绑定到发起它们的登录。
 *
 * <p>令牌的刷新、单飞和吊销都由 Rust 持有（#8201 迁移计划 5b），由 client-services 的测试覆盖；这里用一个假的 {@link BackendAccount.Sessions} 代替 Rust，核对 Java 一侧：换成别的账号或退出以后，旧请求既不重试也不交出结果；同一个账号里 401 后带着被拒的令牌换一枚再试一次；同一个账号重新登录（令牌变了、账号编号没变）不算换号。社区从 5c 起整个经 Rust 的 `msime_client_community`，这里换成假的 {@link CommunityCatalog.Calls}，核对请求的形状、哪些操作允许退回匿名账号，以及请求前后账号编号变了就丢掉结果。
 *
 * <p>释义的用例经 JNI 调真的 Rust（{@link NativeClient#accountGloss}），而测试包里没有 libmsime_android.so，所以这一套以产品包 app.msime.android 为目标、跑在应用进程里：测试包的类排在应用的类前面，原生库按应用的库路径加载，与 CloudClipboardDriverDeviceSmoke 相同。check-host.sh 拦着以测试包自己为目标、却能引用到 NativeClient 的 instrumentation。
 */
public final class BackendAccountSessionDeviceSmoke extends Instrumentation {
    private static final String ACCESS = "a".repeat(64);
    private static final String REFRESH = "b".repeat(64);
    private static final String NEXT_ACCESS = "c".repeat(64);

    /** 代替 Rust 的会话：一个账号编号和它当前的令牌。刚被拒绝的令牌换成 NEXT_ACCESS，记下换了几次。 */
    private static final class FakeSessions implements BackendAccount.Sessions {
        private String user;
        private String token;
        final AtomicInteger refreshes = new AtomicInteger();

        FakeSessions(String user, String token) {
            this.user = user;
            this.token = token;
        }

        @Override public synchronized BackendAccount.SessionCredential session(String rejectedToken) {
            if (user.isEmpty()) return new BackendAccount.SessionCredential("", "");
            if (rejectedToken != null && rejectedToken.equals(token)) {
                token = NEXT_ACCESS;
                refreshes.incrementAndGet();
            }
            return new BackendAccount.SessionCredential(token, user);
        }

        @Override public synchronized String userId() { return user; }

        /** 另一个账号登录了，或同一个账号重新登录（只换了令牌）。 */
        synchronized void signIn(String nextUser, String nextToken) {
            user = nextUser;
            token = nextToken;
        }

        synchronized void signOut() {
            user = "";
            token = "";
        }
    }

    private static void check(boolean value, String message) {
        if (!value) throw new AssertionError(message);
    }

    /**
     * 会话相关的用例借键盘的云剪贴板上传（{@link BackendAccount#addClipboard}）做一次带登录令牌的请求：脚本化的驱动只要一次 `POST /v1/users/me/clipboard`，收到 200 就交出那一条。真的驱动在 Rust 里（{@link CloudClipboardNativeDriver}），这里只核对 Java 一侧怎样把请求绑定到发起它的登录。云剪贴板的开关和删除从 5c 起经 Rust，对话从 5c 起经 Rust 的流式对话，这两处原来的 Java 请求在 5f 删掉。
     */
    private static final String CLIPBOARD = "/v1/users/me/clipboard";
    private static final String NOTE = "synthetic note";

    private static CloudClipboardE2e.Local oneRequestDriver() {
        CloudClipboardE2e.Driver driver = new CloudClipboardE2e.Driver() {
            @Override public CloudClipboardE2e.Step start(CloudClipboardE2e.Operation operation, String keystore,
                    String deviceName) {
                check("add".equals(operation.op()) && NOTE.equals(operation.text()), "the upload reaches the driver");
                return CloudClipboardE2e.Step.request("POST", CLIPBOARD, "{}", "{}", null);
            }

            @Override public CloudClipboardE2e.Step resume(String state, int status, String body) throws Exception {
                if (status != 200) return CloudClipboardE2e.Step.error(status, "", null);
                return CloudClipboardE2e.Step.done(new JSONObject().put("id", "d".repeat(64)).put("text", NOTE)
                    .put("updated_at", "2026-10-18T00:00:00Z").toString(), null);
            }
        };
        CloudClipboardE2e.Keystore keystore = new CloudClipboardE2e.Keystore() {
            @Override public String load() { return null; }
            @Override public void save(String value, CloudClipboardE2e.Guard guard) {
                throw new AssertionError("this driver never hands over a keystore");
            }
            @Override public void delete() { throw new AssertionError("nothing deletes the keystore here"); }
        };
        return new CloudClipboardE2e.Local(driver, keystore, "Synthetic Phone");
    }

    private static CloudClipboardE2e.Reply ok() {
        return new CloudClipboardE2e.Reply(200, "{}");
    }

    private static CloudClipboardE2e.Reply rejected() {
        return new CloudClipboardE2e.Reply(401, "");
    }

    private static void retriesAccountRequestsAfter401() throws Exception {
        FakeSessions sessions = new FakeSessions("user-a", ACCESS);
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(sessions, (method, path, body, token) -> {
            int call = calls.incrementAndGet();
            check(CLIPBOARD.equals(path), "account retry keeps the original endpoint");
            if (call == 1) return rejected();
            check(NEXT_ACCESS.equals(token), "account retry uses the refreshed access token");
            return ok();
        });

        BackendAccount.ClipboardItem item = account.addClipboard(NOTE, "user-a", oneRequestDriver());
        check(NOTE.equals(item.text()), "the retried upload hands over its item");
        check(calls.get() == 2 && sessions.refreshes.get() == 1, "account request and its retry are each issued once");
    }

    private static void oldRequestDoesNotRetryAfterNewLogin() throws Exception {
        FakeSessions sessions = new FakeSessions("user-a", ACCESS);
        AtomicInteger calls = new AtomicInteger();
        BackendAccount old = new BackendAccount(sessions, (method, path, body, token) -> {
            calls.incrementAndGet();
            sessions.signIn("user-b", NEXT_ACCESS);
            return rejected();
        });

        boolean cancelled = false;
        try {
            old.addClipboard(NOTE, "user-a", oneRequestDriver());
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "a rejected old request must not retry with a new login");
        check(calls.get() == 1, "the new login must not receive the old request");
    }

    private static void oldRequestDiscardsSuccessAfterNewLogin() throws Exception {
        FakeSessions sessions = new FakeSessions("user-a", ACCESS);
        BackendAccount old = new BackendAccount(sessions, (method, path, body, token) -> {
            check(CLIPBOARD.equals(path), "clipboard endpoint");
            sessions.signIn("user-b", NEXT_ACCESS);
            return ok();
        });

        boolean cancelled = false;
        try {
            old.addClipboard(NOTE, "user-a", oneRequestDriver());
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "an old response must not be delivered after a new login");
    }

    /** 同一个账号重新登录只换了令牌，账号编号没变：请求照常交出结果，与 Rust 按 user.id 绑定相同。 */
    private static void sameAccountReLoginKeepsTheRequest() throws Exception {
        FakeSessions sessions = new FakeSessions("user-a", ACCESS);
        AtomicInteger calls = new AtomicInteger();
        BackendAccount account = new BackendAccount(sessions, (method, path, body, token) -> {
            calls.incrementAndGet();
            check(CLIPBOARD.equals(path), "clipboard endpoint");
            sessions.signIn("user-a", NEXT_ACCESS);
            return ok();
        });
        account.addClipboard(NOTE, "user-a", oneRequestDriver());
        check(calls.get() == 1, "signing in to the same account again is not an account switch");
    }

    private static void secondaryProcessRejectsChangedOwnerLogin() throws Exception {
        FakeSessions owner = new FakeSessions("user-a", ACCESS);
        AtomicInteger calls = new AtomicInteger();
        BackendAccount secondary = new BackendAccount(owner, (method, path, body, token) -> {
            calls.incrementAndGet();
            owner.signIn("user-b", NEXT_ACCESS);
            return rejected();
        }, false);

        boolean cancelled = false;
        try {
            secondary.addClipboard(NOTE, "user-a", oneRequestDriver());
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "a secondary process must reject an owner login replacement");
        check(calls.get() == 1, "a secondary process must not replay an old request");
    }

    private static void oldRequestDiscardsSuccessAfterSignOut() throws Exception {
        FakeSessions sessions = new FakeSessions("user-a", ACCESS);
        BackendAccount account = new BackendAccount(sessions, (method, path, body, token) -> {
            check(CLIPBOARD.equals(path), "clipboard endpoint");
            sessions.signOut();
            return ok();
        });

        boolean cancelled = false;
        try {
            account.addClipboard(NOTE, "user-a", oneRequestDriver());
        } catch (java.util.concurrent.CancellationException expected) {
            cancelled = true;
        }
        check(cancelled, "a signed-out request must not deliver its old response");
    }

    private static CommunityCatalog.Item syntheticSkin() throws Exception {
        return new CommunityCatalog.Item("10000000-0000-4000-8000-000000000001",
            CommunityRequest.Kind.SKIN, "Synthetic", "", "Synthetic author", 0, 0, 0.0,
            new JSONObject(), CommunityRequest.Category.OTHER, true, 0, new JSONObject());
    }

    private static JSONObject updatedSkin() throws Exception {
        return new JSONObject().put("id", syntheticSkin().id()).put("name", "Synthetic")
            .put("description", "").put("author", "Synthetic author").put("design", new JSONObject())
            .put("downloads", 0).put("rating_count", 0).put("rating_average", 0.0)
            .put("category", "tech").put("owned", true).put("my_rating", 0);
    }

    private static JSONObject syntheticListingPage() throws Exception {
        return new JSONObject().put("skins", new org.json.JSONArray().put(updatedSkin())).put("has_more", false);
    }

    /**
     * 代替 Rust 的社区入口（#8201 迁移计划 5c）：记下每次的操作和是否允许退回匿名账号，按给定的回答或失败码作答，`afterCall` 可以在作答时换掉登录的账号。令牌、刷新和 401 后的重试都在 Rust 里，由 client-services 的测试覆盖；这里只核对 Java 一侧的请求形状和按账号编号的绑定。
     */
    private static final class FakeCommunity implements CommunityCatalog.Calls {
        final AtomicReference<String> login = new AtomicReference<>("synthetic-login-a");
        final List<JSONObject> actions = new java.util.ArrayList<>();
        final List<Boolean> fallbacks = new java.util.ArrayList<>();
        Object answer = JSONObject.NULL;
        String failure = "";
        String afterCall = null;

        @Override public synchronized Object community(JSONObject action, boolean anonymousFallback)
                throws AccountStore.Failure {
            actions.add(action);
            fallbacks.add(anonymousFallback);
            if (afterCall != null) login.set(afterCall);
            if (!failure.isEmpty()) throw new AccountStore.Failure(failure, 0);
            return answer;
        }

        @Override public String userId() { return login.get(); }
    }

    private void communityCategoryRejectsNewLogin() throws Exception {
        FakeCommunity rust = new FakeCommunity();
        rust.answer = updatedSkin();
        rust.afterCall = "synthetic-login-b";
        CommunityCatalog.Update update = new CommunityCatalog(rust)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(rust.actions.size() == 1 && update.failed() && update.failure().contains("登录已切换"),
            "an old category response must not be accepted after a new login");
        check("keyboard_skin_set_category".equals(rust.actions.get(0).getString("operation"))
                && "tech".equals(rust.actions.get(0).getString("category")) && !rust.fallbacks.get(0),
            "the category edit is the owner's own operation, without an anonymous fallback");
    }

    private void communityCategoryWithinTheSameLogin() throws Exception {
        FakeCommunity rust = new FakeCommunity();
        rust.answer = updatedSkin();
        CommunityCatalog.Update update = new CommunityCatalog(rust)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(!update.failed() && update.item().category() == CommunityRequest.Category.TECH,
            "same login category update accepts Rust's updated item");

        FakeCommunity signedOut = new FakeCommunity();
        signedOut.failure = "community_unauthorized";
        CommunityCatalog.Update missing = new CommunityCatalog(signedOut)
            .setCategory(syntheticSkin(), CommunityRequest.Category.TECH);
        check(missing.failed() && missing.failure().contains("请先登录水杉账号"),
            "signed-out category update keeps its sign-in guidance");
    }

    private void communityWritesRejectNewLogin() throws Exception {
        FakeCommunity report = new FakeCommunity();
        report.afterCall = "synthetic-login-b";
        String failure = new CommunityCatalog(report).report(syntheticSkin(), CommunityRequest.REPORT_REASONS.get(0), "");
        check(report.actions.size() == 1 && failure.contains("登录已切换"),
            "an old report must not be accepted under a new login");

        FakeCommunity download = new FakeCommunity();
        download.answer = new JSONObject().put("background", 1);
        download.afterCall = "synthetic-login-b";
        CommunityCatalog.Download got = new CommunityCatalog(download).download(syntheticSkin());
        check(download.actions.size() == 1 && got.failed() && got.failure().contains("登录已切换"),
            "an old download must not be saved under a new login");
    }

    private void communityWritesWithinTheSameLogin() throws Exception {
        FakeCommunity report = new FakeCommunity();
        check(new CommunityCatalog(report).report(syntheticSkin(), CommunityRequest.REPORT_REASONS.get(0), "合成说明").isEmpty(),
            "same login report is taken");
        JSONObject sent = report.actions.get(0);
        check("report".equals(sent.getString("operation")) && "skins".equals(sent.getString("kind"))
                && syntheticSkin().id().equals(sent.getString("id")) && "合成说明".equals(sent.getString("detail"))
                && report.fallbacks.get(0),
            "a report names the item the way Rust expects and may use the anonymous account");

        FakeCommunity download = new FakeCommunity();
        JSONObject design = new JSONObject().put("background", 1);
        download.answer = design;
        CommunityCatalog.Download got = new CommunityCatalog(download).download(syntheticSkin());
        check(!got.failed() && got.design() == design && download.fallbacks.get(0)
                && "keyboard_skin_download".equals(download.actions.get(0).getString("operation")),
            "getting a skin downloads Rust's design and may use the anonymous account");

        FakeCommunity offline = new FakeCommunity();
        offline.failure = "community_unavailable";
        check(new CommunityCatalog(offline).download(syntheticSkin()).failed(),
            "getting a skin fails without a download, so nothing is saved offline");
    }

    private void communityReportKeepsAnonymousRateLimit() throws Exception {
        FakeCommunity rust = new FakeCommunity();
        rust.failure = "account_rate_limited";
        String failure = new CommunityCatalog(rust).report(syntheticSkin(), CommunityRequest.REPORT_REASONS.get(0), "");
        check(failure.contains("操作较频繁"), "a rate-limited anonymous registration keeps the rate-limit guidance");
    }

    private void communityListingAsksRust() throws Exception {
        FakeCommunity rust = new FakeCommunity();
        rust.answer = syntheticListingPage();
        CommunityCatalog.Page page = new CommunityCatalog(rust).list(CommunityRequest.Kind.SKIN, " 海 ", 20,
            CommunityRequest.Category.GUOFENG);
        check(!page.failed() && page.items().size() == 1, "a skin page from Rust is parsed");
        JSONObject sent = rust.actions.get(0);
        check("keyboard_skin_list".equals(sent.getString("operation")) && sent.getInt("offset") == 20
                && "海".equals(sent.getString("search")) && "guofeng".equals(sent.getString("category"))
                && !rust.fallbacks.get(0),
            "the skin listing carries offset, trimmed search and category");

        FakeCommunity resources = new FakeCommunity();
        resources.answer = new JSONObject().put("items", new org.json.JSONArray()).put("has_more", false);
        CommunityCatalog.Page empty = new CommunityCatalog(resources).list(CommunityRequest.Kind.REPLY, "", -5, null);
        JSONObject listed = resources.actions.get(0);
        check(!empty.failed() && "resource_list".equals(listed.getString("operation"))
                && "reply".equals(listed.getString("kind")) && "".equals(listed.getString("scope"))
                && listed.getInt("offset") == 0 && !listed.has("category"),
            "resources go to resource_list by kind, without categories");
    }

    private void communityListingRejectsNewLogin() throws Exception {
        FakeCommunity rust = new FakeCommunity();
        rust.answer = syntheticListingPage();
        rust.afterCall = "synthetic-login-b";
        CommunityCatalog.Page page = new CommunityCatalog(rust).list(CommunityRequest.Kind.SKIN, "", 0, null);
        check(page.failed() && page.failure().contains("登录已切换"),
            "an old catalogue page must not display after a new login");
        FakeCommunity failing = new FakeCommunity();
        failing.failure = "community_unavailable";
        CommunityCatalog.Page down = new CommunityCatalog(failing).list(CommunityRequest.Kind.SKIN, "", 0, null);
        check(down.failed() && down.failure().contains("连不上社区"), "an unavailable community says so");
    }

    // 账号释义（#8201 迁移计划 5e）：请求和解读走真的 Rust（msime_client_account_gloss），HTTP 和令牌是内存实现。
    private void candidateTranslationUsesSyntheticTransport() throws Exception {
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<JSONObject> sent = new AtomicReference<>();
        BackendTranslationClient client = new BackendTranslationClient((method, path, headers, body) -> {
            calls.incrementAndGet();
            check("POST".equals(method) && "/v1/translate".equals(path)
                    && ("Bearer " + ACCESS).equals(headers.get("Authorization")) && body != null,
                "translation uses Rust's endpoint and the shared token");
            try {
                sent.set(new JSONObject(new String(body, java.nio.charset.StandardCharsets.UTF_8)));
            } catch (org.json.JSONException malformed) {
                throw new java.io.IOException(malformed);
            }
            return new CloudApi.Exchange(200, "application/json", null,
                "{\"code\":200,\"data\":[\"synthetic-gloss\"]}".getBytes());
        }, rejected -> ACCESS);
        List<String> translated = client.translate(List.of("合成词"), "en");
        JSONObject body = sent.get();
        check(translated.equals(List.of("synthetic-gloss")) && calls.get() == 1,
            "synthetic translation returns without contacting the backend");
        check(body != null && "合成词".equals(body.getJSONArray("texts").getString(0))
                && "ZH".equals(body.getString("source_lang")) && "EN".equals(body.getString("target_lang")),
            "Rust builds the request body: " + body);
    }

    private void candidateTranslationRetriesWithAFreshToken() throws Exception {
        AtomicInteger calls = new AtomicInteger();
        AtomicReference<String> rejectedToken = new AtomicReference<>();
        BackendTranslationClient client = new BackendTranslationClient((method, path, headers, body) -> {
            calls.incrementAndGet();
            boolean refreshed = ("Bearer " + NEXT_ACCESS).equals(headers.get("Authorization"));
            return new CloudApi.Exchange(refreshed ? 200 : 401, "application/json", null,
                refreshed ? "{\"code\":200,\"data\":[\"fresh-gloss\"]}".getBytes() : new byte[0]);
        }, rejected -> {
            if (rejected == null) return ACCESS;
            rejectedToken.set(rejected);
            return NEXT_ACCESS;
        });
        check(client.translate(List.of("合成词"), "EN").equals(List.of("fresh-gloss")) && calls.get() == 2
                && ACCESS.equals(rejectedToken.get()),
            "a rejected token is exchanged once and the request retried");

        AtomicInteger refusals = new AtomicInteger();
        BackendTranslationClient stuck = new BackendTranslationClient((method, path, headers, body) -> {
            refusals.incrementAndGet();
            return new CloudApi.Exchange(401, "application/json", null, new byte[0]);
        }, rejected -> ACCESS);
        String code = "";
        try {
            stuck.translate(List.of("合成词"), "EN");
        } catch (AccountStore.Failure failure) {
            code = failure.code;
        }
        check("account_unauthorized".equals(code) && refusals.get() == 1,
            "the same token is not sent twice and Rust reads the 401: " + code);
    }

    private void candidateTranslationKeepsGoodItems() throws Exception {
        String oversized = "x".repeat(4097);
        BackendTranslationClient client = new BackendTranslationClient((method, path, headers, body) ->
            new CloudApi.Exchange(200, "application/json", null,
                ("{\"code\":200,\"data\":[\"" + oversized + "\",\" good\\n\",\"合成词\"]}").getBytes()),
            rejected -> ACCESS);
        List<String> translated = client.translate(List.of("长词", "好词", "合成词"), "EN");
        check(translated.equals(java.util.Arrays.asList(null, "good", null)),
            "Rust nulls an oversized or unchanged item and keeps the rest: " + translated);

        BackendTranslationClient limited = new BackendTranslationClient((method, path, headers, body) ->
            new CloudApi.Exchange(429, "application/json", "60",
                "{\"error\":{\"code\":\"rate_limit_exceeded\"}}".getBytes()),
            rejected -> ACCESS);
        AccountStore.Failure refused = null;
        try {
            limited.translate(List.of("合成词"), "EN");
        } catch (AccountStore.Failure failure) {
            refused = failure;
        }
        check(refused != null && "account_rate_limited".equals(refused.code) && refused.retryAfterSeconds == 60,
            "Rust maps a rate limit with its Retry-After");
    }

    private void candidateTranslationKeepsResponseBound() throws Exception {
        BackendTranslationClient client = new BackendTranslationClient((method, path, headers, body) ->
            new CloudApi.Exchange(200, "application/json", null, new byte[256 * 1024 + 1]),
            rejected -> ACCESS);
        boolean tooLarge = false;
        try {
            client.translate(List.of("合成词"), "EN");
        } catch (IllegalStateException bounded) {
            tooLarge = bounded.getMessage().contains("too large");
        }
        check(tooLarge, "translation retains its 256 KiB response bound");
    }

    private static void malformedQqLinksAreRejected() throws Exception {
        CloudApi cloud = new CloudApi((method, path, headers, body) ->
            new CloudApi.Exchange(200, "application/json", null, new byte[] {'{', '}'}),
            ignored -> ACCESS, ignored -> "");
        boolean rejected = false;
        try {
            new TypingUploadApi(cloud).links();
        } catch (CloudApi.Failure failure) {
            rejected = failure.status == 200 && "invalid_response".equals(failure.code);
        }
        check(rejected, "malformed QQ link list must not be treated as empty");
    }

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        String stage = "same login retry";
        try {
            retriesAccountRequestsAfter401();
            stage = "old login";
            oldRequestDoesNotRetryAfterNewLogin();
            oldRequestDiscardsSuccessAfterNewLogin();
            stage = "same account sign-in again";
            sameAccountReLoginKeepsTheRequest();
            stage = "secondary process";
            secondaryProcessRejectsChangedOwnerLogin();
            oldRequestDiscardsSuccessAfterSignOut();
            stage = "community category old login";
            communityCategoryRejectsNewLogin();
            stage = "community category same login";
            communityCategoryWithinTheSameLogin();
            stage = "community writes old login";
            communityWritesRejectNewLogin();
            stage = "community writes same login";
            communityWritesWithinTheSameLogin();
            stage = "community report rate limit";
            communityReportKeepsAnonymousRateLimit();
            stage = "community listing through Rust";
            communityListingAsksRust();
            stage = "community listing old login";
            communityListingRejectsNewLogin();
            stage = "candidate translation synthetic transport";
            candidateTranslationUsesSyntheticTransport();
            stage = "candidate translation fresh token";
            candidateTranslationRetriesWithAFreshToken();
            stage = "candidate translation items through Rust";
            candidateTranslationKeepsGoodItems();
            stage = "candidate translation response bound";
            candidateTranslationKeepsResponseBound();
            stage = "malformed QQ links";
            malformedQqLinksAreRejected();
            result.putString("stream", "MSIME_DEVICE_SMOKE_PASSED: account login lineage, community and translation\n");
            finish(Activity.RESULT_OK, result);
        } catch (Exception | AssertionError | LinkageError error) {
            result.putString("stream", "MSIME_DEVICE_SMOKE_FAILED: account session " + stage + " ("
                + error.getClass().getSimpleName() + ": " + error.getMessage() + ")\n");
            finish(Activity.RESULT_CANCELED, result);
        }
    }
}

package app.msime.android;

import java.util.concurrent.atomic.AtomicInteger;

public final class AccountSessionRoutingSmoke {
    private static final String PACKAGE = "app.msime.android";
    private static final String TOKEN = "e".repeat(64);

    public static void main(String[] args) throws Exception {
        check(AccountSessionRoutingPolicy.ownsSession(PACKAGE, PACKAGE), "the main process owns the session");
        check(!AccountSessionRoutingPolicy.ownsSession(PACKAGE + ":ime", PACKAGE), "the keyboard process does not own the session");
        check(!AccountSessionRoutingPolicy.ownsSession(null, PACKAGE), "an unknown process asks the owner");
        check(!AccountSessionRoutingPolicy.ownsSession(PACKAGE, ""), "a missing package name asks the owner");
        check(!AccountSessionRoutingPolicy.ownsSession(PACKAGE, null), "a null package name asks the owner");
        check(AccountSessionRoutingPolicy.authority(PACKAGE).equals(PACKAGE + ".account-session"), "the authority matches the manifests");
        boolean rejected = false;
        try {
            AccountSessionRoutingPolicy.authority("");
        } catch (IllegalArgumentException expected) {
            rejected = true;
        }
        check(rejected, "an empty package name has no authority");

        check(AccountSessionRoutingPolicy.accepts("access_token", 10123, 10123), "our own uid gets the token");
        check(!AccountSessionRoutingPolicy.accepts("access_token", 10124, 10123), "another uid is refused");
        check(!AccountSessionRoutingPolicy.accepts("refresh_token", 10123, 10123), "no other method is answered");
        check(AccountSessionRoutingPolicy.accepts("shared_access_token", 10123, 10123),
            "our own uid gets the shared token");
        check(!AccountSessionRoutingPolicy.accepts("shared_access_token", 10124, 10123),
            "another uid is refused the shared token");
        check(AccountSessionRoutingPolicy.accepts("voice_contribution", 10123, 10123),
            "our own keyboard may hand a voice contribution to the main process");
        check(!AccountSessionRoutingPolicy.accepts("voice_contribution", 10124, 10123),
            "another uid cannot upload through the main process");
        check(!AccountSessionRoutingPolicy.accepts("anonymous_access_token", 10123, 10123),
            "there is no anonymous-only token any more: Rust has no such source");
        check(!AccountSessionRoutingPolicy.accepts(null, 10123, 10123), "a missing method is refused");

        check(AccountSessionRoutingPolicy.source(true, true) == AccountSessionRoutingPolicy.Source.OWN, "a native sign-in session wins");
        check(AccountSessionRoutingPolicy.source(true, false) == AccountSessionRoutingPolicy.Source.OWN, "a native sign-in session answers alone");
        check(AccountSessionRoutingPolicy.source(false, true) == AccountSessionRoutingPolicy.Source.LEGACY_READ_ONLY, "the combined package's Rust session is read only");
        check(AccountSessionRoutingPolicy.source(false, false) == AccountSessionRoutingPolicy.Source.NONE, "no session means signed out");
        long now = 1_700_000_000_000L;
        check(AccountSessionRoutingPolicy.legacyToken(TOKEN, now + 60_000L, now).equals(TOKEN), "an unexpired Rust token is used as it is");
        check(AccountSessionRoutingPolicy.legacyToken(TOKEN, now + 30_000L, now).isEmpty(), "a token inside the expiry margin is left to the Rust client");
        check(AccountSessionRoutingPolicy.legacyToken(TOKEN, now - 1L, now).isEmpty(), "an expired Rust token is not used");
        check(AccountSessionRoutingPolicy.legacyToken(TOKEN, 0L, now).isEmpty(), "a Rust session without an expiry is not used");
        check(AccountSessionRoutingPolicy.legacyToken(TOKEN,
                now + AccountTokenPolicy.MAX_SESSION_SECONDS * 1000L + 1L, now).isEmpty(),
            "a Rust session beyond the maximum lifetime is not used");
        check(AccountSessionRoutingPolicy.legacyToken(TOKEN, Long.MAX_VALUE, now).isEmpty(),
            "a saturated Rust expiry is not treated as an eternal token");
        check(AccountSessionRoutingPolicy.legacyToken("E".repeat(64), now + 60_000L, now).isEmpty(), "a malformed Rust token is not used");
        check(AccountSessionRoutingPolicy.legacyToken(null, now + 60_000L, now).isEmpty(), "a missing Rust token is not used");
        check(JsonPolicy.strictStringOrEmpty(TOKEN).equals(TOKEN),
            "a string Rust token is read");
        for (Object invalid : new Object[] {
                null, Boolean.TRUE, 42, new java.math.BigInteger("1".repeat(64)),
                java.util.List.of(TOKEN), java.util.Map.of("token", TOKEN)}) {
            check(JsonPolicy.strictStringOrEmpty(invalid).isEmpty(),
                "a non-string Rust token is not converted to a string");
        }
        check(AccountTokenPolicy.strictLong(1_700_000_000_000L, -1) == 1_700_000_000_000L,
            "integer saved expiries are accepted");
        check(AccountTokenPolicy.strictLong(1_700_000_000_000.5, -1) == -1,
            "fractional saved expiries are rejected");
        check(AccountTokenPolicy.strictLong(Long.MAX_VALUE, -1) == Long.MAX_VALUE,
            "exact long integers retain all their bits");
        for (Object invalid : new Object[] {
                null, true, "1700000000000", 1e40, Double.NaN, Double.POSITIVE_INFINITY,
                new java.math.BigDecimal("9223372036854775808"),
                new java.math.BigDecimal("900.000000000000000001")}) {
            check(AccountTokenPolicy.strictLong(invalid, -1) == -1,
                "non-integer or out-of-range account numbers are rejected");
        }

        check(AccountSessionRoutingPolicy.stateFor(TOKEN).equals(AccountSessionRoutingPolicy.STATE_SIGNED_IN), "a token is reported as signed in");
        check(AccountSessionRoutingPolicy.stateFor("").equals(AccountSessionRoutingPolicy.STATE_SIGNED_OUT), "no token is reported as signed out");
        check(AccountSessionRoutingPolicy.tokenFromReply(AccountSessionRoutingPolicy.STATE_SIGNED_IN, TOKEN).equals(TOKEN), "a signed-in reply yields its token");
        check(AccountSessionRoutingPolicy.tokenFromReply(AccountSessionRoutingPolicy.STATE_SIGNED_OUT, "").isEmpty(), "a signed-out reply yields no token");
        check(AccountSessionRoutingPolicy.sharedTokenFromReply(
                AccountSessionRoutingPolicy.STATE_SIGNED_IN, TOKEN).equals(TOKEN),
            "a shared signed-in reply yields its token");
        for (String[] reply : new String[][] {
                {AccountSessionRoutingPolicy.STATE_UNAVAILABLE, ""},
                {AccountSessionRoutingPolicy.STATE_SIGNED_IN, ""},
                {AccountSessionRoutingPolicy.STATE_SIGNED_IN, "not-a-token"},
                {null, TOKEN},
                {"unknown", TOKEN}}) {
            boolean unavailable = false;
            try {
                AccountSessionRoutingPolicy.tokenFromReply(reply[0], reply[1]);
            } catch (IllegalStateException expected) {
                unavailable = true;
            }
            check(unavailable, "reply " + reply[0] + " is a retry, not a sign-out");
        }
        for (String[] reply : new String[][] {
                {AccountSessionRoutingPolicy.STATE_UNAVAILABLE, ""},
                {AccountSessionRoutingPolicy.STATE_SIGNED_OUT, ""},
                {AccountSessionRoutingPolicy.STATE_SIGNED_IN, "not-a-token"},
                {null, TOKEN},
                {"unknown", TOKEN}}) {
            boolean unavailable = false;
            try {
                AccountSessionRoutingPolicy.sharedTokenFromReply(reply[0], reply[1]);
            } catch (IllegalStateException expected) {
                unavailable = true;
            }
            check(unavailable, "shared reply " + reply[0] + " is unavailable");
        }

        // 不持有会话的进程（键盘）只从主进程拿令牌：不登录、不退出、不跑 Rust 的账号操作，也不自己发刷新请求。
        AtomicInteger accountCalls = new AtomicInteger();
        AtomicInteger requests = new AtomicInteger();
        BackendAccount.Requester requester = (method, path, body, token) -> {
            requests.incrementAndGet();
            throw new IllegalStateException("no network in this smoke");
        };
        check(secondary(refused -> login(TOKEN, "user-a"), accountCalls, requester).accessToken().equals(TOKEN),
            "the owner's token is used");
        check(secondary(refused -> login(TOKEN, "user-a"), accountCalls, requester).signedIn(),
            "an owner token means signed in");
        check(secondary(refused -> login("", ""), accountCalls, requester).accessToken().isEmpty(),
            "a signed-out owner means signed out");
        check(secondary(refused -> null, accountCalls, requester).accessToken().isEmpty(),
            "a missing reply means signed out");
        check(secondary(refused -> login("not-a-token", "user-a"), accountCalls, requester).accessToken().isEmpty(),
            "a malformed reply is not used as a token");
        check(secondary(refused -> login(TOKEN, ""), accountCalls, requester).accessToken().isEmpty(),
            "a token without the account it belongs to is not used");
        check(secondary(refused -> {
            throw new IllegalArgumentException("Unknown authority");
        }, accountCalls, requester).accessToken().isEmpty(), "an unreachable owner means signed out rather than a local refresh");
        boolean retry = false;
        try {
            secondary(refused -> {
                throw new IllegalStateException("account session unavailable");
            }, accountCalls, requester).currentAccessToken();
        } catch (IllegalStateException expected) {
            retry = true;
        }
        check(retry, "an owner that cannot tell is reported to callers that word a retry");
        check(requests.get() == 0, "a non-owning process never refreshes");

        // 页面记下的是账号编号（user.id）：换成别的账号以后，旧页面排队的请求连驱动和密钥串都不碰。
        AtomicInteger clipboardRequests = new AtomicInteger();
        BackendAccount clipboard = secondary(refused -> login(TOKEN, "user-b"), accountCalls,
            (method, path, body, token) -> {
                clipboardRequests.incrementAndGet();
                throw new AssertionError("old session must not send a clipboard request");
            });
        CloudClipboardE2e.Local untouched = new CloudClipboardE2e.Local(new CloudClipboardE2e.Driver() {
            @Override public CloudClipboardE2e.Step start(CloudClipboardE2e.Operation operation, String keystore, String name) {
                throw new AssertionError("old session must not start the clipboard driver");
            }
            @Override public CloudClipboardE2e.Step resume(String state, int status, String body) {
                throw new AssertionError("old session must not resume the clipboard driver");
            }
        }, new CloudClipboardE2e.Keystore() {
            @Override public String load() { throw new AssertionError("old session must not read the keystore"); }
            @Override public void save(String value, CloudClipboardE2e.Guard guard) {
                throw new AssertionError("old session must not write the keystore");
            }
            @Override public void delete() { throw new AssertionError("old session must not delete the keystore"); }
        }, "Synthetic Phone");
        boolean oldUploadCancelled = false;
        try {
            clipboard.addClipboard("synthetic note", "user-a", untouched);
        } catch (java.util.concurrent.CancellationException expected) {
            oldUploadCancelled = true;
        }
        check(oldUploadCancelled && clipboardRequests.get() == 0,
            "an upload queued by the old account cannot send using the new account");
        boolean oldListCancelled = false;
        try {
            clipboard.clipboard("", "user-a", untouched);
        } catch (java.util.concurrent.CancellationException expected) {
            oldListCancelled = true;
        }
        check(oldListCancelled && clipboardRequests.get() == 0,
            "a queued list fetch cannot switch to the new account");

        check("f".repeat(64).equals(secondary(refused -> {
            if (refused == null) return login(TOKEN, "user-a");
            check(TOKEN.equals(refused), "the rejected token reaches the owner process");
            return login("f".repeat(64), "user-a");
        }, accountCalls, requester).currentAccessToken(TOKEN)),
            "a secondary process can request one owner refresh after a 401");

        BackendAccount keyboard = secondary(refused -> login(TOKEN, "user-a"), accountCalls, requester);
        boolean loginRejected = false;
        try {
            keyboard.login(new BackendAccount.Challenge("synthetic-challenge", "synthetic-nonce"), "synthetic-token");
        } catch (IllegalStateException expected) {
            loginRejected = true;
        }
        check(loginRejected, "a non-owning process rejects Google sign-in");
        boolean emailRejected = false;
        try {
            keyboard.verifyEmailCode(new BackendAccount.EmailChallenge("synthetic-challenge", "login", 600), "123456");
        } catch (IllegalStateException expected) {
            emailRejected = true;
        }
        check(emailRejected, "a non-owning process rejects email sign-in");
        boolean appleRejected = false;
        try {
            keyboard.loginWithAppleGrant("g".repeat(43), "v".repeat(43), false, null);
        } catch (IllegalStateException expected) {
            appleRejected = true;
        }
        check(appleRejected, "a non-owning process rejects Apple sign-in");
        keyboard.signOut();
        check(accountCalls.get() == 0, "a non-owning process never runs an account operation");
        System.out.println("Android account session routing: single refreshing process passed");
    }

    /** 测试里的「主进程」：`answer` 回答令牌和账号编号，Rust 的账号操作一律不该被调用。 */
    private interface Answer {
        BackendAccount.SessionCredential get(String rejected) throws Exception;
    }

    private static BackendAccount.Sessions sessions(Answer answer, AtomicInteger accountCalls) {
        return new BackendAccount.Sessions() {
            @Override public BackendAccount.SessionCredential session(String rejected) throws Exception {
                return answer.get(rejected);
            }

            @Override public String userId() throws Exception {
                BackendAccount.SessionCredential current = answer.get(null);
                return current == null ? "" : current.sessionId();
            }

            @Override public Object account(org.json.JSONObject action) {
                accountCalls.incrementAndGet();
                throw new AssertionError("a non-owning process must not run an account operation");
            }
        };
    }

    private static BackendAccount secondary(Answer answer, AtomicInteger accountCalls, BackendAccount.Requester requester) {
        return new BackendAccount(sessions(answer, accountCalls), requester, false);
    }

    private static BackendAccount.SessionCredential login(String token, String user) {
        return new BackendAccount.SessionCredential(token, user);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

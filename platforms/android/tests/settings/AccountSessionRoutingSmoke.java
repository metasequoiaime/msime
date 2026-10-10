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
        check(AccountSessionRoutingPolicy.accepts("anonymous_access_token", 10123, 10123),
            "our own uid gets the anonymous token");
        check(!AccountSessionRoutingPolicy.accepts("anonymous_access_token", 10124, 10123),
            "another uid cannot get the anonymous token");
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
        check(AccountTokenPolicy.strictSeconds(900) == 900,
            "integer account lifetimes are accepted");
        check(AccountTokenPolicy.strictSeconds(900.5) == 0,
            "fractional account lifetimes are rejected");
        check(AccountTokenPolicy.strictSeconds(true) == 0,
            "boolean account lifetimes are rejected");
        check(AccountTokenPolicy.strictSeconds("900") == 0,
            "string account lifetimes are rejected");
        check(AccountTokenPolicy.strictSeconds(null) == 0,
            "missing account lifetimes are rejected");
        check(AccountTokenPolicy.strictSeconds(AccountTokenPolicy.MAX_SESSION_SECONDS)
                == AccountTokenPolicy.MAX_SESSION_SECONDS,
            "the maximum integer lifetime is accepted");
        check(AccountTokenPolicy.strictSeconds(AccountTokenPolicy.MAX_SESSION_SECONDS + 1) == 0,
            "lifetimes beyond the maximum are rejected");
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
        check(AccountSessionRoutingPolicy.anonymousTokenFromReply(
                AccountSessionRoutingPolicy.STATE_SIGNED_IN, TOKEN).equals(TOKEN),
            "an anonymous signed-in reply yields its token");
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
                AccountSessionRoutingPolicy.anonymousTokenFromReply(reply[0], reply[1]);
            } catch (IllegalStateException expected) {
                unavailable = true;
            }
            check(unavailable, "anonymous reply " + reply[0] + " is unavailable");
        }

        // A process that does not own the session neither reads nor writes the store and never calls the service; the token comes from the owner alone.
        AtomicInteger storeTouches = new AtomicInteger();
        AtomicInteger requests = new AtomicInteger();
        BackendAccount.SessionStore store = new BackendAccount.SessionStore() {
            @Override public String load() { storeTouches.incrementAndGet(); return null; }
            @Override public void save(String value) { storeTouches.incrementAndGet(); }
            @Override public void clear() { storeTouches.incrementAndGet(); }
        };
        BackendAccount.Requester requester = (method, path, body, token) -> {
            requests.incrementAndGet();
            throw new IllegalStateException("no network in this smoke");
        };
        check(new BackendAccount(store, requester, () -> TOKEN).accessToken().equals(TOKEN), "the owner's token is used");
        check(new BackendAccount(store, requester, () -> TOKEN).signedIn(), "an owner token means signed in");
        check(new BackendAccount(store, requester, () -> "").accessToken().isEmpty(), "a signed-out owner means signed out");
        check(new BackendAccount(store, requester, () -> null).accessToken().isEmpty(), "a missing reply means signed out");
        check(new BackendAccount(store, requester, () -> "not-a-token").accessToken().isEmpty(), "a malformed reply is not used as a token");
        check(new BackendAccount(store, requester, () -> {
            throw new IllegalArgumentException("Unknown authority");
        }).accessToken().isEmpty(), "an unreachable owner means signed out rather than a local refresh");
        boolean retry = false;
        try {
            new BackendAccount(store, requester, () -> {
                throw new IllegalStateException("account session unavailable");
            }).currentAccessToken();
        } catch (IllegalStateException expected) {
            retry = true;
        }
        check(retry, "an owner that cannot tell is reported to callers that word a retry");
        check(storeTouches.get() == 0, "a non-owning process never touches the session store");
        check(requests.get() == 0, "a non-owning process never refreshes");

        AtomicInteger clipboardRequests = new AtomicInteger();
        BackendAccount.TokenSource switchedOwner = new BackendAccount.TokenSource() {
            @Override public String accessToken() { return TOKEN; }
            @Override public BackendAccount.SessionCredential session(String rejected) {
                return new BackendAccount.SessionCredential(TOKEN, "session-b");
            }
        };
        BackendAccount clipboard = new BackendAccount(store, (method, path, body, token) -> {
            clipboardRequests.incrementAndGet();
            throw new AssertionError("old session must not send a clipboard request");
        }, switchedOwner);
        boolean oldUploadCancelled = false;
        try {
            clipboard.addClipboard("synthetic note", "session-a");
        } catch (java.util.concurrent.CancellationException expected) {
            oldUploadCancelled = true;
        }
        check(oldUploadCancelled && clipboardRequests.get() == 0,
            "an upload queued by the old session cannot send using the new session");
        boolean oldListCancelled = false;
        try {
            clipboard.clipboard("", "session-a");
        } catch (java.util.concurrent.CancellationException expected) {
            oldListCancelled = true;
        }
        check(oldListCancelled && clipboardRequests.get() == 0,
            "a queued list fetch cannot switch to the new session");

        BackendAccount.TokenSource rotatingOwner = new BackendAccount.TokenSource() {
            @Override public String accessToken() { return TOKEN; }
            @Override public String accessToken(String rejectedToken) {
                check(TOKEN.equals(rejectedToken), "the rejected token reaches the owner process");
                return "f".repeat(64);
            }
        };
        check("f".repeat(64).equals(
            new BackendAccount(store, requester, rotatingOwner).currentAccessToken(TOKEN)),
            "a secondary process can request one owner refresh after a 401");

        AtomicInteger loginRequests = new AtomicInteger();
        BackendAccount.SessionStore loginStore = new BackendAccount.SessionStore() {
            @Override public String load() { return null; }
            @Override public void save(String value) { loginRequests.incrementAndGet(); }
            @Override public void clear() { loginRequests.incrementAndGet(); }
        };
        BackendAccount secondary = new BackendAccount(loginStore, (method, path, body, token) -> {
            loginRequests.incrementAndGet();
            return new org.json.JSONObject()
                .put("access_token", TOKEN)
                .put("refresh_token", TOKEN)
                .put("token_type", "Bearer")
                .put("expires_in", 900);
        }, () -> TOKEN);
        boolean loginRejected = false;
        try {
            secondary.login(null, null);
        } catch (RuntimeException error) {
            loginRejected = error instanceof IllegalStateException;
        }
        check(loginRejected, "a non-owning process rejects local sign-in");
        check(loginRequests.get() == 0, "a non-owning process never signs in through its local store");

        AtomicInteger signOutTouches = new AtomicInteger();
        BackendAccount.SessionStore signOutStore = new BackendAccount.SessionStore() {
            @Override public String load() { return TOKEN_SESSION; }
            @Override public void save(String value) { signOutTouches.incrementAndGet(); }
            @Override public void clear() { signOutTouches.incrementAndGet(); }
        };
        new BackendAccount(signOutStore, requester, () -> TOKEN).signOut();
        check(signOutTouches.get() == 0, "a non-owning process never signs out through its local store");

        BackendAccount.SessionStore failingStore = new BackendAccount.SessionStore() {
            @Override public String load() { return TOKEN_SESSION; }
            @Override public void save(String value) { throw new AssertionError("no save on sign-out"); }
            @Override public void clear() throws Exception { throw new java.io.IOException("synthetic failure"); }
        };
        boolean clearFailed = false;
        try {
            new BackendAccount(failingStore, requester).signOut();
        } catch (IllegalStateException expected) {
            clearFailed = true;
        }
        check(clearFailed, "sign-out must report a failed persistent clear");
        check(TOKEN_SESSION.equals(failingStore.load()), "a failed clear leaves the stored session present");
        System.out.println("Android account session routing: single refreshing process passed");
    }

    private static final String TOKEN_SESSION = "synthetic-session";

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}

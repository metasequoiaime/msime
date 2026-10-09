package app.msime.android;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.Process;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 向本应用的其他进程提供登录账号或匿名账号的 access token，以及云同步的开关与改动标记（`sync_state` / `sync_dirty`）。
 *
 * <p>Declared in the main process and not exported, so only this uid can reach it. The `:ime` keyboard therefore never holds a refresh token and never refreshes, which is what keeps the rotating refresh token from being spent twice. Which session answers is {@link AccountSessionRoutingPolicy#source}: the native sign-in's own session, refreshed by {@link BackendAccount#owningSession} under the existing in-process lock, or else the combined package's Rust-owned session, read but never refreshed here. The token is returned in the reply and nowhere else: nothing here logs it.
 */
public final class AccountSessionProvider extends ContentProvider {
    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        if (!AccountSessionRoutingPolicy.accepts(method, Binder.getCallingUid(), Process.myUid())) {
            throw new SecurityException("account session");
        }
        Context context = getContext();
        if (AccountSessionRoutingPolicy.syncMethod(method)) return sync(context, method, arg);
        Bundle reply = new Bundle();
        String token = "";
        String state;
        try {
            if (context == null) throw new IllegalStateException("account session");
            token = AccountSessionRoutingPolicy.METHOD_ANONYMOUS_ACCESS_TOKEN.equals(method)
                ? currentAnonymousToken(context, rejectedToken(extras)) : currentToken(context, arg);
            state = AccountSessionRoutingPolicy.stateFor(token);
        } catch (Exception | LinkageError error) {
            token = "";
            state = AccountSessionRoutingPolicy.STATE_UNAVAILABLE;
        }
        reply.putString(AccountSessionRoutingPolicy.KEY_STATE, state);
        reply.putString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN, token);
        return reply;
    }

    /** 同步状态与改动标记由主进程代 `:ime` 读写；回复里只有开关、登录方式、账号 ID 和「已记下」，没有令牌。 */
    private static Bundle sync(Context context, String method, String section) {
        Bundle reply = new Bundle();
        if (context == null) return reply;
        if (AccountSessionRoutingPolicy.METHOD_SYNC_STATE.equals(method)) {
            reply.putBoolean(AccountSessionRoutingPolicy.KEY_SYNC_ENABLED, SyncSwitch.enabled(context));
            reply.putString(AccountSessionRoutingPolicy.KEY_LOGIN_KIND, SyncSwitch.loginKind(context));
            // Do not acquire bindingLock here: the snapshot worker may hold the queue lock
            // while calling us, and sign-out holds bindingLock while cancelling that queue.
            reply.putString(AccountSessionRoutingPolicy.KEY_ACCOUNT_ID, SyncSwitch.accountId(context));
            return reply;
        }
        if (!SyncSwitch.validSection(section)) throw new IllegalArgumentException("unknown sync section");
        SyncSwitch.markDirty(context, section);
        reply.putBoolean(AccountSessionRoutingPolicy.KEY_SYNC_MARKED, true);
        return reply;
    }

    private static String currentToken(Context context, String rejectedToken) throws Exception {
        BackendAccount own = BackendAccount.owningSession(context);
        boolean ownSession = own.hasSession();
        JSONObject legacy = ownSession ? null : legacySession(context);
        return switch (AccountSessionRoutingPolicy.source(ownSession, legacy != null)) {
            case OWN -> own.currentAccessToken(rejectedToken);
            case LEGACY_READ_ONLY -> {
                String token = AccountSessionRoutingPolicy.legacyToken(
                    JsonPolicy.strictStringOrEmpty(
                        legacy.getJSONObject("tokens").opt("access_token")),
                    AccountTokenPolicy.strictLong(legacy.opt("expires_at_unix_ms"), 0), System.currentTimeMillis());
                // Still signed in, but only the Rust client may refresh this session, and it does so when the app runs; say "not now" rather than "signed out".
                if (token.isEmpty() || token.equals(rejectedToken)) {
                    throw new IllegalStateException("account session needs the app");
                }
                yield token;
            }
            case NONE -> "";
        };
    }

    /** 匿名会话也只能由主进程读写，避免键盘进程各自刷新同一枚轮换令牌。 */
    private static String currentAnonymousToken(Context context, String rejectedToken) throws Exception {
        return new BackendAnonymousAccount(context).accessToken(rejectedToken);
    }

    private static String rejectedToken(Bundle extras) {
        if (extras == null) return null;
        String token = extras.getString(AccountSessionRoutingPolicy.KEY_REJECTED_ACCESS_TOKEN);
        return AccountTokenPolicy.validToken(token) ? token : null;
    }

    /** The combined package's session as its Rust client saved it, or null when there is none; never refreshed here. */
    private static JSONObject legacySession(Context context) throws Exception {
        String saved = new AndroidAccountSessionStorage(context).load();
        if (saved == null) return null;
        try {
            JSONObject session = new JSONObject(saved);
            return session.optJSONObject("tokens") == null ? null : session;
        } catch (JSONException malformed) {
            return null;
        }
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection,
                                  String[] selectionArgs, String sortOrder) {
        throw new UnsupportedOperationException();
    }

    @Override public String getType(Uri uri) { return null; }

    @Override public Uri insert(Uri uri, ContentValues values) {
        throw new UnsupportedOperationException();
    }

    @Override public int delete(Uri uri, String selection, String[] selectionArgs) {
        throw new UnsupportedOperationException();
    }

    @Override public int update(Uri uri, ContentValues values, String selection,
                                String[] selectionArgs) {
        throw new UnsupportedOperationException();
    }
}

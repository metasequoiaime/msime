package app.msime.android;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.os.Binder;
import android.os.Bundle;
import android.os.Process;
import android.util.Base64;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.LinkOption;
import java.nio.file.Path;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 向本应用的其他进程提供登录账号或「登录的优先、否则匿名」的 access token，代 `:ime` 上传语音贡献，以及主进程记着的账号绑定（`sync_state`）。
 *
 * <p>Declared in the main process and not exported, so only this uid can reach it. The `:ime` keyboard therefore never holds a refresh token and never refreshes, which is what keeps the rotating refresh token from being spent twice. Which session answers is {@link AccountSessionRoutingPolicy#source}: the native sign-in's own session（Rust 经 {@link AccountStore} 持有并刷新）, or else the combined package's Rust-owned session, read but never refreshed here. The token is returned in the reply and nowhere else: nothing here logs it. 回复里的 session_id 是账号编号（user.id）。
 */
public final class AccountSessionProvider extends ContentProvider {
    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        if (!AccountSessionRoutingPolicy.accepts(method, Binder.getCallingUid(), Process.myUid())) {
            throw new SecurityException("account session");
        }
        Context context = getContext();
        if (AccountSessionRoutingPolicy.syncMethod(method)) return sync(context);
        if (AccountSessionRoutingPolicy.METHOD_VOICE_CONTRIBUTION.equals(method)) return voice(context, extras);
        Bundle reply = new Bundle();
        String token = "";
        String sessionId = "";
        String state;
        try {
            if (context == null) throw new IllegalStateException("account session");
            if (AccountSessionRoutingPolicy.METHOD_SHARED_ACCESS_TOKEN.equals(method)) {
                token = new BackendAnonymousAccount(context).accessToken(rejectedToken(extras));
            } else {
                BackendAccount.SessionCredential session = currentToken(context, arg);
                token = session.token();
                sessionId = session.sessionId();
            }
            state = AccountSessionRoutingPolicy.stateFor(token);
        } catch (Exception | LinkageError error) {
            token = "";
            sessionId = "";
            state = AccountSessionRoutingPolicy.STATE_UNAVAILABLE;
        }
        reply.putString(AccountSessionRoutingPolicy.KEY_STATE, state);
        reply.putString(AccountSessionRoutingPolicy.KEY_ACCESS_TOKEN, token);
        if (AccountSessionRoutingPolicy.METHOD_ACCESS_TOKEN.equals(method))
            reply.putString(AccountSessionRoutingPolicy.KEY_SESSION_ID, sessionId);
        return reply;
    }

    /** 主进程记着的账号绑定（账号编号与登录代次）由这里交给 `:ime`；回复里没有令牌。同步开关和改动标记 `:ime` 直接经 Rust 读写（{@link SyncSignals}）。 */
    private static Bundle sync(Context context) {
        Bundle reply = new Bundle();
        if (context == null) return reply;
        // 这里不能取 bindingLock：退出登录持着它取消快照队列，而键盘准备这个队列时会来问当前账号。
        SyncSwitch.Binding binding = SyncSwitch.bindingSnapshot(context);
        if (binding != null) {
            reply.putString(AccountSessionRoutingPolicy.KEY_ACCOUNT_ID, binding.accountId());
            reply.putLong(AccountSessionRoutingPolicy.KEY_BINDING_GENERATION, binding.generation());
        }
        return reply;
    }

    private static BackendAccount.SessionCredential currentToken(Context context, String rejectedToken) throws Exception {
        BackendAccount own = BackendAccount.owningSession(context);
        boolean ownSession = own.hasSession();
        JSONObject legacy = ownSession ? null : legacySession(context);
        return switch (AccountSessionRoutingPolicy.source(ownSession, legacy != null)) {
            case OWN -> own.currentSession(rejectedToken);
            case LEGACY_READ_ONLY -> {
                String token = AccountSessionRoutingPolicy.legacyToken(
                    JsonPolicy.strictStringOrEmpty(
                        legacy.getJSONObject("tokens").opt("access_token")),
                    AccountTokenPolicy.strictLong(legacy.opt("expires_at_unix_ms"), 0), System.currentTimeMillis());
                // Still signed in, but only the Rust client may refresh this session, and it does so when the app runs; say "not now" rather than "signed out".
                if (token.isEmpty() || token.equals(rejectedToken)) {
                    throw new IllegalStateException("account session needs the app");
                }
                // 与原生登录一样按账号编号绑定：Rust 刷新这份会话不会让键盘里进行中的请求作废。
                JSONObject user = legacy.getJSONObject("tokens").optJSONObject("user");
                String sessionId = user == null ? "" : JsonPolicy.strictStringOrEmpty(user.opt("id"));
                if (sessionId.isEmpty()) throw new IllegalStateException("account session unavailable");
                yield new BackendAccount.SessionCredential(token, sessionId);
            }
            case NONE -> new BackendAccount.SessionCredential("", "");
        };
    }

    /**
     * 代 `:ime` 上传一次语音贡献。音频在 `:ime` 写进 cache 下的 {@link AccountSessionRoutingPolicy#VOICE_CONTRIBUTION_DIRECTORY}，这里只读那个目录里的普通文件（不跟随符号链接，最多 2 MiB），读完就删；元数据和音频都交给 Rust 校验，Rust 只用本机匿名账号。还没有匿名会话时先注册一次再试。回复里是服务端给的编号或 Rust 的错误码。阻塞网络，最长约 60 秒。
     */
    private static Bundle voice(Context context, Bundle extras) {
        Bundle reply = new Bundle();
        String code;
        try {
            if (context == null || extras == null) throw new AccountStore.Failure("account_invalid", 0);
            byte[] audio = voiceAudio(context, extras.getString(AccountSessionRoutingPolicy.KEY_VOICE_AUDIO_PATH));
            JSONObject action = new JSONObject().put("operation", "voice_contribution")
                .put("language", extras.getString(AccountSessionRoutingPolicy.KEY_VOICE_LANGUAGE, ""))
                .put("provider", extras.getString(AccountSessionRoutingPolicy.KEY_VOICE_PROVIDER, ""))
                .put("transcript", extras.getString(AccountSessionRoutingPolicy.KEY_VOICE_TRANSCRIPT, ""))
                .put("app_version", extras.getString(AccountSessionRoutingPolicy.KEY_VOICE_APP_VERSION, ""))
                .put("audio", Base64.encodeToString(audio, Base64.NO_WRAP));
            Object value;
            try {
                value = AccountStore.account(context, action);
            } catch (AccountStore.Failure failure) {
                if (!"account_unauthorized".equals(failure.code)) throw failure;
                new BackendAnonymousAccount(context).ensureRegistered();
                value = AccountStore.account(context, action);
            }
            String id = value instanceof JSONObject answer ? JsonPolicy.strictStringOrEmpty(answer.opt("id")) : "";
            reply.putString(AccountSessionRoutingPolicy.KEY_VOICE_ID, id);
            return reply;
        } catch (AccountStore.Failure failure) {
            code = failure.code;
        } catch (BackendAnonymousAccount.RateLimited limited) {
            code = "account_rate_limited";
        } catch (Exception | LinkageError error) {
            code = "account_unavailable";
        }
        reply.putString(AccountSessionRoutingPolicy.KEY_ERROR, code);
        return reply;
    }

    /** 读出并删掉 `:ime` 交来的音频文件；路径不在约定的目录里、是符号链接或超过上限时按请求不合法处理。 */
    private static byte[] voiceAudio(Context context, String path) throws AccountStore.Failure {
        if (path == null || path.isEmpty()) throw new AccountStore.Failure("account_invalid", 0);
        File cache = context.getCacheDir();
        if (cache == null) throw new AccountStore.Failure("account_storage", 0);
        Path directory = cache.toPath().resolve(AccountSessionRoutingPolicy.VOICE_CONTRIBUTION_DIRECTORY).normalize();
        Path file = directory.resolve(path).normalize();
        if (!file.isAbsolute() || !directory.equals(file.getParent()))
            throw new AccountStore.Failure("account_invalid", 0);
        try {
            if (!Files.isRegularFile(file, LinkOption.NOFOLLOW_LINKS)) throw new AccountStore.Failure("account_invalid", 0);
            byte[] audio;
            try (InputStream input = Files.newInputStream(file, LinkOption.NOFOLLOW_LINKS)) {
                audio = HttpBodyPolicy.readBounded(input, VoiceContributionApi.MAX_AUDIO_BYTES);
            }
            if (audio == null) throw new AccountStore.Failure("account_invalid", 0);
            return audio;
        } catch (IOException unreadable) {
            throw new AccountStore.Failure("account_storage", 0);
        } finally {
            try {
                Files.deleteIfExists(file);
            } catch (IOException ignored) {
                // `:ime` 在调用结束后也会删一次。
            }
        }
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

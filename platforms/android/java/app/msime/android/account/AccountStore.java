package app.msime.android;

import android.app.Application;
import android.content.Context;
import android.os.Build;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 主进程里注入给 Rust 的那一份会话存储（{@code msime_client_session_store}，#8201 迁移计划 5b）：登录会话、匿名会话、匿名身份和云剪贴板密钥串的字节都还在原来的地方，读写、编解码、校验、刷新和登录都由 Rust 做，Java 只经这里调用 Rust 的账号入口。
 *
 * <p>只在主进程里懒注册一次，flags 为 0、不给锁：SharedPreferences 在每个进程里各缓存一份，不满足 SHARED 要求的「load 返回任何进程最后一次 save 的字节」，所以 `:ime` 不注册，它经 {@link AccountSessionProvider} 向主进程要令牌。同一份存储在一个进程里注册两次，两次注册会各自缓存会话、拿同一枚会轮换的刷新令牌各刷新一次，服务端随即吊销整个会话，所以 {@link #register} 拒绝第二次。
 *
 * <p>经这里的调用都可能阻塞网络（status 和 forget 除外），不要在主线程上调用。
 */
public final class AccountStore {
    /** 原生安装包的登录会话（AndroidKeyStore 加密的 SharedPreferences），布局是 Rust 的 Native（{@code SavedAccountSession}）。 */
    static final String ACCOUNT_SESSION = "msime_account_session_v2";
    /** 本机匿名账号的会话，布局同上。 */
    static final String ANONYMOUS_SESSION = "msime_anonymous_session_v1";
    /** 匿名账号的身份 {@code {subject, secret}}。 */
    static final String ANONYMOUS_IDENTITY = "msime_anonymous_account_v1";
    private static final Object LOCK = new Object();
    /** 匿名身份「不存在才写」的检查与写入在这把锁里做，Rust 的几个线程不会各写一份。 */
    private static final Object IDENTITY_LOCK = new Object();
    private static long registered;

    /** Rust 账号入口的一次失败：错误码（account_unauthorized 等），以及服务端给的 Retry-After 秒数（没有时为 0）。 */
    public static final class Failure extends Exception {
        private static final long serialVersionUID = 1L;
        public final String code;
        public final long retryAfterSeconds;

        public Failure(String code, long retryAfterSeconds) {
            super(code);
            this.code = code == null || code.isEmpty() ? "account_unavailable" : code;
            this.retryAfterSeconds = BoundsPolicy.nonNegative(retryAfterSeconds);
        }
    }

    private AccountStore() {}

    /** 一次失败的错误码；不是 Rust 账号入口报的失败时为空串。 */
    public static String code(Throwable error) {
        return error instanceof Failure failure ? failure.code : "";
    }

    /** 本进程注册好的 store；第一次调用时注册。不是主进程时抛出。 */
    static long id(Context context) throws Failure {
        synchronized (LOCK) {
            if (registered > 0) return registered;
            return register(context, Application.getProcessName());
        }
    }

    /**
     * 注册一次。`processName` 不是本应用的主进程，或本进程已经注册过时抛出 {@link IllegalStateException}，不碰 Rust；注册本身失败时抛 {@link Failure}。
     *
     * <p>{@link #id} 之外只有设备冒烟直接调用它，核对这两道守卫。
     */
    static long register(Context context, String processName) throws Failure {
        synchronized (LOCK) {
            Context application = context.getApplicationContext();
            if (!AccountSessionRoutingPolicy.ownsSession(processName, application.getPackageName()))
                throw new IllegalStateException("account store owner");
            if (registered > 0) throw new IllegalStateException("account store registered");
            Object value = value(NativeClient.sessionStoreRegister(new Items(application), clientAgent(application)));
            long id = value instanceof JSONObject object ? NumberPolicy.strictLong(object.opt("session_store"), 0) : 0;
            if (id <= 0) throw new Failure("account_unavailable", 0);
            registered = id;
            return id;
        }
    }

    /**
     * 本宿主自报的 User-Agent，由 Rust 拼装和清洗：`msime-android/<versionName> (<Build.MODEL>; Android <RELEASE>; edition=<id>)`。服务端在「我的设备」里按登录时的它显示这台设备。拼不出时为 null，请求以 MSIME/Android 发出。
     */
    static String clientAgent(Context context) {
        try {
            JSONObject request = new JSONObject().put("platform", "android")
                .put("version", AppVersionPolicy.versionName(context, ""))
                .put("device", Build.MODEL == null ? "" : Build.MODEL)
                .put("system", "Android " + (Build.VERSION.RELEASE == null ? "" : Build.VERSION.RELEASE))
                .put("edition", AppEdition.current().id());
            Object agent = value(NativeClient.clientAgent(request.toString()));
            return agent instanceof String text && !text.isEmpty() ? text : null;
        } catch (Failure | JSONException | RuntimeException error) {
            return null;
        }
    }

    /** 跑一个 {@code msime_client_account} 操作，返回它的 value（对象、数组、布尔、字符串或 {@link JSONObject#NULL}）。 */
    static Object account(Context context, JSONObject action) throws Failure {
        final String request;
        try {
            request = new JSONObject().put("session_store", id(context)).put("action", action).toString();
        } catch (JSONException impossible) {
            throw new Failure("account_invalid", 0);
        }
        return value(NativeClient.account(request));
    }

    /** 登录的优先、否则匿名账号的令牌：{@code {access_token, user_id, anonymous}}。rejected 是刚被服务端以 401 拒绝的令牌，给了就强制刷新。两种会话都没有时是 account_unauthorized。 */
    static JSONObject accessToken(Context context, String rejectedToken) throws Failure {
        final String request;
        try {
            JSONObject body = new JSONObject().put("session_store", id(context));
            if (rejectedToken != null && !rejectedToken.isEmpty()) body.put("rejected_token", rejectedToken);
            request = body.toString();
        } catch (JSONException impossible) {
            throw new Failure("account_invalid", 0);
        }
        Object value = value(NativeClient.accountAccessToken(request));
        if (!(value instanceof JSONObject token)) throw new Failure("account_unavailable", 0);
        return token;
    }

    /** 信封 {@code {ok, value}} 或 {@code {ok:false, error, retry_after_seconds?}}；读不懂时按 account_unavailable。{@link RustAccount} 的其他入口也用它读回答。 */
    static Object value(String response) throws Failure {
        final JSONObject root;
        try {
            root = new JSONObject(response);
        } catch (JSONException | RuntimeException malformed) {
            throw new Failure("account_unavailable", 0);
        }
        if (!JsonPolicy.strictTrue(root.opt("ok"))) {
            Object code = root.opt("error");
            throw new Failure(code instanceof String text ? text : "",
                NumberPolicy.strictLong(root.opt("retry_after_seconds"), 0));
        }
        Object value = root.opt("value");
        return value == null ? JSONObject.NULL : value;
    }

    /**
     * 交给 Rust 的四个存储项，都放在原来的位置：登录会话、匿名会话和匿名身份是 AndroidKeyStore 加密的 SharedPreferences，云剪贴板密钥串是 {@link CloudClipboardKeystore} 的那个文件（键盘进程也直接读写它）。
     *
     * <p>Rust 可能在任意线程上并发调用这里，包括它自己的线程。删除密钥串就在调用线程上做完再返回，返回以后 load 读到的就是没有：Rust 在持着会话锁时删它，而 Java 的端到端加密驱动在持着密钥串的锁时会经 Rust 核对会话（键盘进程经 provider 绕回主进程），所以 {@link CloudClipboardKeystore#delete} 不等那把锁，只等一把从不在持有时调外部代码的短锁，正在保存的一方核对到删过就不再写。删不掉时报失败，与文件存储的会话相同。
     */
    private static final class Items implements NativeClient.SessionStore {
        private final AndroidAccountSessionStorage account;
        private final AndroidAccountSessionStorage anonymous;
        private final AndroidAccountSessionStorage identity;
        private final CloudClipboardE2e.Keystore keystore;

        Items(Context application) {
            this(new AndroidAccountSessionStorage(application, ACCOUNT_SESSION),
                new AndroidAccountSessionStorage(application, ANONYMOUS_SESSION),
                new AndroidAccountSessionStorage(application, ANONYMOUS_IDENTITY),
                CloudClipboardKeystore.deferred(application));
        }

        /** JVM 冒烟（AccountStoreKeystoreItemSmoke）经这里换成临时目录里的密钥串；三个 SharedPreferences 项离不开 Android，那里不构造。 */
        Items(AndroidAccountSessionStorage account, AndroidAccountSessionStorage anonymous,
                AndroidAccountSessionStorage identity, CloudClipboardE2e.Keystore keystore) {
            this.account = account;
            this.anonymous = anonymous;
            this.identity = identity;
            this.keystore = keystore;
        }

        @Override public byte[] load(int item) {
            try {
                String value = switch (item) {
                    case ITEM_ACCOUNT -> account.load();
                    case ITEM_ANONYMOUS -> anonymous.load();
                    case ITEM_ANONYMOUS_IDENTITY -> identity.load();
                    case ITEM_CLIPBOARD_KEYSTORE -> keystore.load();
                    default -> throw new IllegalArgumentException("session item");
                };
                return value == null ? null : TextPolicy.utf8Bytes(value);
            } catch (RuntimeException error) {
                throw error;
            } catch (Exception error) {
                throw new IllegalStateException("account_storage", error);
            }
        }

        @Override public int save(int item, byte[] bytes) {
            try {
                String value = TextPolicy.utf8(bytes);
                switch (item) {
                    case ITEM_ACCOUNT -> account.save(value);
                    case ITEM_ANONYMOUS -> anonymous.save(value);
                    case ITEM_ANONYMOUS_IDENTITY -> {
                        synchronized (IDENTITY_LOCK) {
                            if (identity.load() != null) return 1;
                            identity.save(value);
                        }
                    }
                    case ITEM_CLIPBOARD_KEYSTORE -> keystore.save(value, () -> { });
                    default -> throw new IllegalArgumentException("session item");
                }
                return 0;
            } catch (RuntimeException error) {
                throw error;
            } catch (Exception error) {
                throw new IllegalStateException("account_storage", error);
            }
        }

        @Override public int remove(int item) {
            try {
                switch (item) {
                    case ITEM_ACCOUNT -> account.clear();
                    case ITEM_ANONYMOUS -> anonymous.clear();
                    case ITEM_ANONYMOUS_IDENTITY -> identity.clear();
                    case ITEM_CLIPBOARD_KEYSTORE -> keystore.delete();
                    default -> throw new IllegalArgumentException("session item");
                }
                return 0;
            } catch (RuntimeException error) {
                throw error;
            } catch (Exception error) {
                throw new IllegalStateException("account_storage", error);
            }
        }
    }
}

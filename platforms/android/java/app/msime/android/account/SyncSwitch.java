package app.msime.android;

import android.content.Context;
import android.content.SharedPreferences;
import java.util.List;

/**
 * 主进程记着的账号绑定：当前登录的账号编号、登录方式和绑定代次，以及同步分类的名字。
 *
 * <p>存在宿主本地的 `SharedPreferences("msime_sync_v1")`。云同步的开关和进度原来也存在这里，#8201 迁移计划 5d 起改由 Rust 的 `cloud-sync/state.json` 持有（{@link RustCloudSync}、{@link SyncSignals}）；这份偏好里旧版本留下的开关、游标和改动代数只在升级后第一次启动时由 Rust 的 `import_legacy` 读一次，Java 不再读写它们，退出登录和换账号时随 {@link #clear} 一起清掉。账号绑定仍是 `:ime` 判断云剪贴板和候选翻译归属的依据，只在主进程读写，`:ime` 经 {@link AccountSessionProvider} 的 `sync_state` 读。
 */
public final class SyncSwitch {
    public static final String STORE = "msime_sync_v1";
    public static final String SETTINGS = "settings";
    public static final String PHRASES = "phrases";
    public static final String SKINS = "skins";
    public static final String DICTIONARY = "dictionary";
    /** 同步分类，名字与 Rust 的 `mark_dirty` 认的相同。 */
    public static final List<String> SECTIONS = List.of(SETTINGS, PHRASES, SKINS, DICTIONARY);
    /** 真实账号的登录方式；匿名账号不同步，所以没有对应的值。 */
    public static final List<String> LOGIN_KINDS = List.of("google", "apple", "email");

    static final String KEY_ACCOUNT_ID = "account_id";
    static final String KEY_LOGIN_KIND = "login_kind";
    private static final String KEY_BINDING_GENERATION = "binding_generation";

    /** 账号绑定的读改写都在这把锁里；登录、退出与依赖绑定的本机写入也用它串行化。 */
    private static final Object BINDING_LOCK = new Object();

    /** 账号绑定锁：不能与换绑账号交错的本机写入（例如云同步写回 `host_writes`）在这把锁里做。 */
    public static Object bindingLock() { return BINDING_LOCK; }

    private SyncSwitch() {}

    /** 是不是已知的同步分类；记改动之前先过这一关。 */
    public static boolean validSection(String section) {
        return section != null && SECTIONS.contains(section);
    }

    public static boolean validLoginKind(String kind) {
        return kind != null && LOGIN_KINDS.contains(kind);
    }

    private static SharedPreferences store(Context context) {
        return context.getApplicationContext().getSharedPreferences(STORE, Context.MODE_PRIVATE);
    }

    public static String accountId(Context context) {
        return store(context).getString(KEY_ACCOUNT_ID, "");
    }

    /** 当前登录方式（google / apple / email），未登录时为空字符串。 */
    public static String loginKind(Context context) {
        return store(context).getString(KEY_LOGIN_KIND, "");
    }

    /** Monotonic identity generation used to fence in-flight work across sign-out and re-login. */
    public static long bindingGeneration(Context context) {
        synchronized (BINDING_LOCK) {
            return store(context).getLong(KEY_BINDING_GENERATION, 0L);
        }
    }

    /** A lock-free consistent snapshot for provider calls, which must not take bindingLock. */
    public static Binding bindingSnapshot(Context context) {
        SharedPreferences values = store(context);
        long before = values.getLong(KEY_BINDING_GENERATION, 0L);
        String accountId = values.getString(KEY_ACCOUNT_ID, "");
        long after = values.getLong(KEY_BINDING_GENERATION, 0L);
        return before == after ? new Binding(accountId, after) : null;
    }

    public record Binding(String accountId, long generation) {}

    /**
     * 记下这次登录的账号与方式。账号与已记录的不同（包括原来没有记录）时先 {@link #clear}。云同步的进度由 Rust 按会话里的用户 id 自己对齐，不看这里。
     *
     * @param accountId 服务端的用户 id；拿不到时传空字符串，此时一律按换账号处理
     */
    public static void bindAccount(Context context, String accountId, String loginKind) {
        if (!validLoginKind(loginKind)) throw new IllegalArgumentException("unknown login kind");
        String id = accountId == null ? "" : accountId;
        if (id.isEmpty()) {
            // 换到了令牌却没有用户 id，这样的绑定不能用：先清空，等 ProfilePage 读到资料后再绑定真实的账号编号。
            clear(context);
            return;
        }
        if (!id.equals(accountId(context))) clear(context);
        synchronized (BINDING_LOCK) {
            SharedPreferences values = store(context);
            long generation = values.getLong(KEY_BINDING_GENERATION, 0L);
            values.edit().putString(KEY_ACCOUNT_ID, id).putString(KEY_LOGIN_KIND, loginKind)
                .putLong(KEY_BINDING_GENERATION, generation + 1L).commit();
        }
    }

    /** 退出登录与换账号时调用：清空账号绑定（连同旧版本留下的同步开关与进度），代次加一。同步写盘，返回时已经生效。 */
    public static void clear(Context context) {
        synchronized (BINDING_LOCK) {
            SharedPreferences values = store(context);
            long generation = values.getLong(KEY_BINDING_GENERATION, 0L);
            values.edit().clear().putLong(KEY_BINDING_GENERATION, generation + 1L).commit();
        }
    }
}

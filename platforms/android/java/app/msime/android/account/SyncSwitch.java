package app.msime.android;

import android.content.Context;
import android.content.SharedPreferences;
import java.util.List;

/**
 * 云同步的本机开关与进度：开关、当前绑定的账号与登录方式、各分类的游标与待上传标记、上次同步时间。
 *
 * <p>只存在宿主本地的 `SharedPreferences("msime_sync_v1")`，不进共享偏好，也不随设置同步。开关默认关闭，登录本身不打开它；退出登录和换账号时由登录流程（home/SignIn）调用 {@link #clear}，开关随之关闭、游标清空。只在主进程读写；`:ime` 进程经 {@link AccountSessionProvider} 的 `sync_state` / `sync_dirty` 间接访问。
 */
public final class SyncSwitch {
    public static final String STORE = "msime_sync_v1";
    public static final String SETTINGS = "settings";
    public static final String PHRASES = "phrases";
    public static final String SKINS = "skins";
    public static final String DICTIONARY = "dictionary";
    /** 同步分类，顺序即同步页的展示顺序。 */
    public static final List<String> SECTIONS = List.of(SETTINGS, PHRASES, SKINS, DICTIONARY);
    /** 真实账号的登录方式；匿名账号不同步，所以没有对应的值。 */
    public static final List<String> LOGIN_KINDS = List.of("google", "apple", "email");

    static final String KEY_ENABLED = "enabled";
    static final String KEY_ACCOUNT_ID = "account_id";
    static final String KEY_LOGIN_KIND = "login_kind";
    static final String KEY_LAST_SYNCED_AT = "last_synced_at";

    private SyncSwitch() {}

    /** 是不是已知的同步分类；provider 收到的分类名先过这一关。 */
    public static boolean validSection(String section) {
        return section != null && SECTIONS.contains(section);
    }

    public static boolean validLoginKind(String kind) {
        return kind != null && LOGIN_KINDS.contains(kind);
    }

    /** 一个分类的游标在存储里的键名。 */
    static String cursorKey(String section) {
        return "cursor_" + requireSection(section);
    }

    /** 一个分类「本机有改动尚未上传」标记的键名。 */
    static String dirtyKey(String section) {
        return "dirty_" + requireSection(section);
    }

    private static String requireSection(String section) {
        if (!validSection(section)) throw new IllegalArgumentException("unknown sync section");
        return section;
    }

    private static SharedPreferences store(Context context) {
        return context.getApplicationContext().getSharedPreferences(STORE, Context.MODE_PRIVATE);
    }

    public static boolean enabled(Context context) {
        return store(context).getBoolean(KEY_ENABLED, false);
    }

    /** 打开或关闭同步；关闭时保留游标，便于重新打开后增量继续。没有真实账号时不能打开。 */
    public static void setEnabled(Context context, boolean enabled) {
        if (enabled && !validLoginKind(loginKind(context))) {
            throw new IllegalStateException("sync needs a signed-in account");
        }
        store(context).edit().putBoolean(KEY_ENABLED, enabled).apply();
    }

    public static String accountId(Context context) {
        return store(context).getString(KEY_ACCOUNT_ID, "");
    }

    /** 当前登录方式（google / apple / email），未登录时为空字符串。 */
    public static String loginKind(Context context) {
        return store(context).getString(KEY_LOGIN_KIND, "");
    }

    /**
     * 记下这次登录的账号与方式。账号与已记录的不同（包括原来没有记录）时先 {@link #clear}，所以换账号后开关总是关闭、游标从头开始。
     *
     * @param accountId 服务端的用户 id；拿不到时传空字符串，此时一律按换账号处理
     */
    public static void bindAccount(Context context, String accountId, String loginKind) {
        if (!validLoginKind(loginKind)) throw new IllegalArgumentException("unknown login kind");
        String id = accountId == null ? "" : accountId;
        if (id.isEmpty() || !id.equals(accountId(context))) clear(context);
        store(context).edit().putString(KEY_ACCOUNT_ID, id).putString(KEY_LOGIN_KIND, loginKind).commit();
    }

    public static String cursor(Context context, String section) {
        return store(context).getString(cursorKey(section), "");
    }

    public static void setCursor(Context context, String section, String cursor) {
        store(context).edit().putString(cursorKey(section), cursor == null ? "" : cursor).apply();
    }

    public static boolean dirty(Context context, String section) {
        return store(context).getBoolean(dirtyKey(section), false);
    }

    /** 标记一个分类本机有改动；同步关闭时不记，打开同步时本来就会整份比对。 */
    public static void markDirty(Context context, String section) {
        String key = dirtyKey(section);
        if (!enabled(context)) return;
        store(context).edit().putBoolean(key, true).apply();
    }

    public static void clearDirty(Context context, String section) {
        store(context).edit().remove(dirtyKey(section)).apply();
    }

    /** 上次成功同步的 Unix 毫秒时间，从未同步过为 0。 */
    public static long lastSyncedAt(Context context) {
        return store(context).getLong(KEY_LAST_SYNCED_AT, 0L);
    }

    public static void setLastSyncedAt(Context context, long unixMillis) {
        store(context).edit().putLong(KEY_LAST_SYNCED_AT, Math.max(0L, unixMillis)).apply();
    }

    /** 退出登录与换账号时调用：关闭开关，清空账号、游标、标记与同步时间。同步写盘，返回时已经生效。 */
    public static void clear(Context context) {
        store(context).edit().clear().commit();
    }
}

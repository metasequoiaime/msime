package app.msime.android;

import android.content.Context;
import android.content.SharedPreferences;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

/**
 * 云同步的本机开关与进度：开关、当前绑定的账号与登录方式、各分类的游标与待上传标记（改动代数）、本机收不下的云端常用语、上次同步时间。
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
    static final String KEY_PHRASES_UNHELD = "phrases_unheld";
    private static final String KEY_BINDING_GENERATION = "binding_generation";

    /** 待上传标记的读改写都在这把锁里：provider 的 binder 线程与 CloudSync 的工作线程同在主进程里并发调用。 */
    private static final Object DIRTY_LOCK = new Object();

    /** Shared lock for sync-owned local writes that must exclude account rebinding. */
    public static Object bindingLock() { return DIRTY_LOCK; }

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

    /** 一个分类本机改动代数的键名：每次 {@link #markDirty} 加一。 */
    static String generationKey(String section) {
        return "gen_" + requireSection(section);
    }

    /** 一个分类上次清掉待上传标记时的代数；与 {@link #generationKey} 不同即「本机有改动尚未上传」。 */
    static String cleanKey(String section) {
        return "clean_" + requireSection(section);
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
        if (enabled && (!validLoginKind(loginKind(context)) || accountId(context).isEmpty())) {
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

    /** Monotonic identity generation used to fence in-flight work across sign-out and re-login. */
    public static long bindingGeneration(Context context) {
        synchronized (DIRTY_LOCK) {
            return store(context).getLong(KEY_BINDING_GENERATION, 0L);
        }
    }

    /**
     * 记下这次登录的账号与方式。账号与已记录的不同（包括原来没有记录）时先 {@link #clear}，所以换账号后开关总是关闭、游标从头开始。
     *
     * @param accountId 服务端的用户 id；拿不到时传空字符串，此时一律按换账号处理
     */
    public static void bindAccount(Context context, String accountId, String loginKind) {
        if (!validLoginKind(loginKind)) throw new IllegalArgumentException("unknown login kind");
        String id = accountId == null ? "" : accountId;
        if (id.isEmpty()) {
            // A successful token exchange without a user id is not a usable sync binding.
            // Keep sync disabled until ProfilePage can bind the real account id.
            clear(context);
            return;
        }
        if (!id.equals(accountId(context))) clear(context);
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            long generation = values.getLong(KEY_BINDING_GENERATION, 0L);
            values.edit().putString(KEY_ACCOUNT_ID, id).putString(KEY_LOGIN_KIND, loginKind)
                .putLong(KEY_BINDING_GENERATION, generation + 1L).commit();
        }
    }

    public static String cursor(Context context, String section) {
        return store(context).getString(cursorKey(section), "");
    }

    public static void setCursor(Context context, String section, String cursor) {
        store(context).edit().putString(cursorKey(section), cursor == null ? "" : cursor).apply();
    }

    /** Writes a cursor only while the account binding that started the work is still current. */
    public static boolean setCursorIfCurrent(Context context, String section, String cursor,
            long expectedBindingGeneration) {
        String key = cursorKey(section);
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            if (values.getLong(KEY_BINDING_GENERATION, 0L) != expectedBindingGeneration) return false;
            values.edit().putString(key, cursor == null ? "" : cursor).apply();
            return true;
        }
    }

    public static boolean dirty(Context context, String section) {
        return dirty(store(context), section);
    }

    /** 标记一个分类本机有改动（代数加一）；同步关闭时不记，打开同步时本来就会整份比对。 */
    public static void markDirty(Context context, String section) {
        markDirty(store(context), section);
    }

    /** Marks a section dirty only for the binding that owns the current sync run. */
    public static boolean markDirtyIfCurrent(Context context, String section,
            long expectedBindingGeneration) {
        String key = generationKey(section);
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            if (values.getLong(KEY_BINDING_GENERATION, 0L) != expectedBindingGeneration
                    || !values.getBoolean(KEY_ENABLED, false)) return false;
            values.edit().putLong(key, values.getLong(key, 0L) + 1L).apply();
            return true;
        }
    }

    /**
     * 一个分类当前的改动代数。同步在读本机快照之前记下它，上传完用 {@link #clearDirtyIf} 只在期间没有新改动时清标记。
     */
    public static long generation(Context context, String section) {
        return generation(store(context), section);
    }

    /**
     * 代数仍是 `expected` 时清掉待上传标记并返回 true；期间又有改动（代数更大）时保留标记并返回 false，下一轮会把那次改动传上去。
     *
     * @param expected 读快照前的代数，加上这一轮自己写本机引起的标记次数
     */
    public static boolean clearDirtyIf(Context context, String section, long expected) {
        return clearDirtyIf(store(context), section, expected);
    }

    /** Clears a dirty mark only while both the local generation and account binding are unchanged. */
    public static boolean clearDirtyIfCurrent(Context context, String section, long expected,
            long expectedBindingGeneration) {
        String key = generationKey(section);
        String clean = cleanKey(section);
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            if (values.getLong(KEY_BINDING_GENERATION, 0L) != expectedBindingGeneration
                    || values.getLong(key, 0L) != expected) return false;
            values.edit().putLong(clean, expected).apply();
            return true;
        }
    }

    /** 无条件清掉待上传标记；只给确实要丢弃本机改动的路径用（例如整份用云端替换）。 */
    public static void clearDirty(Context context, String section) {
        clearDirty(store(context), section);
    }

    static boolean dirty(SharedPreferences store, String section) {
        synchronized (DIRTY_LOCK) {
            return store.getLong(generationKey(section), 0L) != store.getLong(cleanKey(section), 0L);
        }
    }

    static void markDirty(SharedPreferences store, String section) {
        String key = generationKey(section);
        synchronized (DIRTY_LOCK) {
            if (!store.getBoolean(KEY_ENABLED, false)) return;
            store.edit().putLong(key, store.getLong(key, 0L) + 1L).apply();
        }
    }

    static long generation(SharedPreferences store, String section) {
        String key = generationKey(section);
        synchronized (DIRTY_LOCK) {
            return store.getLong(key, 0L);
        }
    }

    static boolean clearDirtyIf(SharedPreferences store, String section, long expected) {
        String key = generationKey(section);
        String clean = cleanKey(section);
        synchronized (DIRTY_LOCK) {
            if (store.getLong(key, 0L) != expected) return false;
            store.edit().putLong(clean, expected).apply();
            return true;
        }
    }

    static void clearDirty(SharedPreferences store, String section) {
        String key = generationKey(section);
        String clean = cleanKey(section);
        synchronized (DIRTY_LOCK) {
            store.edit().putLong(clean, store.getLong(key, 0L)).apply();
        }
    }

    /**
     * 上次应用云端常用语时本机收不下的那些正文（超出本机条数或长度上限、或写入失败）。上传时把云端里这些正文原样带上，免得只因为本机放不下就从云端删掉别的设备的常用语。
     */
    public static Set<String> unheldPhrases(Context context) {
        Set<String> stored = store(context).getStringSet(KEY_PHRASES_UNHELD, null);
        return SetPolicy.copyOrEmpty(stored);
    }

    /** 每次成功应用云端常用语后整份替换。 */
    public static void setUnheldPhrases(Context context, Set<String> texts) {
        SharedPreferences.Editor editor = store(context).edit();
        if (texts == null || texts.isEmpty()) editor.remove(KEY_PHRASES_UNHELD);
        else editor.putStringSet(KEY_PHRASES_UNHELD, new HashSet<>(texts));
        editor.apply();
    }

    /** Updates held phrase metadata only for the binding that owns the current sync run. */
    public static boolean setUnheldPhrasesIfCurrent(Context context, Set<String> texts,
            long expectedBindingGeneration) {
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            if (values.getLong(KEY_BINDING_GENERATION, 0L) != expectedBindingGeneration) return false;
            SharedPreferences.Editor editor = values.edit();
            if (texts == null || texts.isEmpty()) editor.remove(KEY_PHRASES_UNHELD);
            else editor.putStringSet(KEY_PHRASES_UNHELD, new HashSet<>(texts));
            editor.apply();
            return true;
        }
    }

    /** 上次成功同步的 Unix 毫秒时间，从未同步过为 0。 */
    public static long lastSyncedAt(Context context) {
        return store(context).getLong(KEY_LAST_SYNCED_AT, 0L);
    }

    public static void setLastSyncedAt(Context context, long unixMillis) {
        store(context).edit().putLong(KEY_LAST_SYNCED_AT, BoundsPolicy.nonNegative(unixMillis)).apply();
    }

    /** Writes the last-sync timestamp only for the binding that performed the sync. */
    public static boolean setLastSyncedAtIfCurrent(Context context, long unixMillis,
            long expectedBindingGeneration) {
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            if (values.getLong(KEY_BINDING_GENERATION, 0L) != expectedBindingGeneration) return false;
            values.edit().putLong(KEY_LAST_SYNCED_AT, BoundsPolicy.nonNegative(unixMillis)).apply();
            return true;
        }
    }

    /** 退出登录与换账号时调用：关闭开关，清空账号、游标、标记与同步时间。同步写盘，返回时已经生效。 */
    public static void clear(Context context) {
        synchronized (DIRTY_LOCK) {
            SharedPreferences values = store(context);
            long generation = values.getLong(KEY_BINDING_GENERATION, 0L);
            values.edit().clear().putLong(KEY_BINDING_GENERATION, generation + 1L).commit();
        }
    }
}

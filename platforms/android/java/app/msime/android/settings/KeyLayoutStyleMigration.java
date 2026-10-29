package app.msime.android;

import android.content.Context;
import java.io.File;
import java.io.IOException;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 把 Android 本机的五个旧设置迁进共享偏好（.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md、#10223、#10229）：`platform.android.long_space_layout` 为真、共享的 `touch_twenty_six_key_style` 缺键或明写 `standard` 时写 `long_space`；`platform.android.number_row` 为真、共享的 `touch_number_row` 缺键或明写 `false` 时写开；两张九键左侧符号表（`platform.android.nine_key_symbols`、`platform.android.nine_key_digit_symbols`）解析得出、又不等于出厂表时放进请求，共享的 `touch_nine_key_sidebar_symbols` / `touch_nine_key_digit_sidebar_symbols` 缺键或是空数组时才写入；`platform.android.nine_key_swipe` 是 `up` 或 `down`、共享的 `touch_nine_key_swipe_digits` 缺键或明写 `off` 时原样写这个方向。这些判断在 client-core 的偏好锁里按原始文档做，共享偏好里其他任何值（别的设备选的样式、符号表和方向、较新版本加的写法、类型不对的值）都原样留着。写好之后删掉这五个本机键（不管旧值是不是出厂表、能不能解析），之后谁也不再读它们：共享偏好里空表既是缺键也是「恢复默认」，旧键留着的话下一次迁移会把用户刚恢复掉的旧表写回去再同步出去。「九键滑动距离」`platform.android.nine_key_swipe_distance` 仍是本机设置，不迁不删。
 *
 * <p>必须赶在设置第一次导出或应用之前：云同步每一轮（{@link RustCloudSync#run}）、本地备份导出、键盘建会话前读实时偏好、设置页读取时都先调 {@link #migrate}。备份恢复先迁当前旧键，再用 {@link #restoredSharedEdits} 翻译备份中明确保存的旧值。本机没有旧键时只读一次带缓存的本地设置文件，不碰共享偏好。两个进程同时迁移时靠共享偏好按修订号保存（被拒就重读重算）和本地设置的文件锁收敛，结果相同。迁移写进共享偏好的改动记成本机改动（{@link SyncSignals#markDirty}），与用户在设置页改的一样会上传，不会被下一轮下载的默认值盖回去。
 */
public final class KeyLayoutStyleMigration {
    private static final Object LOCK = new Object();
    /** 共享偏好被别处抢先改过（修订号不对）时重读重算的次数。 */
    private static final int SAVE_ATTEMPTS = 3;
    /** 本地备份沿账号设置文档恢复，字段名与 settings_sync.rs 保持一致。 */
    private static final String RESTORED_STYLE_FIELD = "platform.android.twenty_six_key_style";
    private static final String RESTORED_NUMBER_FIELD = "platform.android.number_row";
    /** 两张九键左侧符号表在账号设置文档里的字段名，与本机旧键同名（settings_sync.rs 的 `NINE_KEY_SIDEBAR_SYMBOLS`、`NINE_KEY_DIGIT_SIDEBAR_SYMBOLS`）。 */
    private static final String RESTORED_NINE_KEY_FIELD = "platform.android.nine_key_symbols";
    private static final String RESTORED_NINE_KEY_DIGIT_FIELD = "platform.android.nine_key_digit_symbols";
    private static final String RESTORED_NINE_KEY_SWIPE_FIELD = "platform.android.nine_key_swipe";

    private KeyLayoutStyleMigration() {}

    /** 备份里的显式旧值有恢复优先权；同一备份带着新字段时以新字段为准，缺失或无效旧键不补默认值。两张符号表的旧文本转成账号文档的写法（{@link NineKeySidebarPolicy#format}），等于出厂表或解析不出时写空串，即用出厂表。 */
    public static Map<String, Object> restoredSharedEdits(Map<String, ?> legacy, Map<String, ?> shared) {
        Map<String, Object> edits = new LinkedHashMap<>(5);
        if (!shared.containsKey(RESTORED_STYLE_FIELD)
                && legacy.get(AndroidLocalSettings.LONG_SPACE_LAYOUT) instanceof Boolean enabled)
            edits.put(RESTORED_STYLE_FIELD,
                enabled ? TouchKeyStyle.Row.LONG_SPACE.wire : TouchKeyStyle.Row.STANDARD.wire);
        if (!shared.containsKey(RESTORED_NUMBER_FIELD)
                && legacy.get(AndroidLocalSettings.NUMBER_ROW) instanceof Boolean enabled)
            edits.put(RESTORED_NUMBER_FIELD, enabled);
        if (!shared.containsKey(RESTORED_NINE_KEY_FIELD)
                && legacy.get(AndroidLocalSettings.NINE_KEY_SYMBOLS) instanceof String text)
            edits.put(RESTORED_NINE_KEY_FIELD,
                NineKeySidebarPolicy.format(legacyTable(text, NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS)));
        if (!shared.containsKey(RESTORED_NINE_KEY_DIGIT_FIELD)
                && legacy.get(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS) instanceof String text)
            edits.put(RESTORED_NINE_KEY_DIGIT_FIELD,
                NineKeySidebarPolicy.format(legacyTable(text, NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS)));
        // 关闭也写成共享编辑，和数字行把 false 也写成关闭一样：备份里明确选过关闭，恢复后就是关闭。
        if (!shared.containsKey(RESTORED_NINE_KEY_SWIPE_FIELD)
                && legacy.get(AndroidLocalSettings.NINE_KEY_SWIPE) instanceof String direction
                && direction.equals(NineKeySwipePolicy.mode(direction)))
            edits.put(RESTORED_NINE_KEY_SWIPE_FIELD, direction);
        return edits;
    }

    /** 一张旧符号表要迁进共享偏好的内容：旧文本解析得出、又不等于本端出厂表时是那张表，否则是空表（不迁，或恢复成出厂表）。 */
    public static List<String> legacyTable(String text, List<String> defaults) {
        List<String> parsed = NineKeySidebarPolicy.parse(text);
        return parsed == null || parsed.equals(defaults) ? List.of() : parsed;
    }

    /** 本机设置里一张旧符号表要迁的内容；没有这个旧键时是空表。 */
    public static List<String> legacyTable(AndroidLocalSettings.Snapshot local, String key, List<String> defaults) {
        return local.has(key) ? legacyTable(local.text(key), defaults) : List.of();
    }

    /** 恢复时转换进共享偏好的旧键，只删除这五项，不丢掉其他本机设置。 */
    public static boolean legacyKey(String key) {
        return AndroidLocalSettings.LONG_SPACE_LAYOUT.equals(key) || AndroidLocalSettings.NUMBER_ROW.equals(key)
            || AndroidLocalSettings.NINE_KEY_SYMBOLS.equals(key) || AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS.equals(key)
            || AndroidLocalSettings.NINE_KEY_SWIPE.equals(key);
    }

    /** 本机设置文件里还留着五个旧键中的任何一个。 */
    public static boolean pending(AndroidLocalSettings.Snapshot local) {
        return local.has(AndroidLocalSettings.LONG_SPACE_LAYOUT) || local.has(AndroidLocalSettings.NUMBER_ROW)
            || local.has(AndroidLocalSettings.NINE_KEY_SYMBOLS) || local.has(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS)
            || local.has(AndroidLocalSettings.NINE_KEY_SWIPE);
    }

    /**
     * 做一次迁移，返回本机是否已经没有旧键。在工作线程上调用：要读写共享偏好和本地设置两个文件。共享偏好还不存在（首次准备之前）或读写失败时什么也不删，下次再试。
     *
     * @param preferencesDirectory 共享偏好所在的目录（`runtime-options.json` 的 `preferences_directory`）
     */
    public static boolean migrate(Context context, String preferencesDirectory) {
        if (!pending(AndroidLocalSettings.load(context))) return true;
        if (preferencesDirectory == null || preferencesDirectory.isEmpty()
                || !new File(preferencesDirectory).isAbsolute()) return false;
        synchronized (LOCK) {
            try {
                for (int attempt = 0; attempt < SAVE_ATTEMPTS; attempt++) {
                    AndroidLocalSettings.Snapshot local = AndroidLocalSettings.load(context);
                    if (!pending(local)) return true;
                    boolean longSpace = local.bool(AndroidLocalSettings.LONG_SPACE_LAYOUT);
                    boolean numberRow = local.bool(AndroidLocalSettings.NUMBER_ROW);
                    List<String> nineKeySymbols = legacyTable(local, AndroidLocalSettings.NINE_KEY_SYMBOLS,
                        NineKeySidebarPolicy.DEFAULT_LETTER_SYMBOLS);
                    List<String> nineKeyDigitSymbols = legacyTable(local, AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS,
                        NineKeySidebarPolicy.DEFAULT_DIGIT_SYMBOLS);
                    JSONObject snapshot = JsonPolicy.objectValue(new JSONObject(
                        NativeClient.loadPreferences(preferencesDirectory)));
                    if (snapshot == null) return false;
                    // 在共享修订号之后复查旧键来源：恢复会先删旧键再写共享值，另一进程不能拿恢复前的旧值为这个新修订号背书。
                    if (!local.equals(AndroidLocalSettings.load(context))) continue;
                    long revision = PreferencesRevisionPolicy.read(snapshot.opt("revision"), -1);
                    if (revision < 0) return false;
                    JSONObject request = new JSONObject().put("directory", preferencesDirectory)
                        .put("revision", revision).put("long_space", longSpace).put("number_row", numberRow)
                        .put("nine_key_symbols", new JSONArray(nineKeySymbols))
                        .put("nine_key_digit_symbols", new JSONArray(nineKeyDigitSymbols));
                    // 本机没有这个旧键时不带这个字段，等于没有旧值；有时原样交给 client-core，由它在偏好锁里决定写不写。
                    if (local.has(AndroidLocalSettings.NINE_KEY_SWIPE))
                        request.put("nine_key_swipe", local.choice(AndroidLocalSettings.NINE_KEY_SWIPE));
                    JSONObject saved = JsonPolicy.objectValue(new JSONObject(NativeClient.migrateKeyLayout(request.toString())));
                    // 修订号不对：别处刚改过共享偏好，重读后按新值再算一遍。
                    if (saved == null) continue;
                    if (PreferencesRevisionPolicy.read(saved.opt("revision"), -1) != revision)
                        SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                    Map<String, Object> removal = new LinkedHashMap<>(5);
                    removal.put(AndroidLocalSettings.LONG_SPACE_LAYOUT, null);
                    removal.put(AndroidLocalSettings.NUMBER_ROW, null);
                    removal.put(AndroidLocalSettings.NINE_KEY_SYMBOLS, null);
                    removal.put(AndroidLocalSettings.NINE_KEY_DIGIT_SYMBOLS, null);
                    removal.put(AndroidLocalSettings.NINE_KEY_SWIPE, null);
                    AndroidLocalSettings.update(context, removal);
                    return true;
                }
                return false;
            } catch (IOException | JSONException | RuntimeException | LinkageError unavailable) {
                // 这是读写两个设置文件和原生库的边界：任何一步失败都保留旧键，下次调用时再迁。
                return false;
            }
        }
    }
}

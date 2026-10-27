package app.msime.android;

import android.content.Context;
import java.io.File;
import java.io.IOException;
import java.util.LinkedHashMap;
import java.util.Map;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 把 Android 本机的两个旧设置迁进共享偏好（.agents/notes/implemented/feature/2026-10-27-touch-key-layout-styles.md）：`platform.android.long_space_layout` 为真、共享的 `touch_twenty_six_key_style` 还是标准底行时写 `long_space`；`platform.android.number_row` 为真、共享的 `touch_number_row` 还是关时写开。写好之后删掉这两个本机键，之后谁也不再读它们。
 *
 * <p>必须赶在设置第一次导出或应用之前：云同步每一轮（{@link RustCloudSync#run}）、本地备份导出、键盘建会话前读实时偏好、设置页读取时都先调 {@link #migrate}。备份恢复先迁当前旧键，再用 {@link #restoredSharedEdits} 翻译备份中明确保存的旧值。本机没有旧键时只读一次带缓存的本地设置文件，不碰共享偏好。两个进程同时迁移时靠共享偏好按修订号保存（被拒就重读重算）和本地设置的文件锁收敛，结果相同。迁移写进共享偏好的改动记成本机改动（{@link SyncSignals#markDirty}），与用户在设置页改的一样会上传，不会被下一轮下载的默认值盖回去。
 */
public final class KeyLayoutStyleMigration {
    private static final Object LOCK = new Object();
    /** 共享偏好被别处抢先改过（修订号不对）时重读重算的次数。 */
    private static final int SAVE_ATTEMPTS = 3;
    private static final int FORMAT_VERSION = 1;
    /** 本地备份沿账号设置文档恢复，字段名与 settings_sync.rs 保持一致。 */
    private static final String RESTORED_STYLE_FIELD = "platform.android.twenty_six_key_style";
    private static final String RESTORED_NUMBER_FIELD = "platform.android.number_row";

    private KeyLayoutStyleMigration() {}

    /**
     * 要写进共享偏好的改动（偏好键 → 值），按迁移规则算出，可能为空。共享偏好里已经选了别的样式、或数字行已经开着时不改。
     *
     * @param legacyLongSpace 本机旧设置 `platform.android.long_space_layout`
     * @param legacyNumberRow 本机旧设置 `platform.android.number_row`
     * @param twentySixKeyStyle 共享偏好里 `touch_twenty_six_key_style` 的写法，缺键时为 null
     * @param numberRow 共享偏好里的 `touch_number_row`
     */
    public static Map<String, Object> sharedEdits(boolean legacyLongSpace, boolean legacyNumberRow,
            String twentySixKeyStyle, boolean numberRow) {
        Map<String, Object> edits = new LinkedHashMap<>(2);
        if (legacyLongSpace && TouchKeyStyle.parse(twentySixKeyStyle, TouchKeyStyle.TWENTY_SIX_KEY_ROWS)
                == TouchKeyStyle.Row.STANDARD)
            edits.put(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE, TouchKeyStyle.Row.LONG_SPACE.wire);
        if (legacyNumberRow && !numberRow) edits.put(TouchKeyStyle.NUMBER_ROW_PREFERENCE, true);
        return edits;
    }

    /** 备份里的显式旧值有恢复优先权；同一备份带着新字段时以新字段为准，缺失或无效旧键不补默认值。 */
    public static Map<String, Object> restoredSharedEdits(Map<String, ?> legacy, Map<String, ?> shared) {
        Map<String, Object> edits = new LinkedHashMap<>(2);
        if (!shared.containsKey(RESTORED_STYLE_FIELD)
                && legacy.get(AndroidLocalSettings.LONG_SPACE_LAYOUT) instanceof Boolean enabled)
            edits.put(RESTORED_STYLE_FIELD,
                enabled ? TouchKeyStyle.Row.LONG_SPACE.wire : TouchKeyStyle.Row.STANDARD.wire);
        if (!shared.containsKey(RESTORED_NUMBER_FIELD)
                && legacy.get(AndroidLocalSettings.NUMBER_ROW) instanceof Boolean enabled)
            edits.put(RESTORED_NUMBER_FIELD, enabled);
        return edits;
    }

    /** 恢复时转换进共享偏好的旧键，只删除这两项，不丢掉其他本机设置。 */
    public static boolean legacyKey(String key) {
        return AndroidLocalSettings.LONG_SPACE_LAYOUT.equals(key) || AndroidLocalSettings.NUMBER_ROW.equals(key);
    }

    /** 本机设置文件里还留着两个旧键中的任何一个。 */
    public static boolean pending(AndroidLocalSettings.Snapshot local) {
        return local.has(AndroidLocalSettings.LONG_SPACE_LAYOUT) || local.has(AndroidLocalSettings.NUMBER_ROW);
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
                    JSONObject snapshot = JsonPolicy.objectValue(new JSONObject(
                        NativeClient.loadPreferences(preferencesDirectory)));
                    if (snapshot == null) return false;
                    // 在共享修订号之后复查旧键来源：恢复会先删旧键再写共享值，另一进程不能拿恢复前的旧值为这个新修订号背书。
                    if (!local.equals(AndroidLocalSettings.load(context))) continue;
                    JSONObject preferences = snapshot.getJSONObject("preferences");
                    Map<String, Object> edits = sharedEdits(longSpace, numberRow,
                        JsonPolicy.strictString(preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE)),
                        preferences.optBoolean(TouchKeyStyle.NUMBER_ROW_PREFERENCE, false));
                    if (!edits.isEmpty()) {
                        long revision = PreferencesRevisionPolicy.read(snapshot.opt("revision"), -1);
                        if (revision < 0) return false;
                        for (Map.Entry<String, Object> edit : edits.entrySet())
                            preferences.put(edit.getKey(), edit.getValue());
                        snapshot.put("format_version", FORMAT_VERSION);
                        JSONObject saved = JsonPolicy.objectValue(new JSONObject(
                            NativeClient.savePreferences(preferencesDirectory, revision, snapshot.toString())));
                        // 修订号不对：别处刚改过共享偏好，重读后按新值再算一遍。
                        if (saved == null) continue;
                        SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                    }
                    Map<String, Object> removal = new LinkedHashMap<>(2);
                    removal.put(AndroidLocalSettings.LONG_SPACE_LAYOUT, null);
                    removal.put(AndroidLocalSettings.NUMBER_ROW, null);
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

package app.msime.android;

import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;

/**
 * 本地备份包（#5659）的格式规则：文件名、包里的条目和各自的大小上限、能不能恢复、设置文档的字段类型，以及恢复完给用户看的那句话。读写文件和调用原生库的一半在 {@code home/LocalBackup}，这里不依赖 Android。
 *
 * <p>备份包是一个 zip：`manifest.json` 说明格式与来源版本；`settings.json` 是与云同步相同的设置文档（{@code msime_client_account_settings_export}，不含凭据与诊断日志）；`android-local.json` 是 Android 本地设置文件里显式写过的值；`skins.json` 是自定义键盘皮肤的设计参数；`phrases.json` 是自己添加的常用语；`dictionary.ndjson` 是个人词库，格式与云端词库快照相同。各条目都可以缺，恢复时有哪项恢复哪项。
 */
public final class LocalBackupPolicy {
    public static final String FORMAT = "msime-android-backup";
    public static final int VERSION = 1;

    public static final String MANIFEST = "manifest.json";
    public static final String SETTINGS = "settings.json";
    public static final String ANDROID_LOCAL = "android-local.json";
    public static final String SKINS = "skins.json";
    public static final String PHRASES = "phrases.json";
    public static final String DICTIONARY = "dictionary.ndjson";

    /** 选中的备份文件最多这么大；个人词库快照本身最多 512 MiB，压缩后远小于它。 */
    public static final long MAX_ARCHIVE_BYTES = 600L * 1024 * 1024;
    public static final int MAX_MANIFEST_BYTES = 64 * 1024;
    /** 与 {@code msime_client_account_settings_apply} 的请求上限相同。 */
    public static final int MAX_SETTINGS_BYTES = 4 * 1024 * 1024;
    public static final int MAX_ANDROID_LOCAL_BYTES = 64 * 1024;
    public static final int MAX_SKINS_BYTES = 16 * 1024 * 1024;
    /** 自己添加的常用语最多 200 条、每条 1000 个 UTF-16 单元。 */
    public static final int MAX_PHRASES_BYTES = 2 * 1024 * 1024;
    /** 与个人词库快照的上限相同（{@code DictionarySnapshotQueue.MAXIMUM_SNAPSHOT_BYTES}）。 */
    public static final long MAX_DICTIONARY_BYTES = 512L * 1024 * 1024;

    /** 设置文档的字段最多这么多个，与 client-core 的上限相同。 */
    static final int MAX_SETTINGS_FIELDS = 512;

    private static final DateTimeFormatter STAMP = DateTimeFormatter.ofPattern("yyyyMMdd-HHmm", Locale.ROOT);
    /** 开发者选项在本地设置里的键名前缀，见 {@link #backsUpLocalSetting}。 */
    private static final String DEVELOPER_PREFIX = "platform.android.developer.";

    /** 一份备份包能不能在这个版本上恢复。 */
    public enum Compatibility {
        OK,
        /** 不是本应用导出的备份包。 */
        NOT_A_BACKUP,
        /** 更新的应用导出的、这个版本还不认识的格式。 */
        NEWER_FORMAT,
    }

    private LocalBackupPolicy() {}

    /** 「水杉输入法-0.2.2-20261008-0915.zip」：应用名加版本号，方便降级时找到对应的包；再加导出时间，同一天导出多份不会互相覆盖。文件名里不能用的字符换成「_」。 */
    public static String fileName(String appName, String versionName, LocalDateTime now) {
        String app = safeNamePart(appName, "msime");
        String version = safeNamePart(versionName, "0");
        return app + "-" + version + "-" + STAMP.format(now) + ".zip";
    }

    static String safeNamePart(String value, String fallback) {
        if (value == null || value.isBlank()) return fallback;
        StringBuilder name = new StringBuilder(value.length());
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            boolean unsafe = character < 0x20 || "/\\:*?\"<>|".indexOf(character) >= 0 || Character.isWhitespace(character);
            name.append(unsafe ? '_' : character);
        }
        return name.toString();
    }

    /**
     * Android 本地设置里哪些键进备份、从备份恢复。语音数据贡献（{@code AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO}）是用户在本机看过说明、点了确认才开的上传授权，开发者选项（`platform.android.developer.*`：输入日志、调试信息、日志级别和 MCP 可访问的日志）是排查一次问题时临时打开的开关；从备份恢复它们会绕过确认，在另一台设备上悄悄开始上传语音或记录输入日志。所以这两类既不导出，也不从（旧版或别人改过的）备份里恢复；其余的本地设置照常带走。
     */
    public static boolean backsUpLocalSetting(String key) {
        return key != null && !key.equals(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO)
            && !key.startsWith(DEVELOPER_PREFIX);
    }

    /** 按 manifest 里的 `format` 和 `version` 判断；`version` 读不出整数时传 null。 */
    public static Compatibility compatibility(Object format, Object version) {
        if (!FORMAT.equals(format) || !(version instanceof Integer number) || number < 1) {
            return Compatibility.NOT_A_BACKUP;
        }
        return number > VERSION ? Compatibility.NEWER_FORMAT : Compatibility.OK;
    }

    /**
     * 恢复设置时交给 {@code msime_client_account_settings_apply} 的字段类型：备份里的设置文档是 client-core 自己导出的，每个键按它自己的值声明类型，这样应用的逻辑和云同步完全相同，只是没有服务端字段表。值不是布尔、整数、小数或字符串的键不声明（client-core 会跳过它），超过字段上限时返回 null。
     */
    public static Map<String, String> fieldTypes(Map<String, ?> settings) {
        if (settings.size() > MAX_SETTINGS_FIELDS) return null;
        Map<String, String> types = new LinkedHashMap<>(settings.size());
        for (Map.Entry<String, ?> entry : settings.entrySet()) {
            String type = valueType(entry.getValue());
            if (type != null) types.put(entry.getKey(), type);
        }
        return types;
    }

    static String valueType(Object value) {
        if (value instanceof Boolean) return "boolean";
        if (value instanceof Integer || value instanceof Long) return "integer";
        if (value instanceof Double number) {
            if (number.isNaN() || number.isInfinite()) return null;
            return "number";
        }
        if (value instanceof String) return "string";
        return null;
    }

    /** 恢复的结果。`words` 是交给键盘写入的词数（整份快照激活，或排进待发送队列后分批写入）；`failed` 是没能恢复的部分的名字，按发生顺序。 */
    public record Restored(boolean settings, int skins, int phrases, int words, int skippedWords,
            java.util.List<String> failed) {}

    /** 恢复完给用户看的那句话。 */
    public static String summary(Restored restored) {
        java.util.List<String> parts = new java.util.ArrayList<>(4);
        if (restored.settings()) parts.add("设置");
        if (restored.skins() > 0) parts.add(restored.skins() + " 个自定义皮肤");
        if (restored.phrases() > 0) parts.add(restored.phrases() + " 条常用语");
        StringBuilder text = new StringBuilder();
        if (parts.isEmpty() && restored.words() == 0) {
            text.append("备份里没有需要恢复的新内容");
        } else {
            if (!parts.isEmpty()) text.append("已恢复").append(String.join("、", parts));
            if (restored.words() > 0) {
                if (text.length() > 0) text.append("；");
                text.append(restored.words()).append(" 个词会在键盘空闲时陆续写入词库");
            }
        }
        if (restored.skippedWords() > 0) text.append("；").append(restored.skippedWords()).append(" 个词无法导入，已跳过");
        if (!restored.failed().isEmpty()) {
            text.append("；").append(String.join("、", restored.failed())).append("没有恢复，请重试");
        }
        return text.append("。").toString();
    }
}

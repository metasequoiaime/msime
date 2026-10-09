package app.msime.android;

import java.time.LocalDateTime;
import java.time.format.DateTimeFormatter;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * 本地备份包（#5659）的格式规则：文件名、包里的条目和各自的大小上限、能不能恢复、设置文档的字段类型，以及恢复完给用户看的那句话。读写文件和调用原生库的一半在 {@code home/LocalBackup}，这里不依赖 Android。
 *
 * <p>备份包是一个 zip：`manifest.json` 说明格式与来源版本；`settings.json` 是与云同步相同的设置文档（{@code msime_client_account_settings_export}，不含凭据与诊断日志）；`android-local.json` 是 Android 本地设置文件里显式写过的值；`skins.json` 是自定义键盘皮肤的设计参数；`phrases.json` 是自己添加的常用语；`dictionary.ndjson` 是个人词库，格式与云端词库快照相同，从 #5659 起还带输入记录（学习调权、删除记录、固定位置和选词计数，都是这个快照格式第 1 版本来就有的记录，所以 {@link #VERSION} 不变，旧版本恢复时照样认得）；`habits.ndjson` 是快照格式装不下的输入习惯（整句联想、连续选词、拼写纠错、自动纠错抑制和置顶的候选，格式见 host-api 的 `dictionary_snapshot::habits`），旧版本按固定条目名读包，不认识它就不读。各条目都可以缺，恢复时有哪项恢复哪项。
 *
 * <p>manifest 的 `checksums` 记着其余每个条目的 SHA-256（{@link #checksumsMatch}）；恢复前先核对它，再逐项解析和校验，全部通过才动本机（旧版本导出的包没有 `checksums`，只做解析和校验）。新增 `checksums` 和 `habits.ndjson` 都不改 {@link #VERSION}：旧版本只读它认识的键和条目。
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
    public static final String HABITS = "habits.ndjson";
    /** manifest 里记各条目 SHA-256 的键。 */
    public static final String CHECKSUMS = "checksums";
    /** manifest 之外的全部条目，按写进包的顺序；校验和只对这些条目核对。 */
    public static final List<String> ENTRIES = List.of(SETTINGS, ANDROID_LOCAL, SKINS, PHRASES, DICTIONARY, HABITS);

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
    /** 与原生侧输入习惯文件的上限相同；整句联想最多 20 万行，远到不了。 */
    public static final long MAX_HABITS_BYTES = 256L * 1024 * 1024;

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
        /** 是备份包，但内容和 manifest 记的校验和对不上，或者有条目解析、校验不过：文件损坏或不完整。 */
        DAMAGED,
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

    /** 一个条目最多读多少字节，与恢复时读它的上限相同；不认识的条目名返回 -1。 */
    public static long entryLimit(String name) {
        return switch (name) {
            case MANIFEST -> MAX_MANIFEST_BYTES;
            case SETTINGS -> MAX_SETTINGS_BYTES;
            case ANDROID_LOCAL -> MAX_ANDROID_LOCAL_BYTES;
            case SKINS -> MAX_SKINS_BYTES;
            case PHRASES -> MAX_PHRASES_BYTES;
            case DICTIONARY -> MAX_DICTIONARY_BYTES;
            case HABITS -> MAX_HABITS_BYTES;
            default -> -1;
        };
    }

    /**
     * 包里的条目和 manifest 记的校验和对不对得上。`declared` 是 manifest 的 `checksums`（条目名到小写十六进制 SHA-256），旧版本导出的包没有它，传 null，这时一律算对上（只靠逐项解析和校验）。`actual` 是包里实际有的、{@link #ENTRIES} 里的条目和它们算出来的 SHA-256。
     *
     * <p>对得上的条件：记了的条目包里都有、值相同；包里有的条目都记了。前者拦住截断和被改过的条目，后者拦住被塞进去的条目。manifest 里记了、这个版本不认识的条目名（更新的版本加的）不核对，恢复时也不读它。
     */
    public static boolean checksumsMatch(Map<String, String> declared, Map<String, String> actual) {
        if (declared == null) return true;
        for (String name : ENTRIES) {
            String expected = declared.get(name);
            String found = actual.get(name);
            if (expected == null && found == null) continue;
            if (expected == null || found == null || !expected.equalsIgnoreCase(found)) return false;
        }
        return true;
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

    /** 恢复的结果。`words` 是交给键盘写入的词数（整份快照激活，或排进待发送队列后分批写入）；`learning` 是交给键盘写入的输入记录条数（随整份快照激活，或在键盘收起后的空闲时合并），`habits` 是输入习惯条数（键盘收起后的空闲时合并）；`failed` 是没能恢复的部分的名字，按发生顺序。 */
    public record Restored(boolean settings, int skins, int phrases, int words, int skippedWords, int learning,
            int habits, java.util.List<String> failed) {}

    /** 备份里的个人词库怎么恢复：{@link #ACTIVATE} 把整份快照交给激活队列，替换本机的全部词和输入记录；{@link #MERGE} 词进待发送队列、输入记录排给键盘合并，本机已有的都保留。 */
    public enum DictionaryRestore { ACTIVATE, MERGE }

    /**
     * 选恢复个人词库的方式。整份激活会替换本机的全部学习状态，所以只有备份里有词、本机确定既没有用户词也没有任何输入记录（新手机、重装）时才用它；本机哪怕只学过一点、或者数不出来（参数为 null），都走合并，不冒覆盖本机的险。备份里只有输入记录没有词时同样走合并。
     *
     * @param localWords 本机用户词数，读不出来时为 null
     * @param localLearning 本机输入记录条数（`learning_count`），读不出来时为 null
     */
    public static DictionaryRestore dictionaryRestore(int backupWords, Integer localWords, Integer localLearning) {
        return backupWords > 0 && localWords != null && localWords == 0 && localLearning != null && localLearning == 0
            ? DictionaryRestore.ACTIVATE : DictionaryRestore.MERGE;
    }

    /** 恢复完给用户看的那句话。 */
    public static String summary(Restored restored) {
        java.util.List<String> parts = new java.util.ArrayList<>(4);
        if (restored.settings()) parts.add("设置");
        if (restored.skins() > 0) parts.add(restored.skins() + " 个自定义皮肤");
        if (restored.phrases() > 0) parts.add(restored.phrases() + " 条常用语");
        StringBuilder text = new StringBuilder();
        int records = restored.learning() + restored.habits();
        if (parts.isEmpty() && restored.words() == 0 && records == 0) {
            text.append("备份里没有需要恢复的新内容");
        } else {
            if (!parts.isEmpty()) text.append("已恢复").append(String.join("、", parts));
            if (restored.words() > 0) {
                if (text.length() > 0) text.append("；");
                text.append(restored.words()).append(" 个词会在键盘空闲时陆续写入词库");
            }
            if (records > 0) {
                if (text.length() > 0) text.append("；");
                text.append(records).append(" 条输入记录会在键盘空闲时合并，本机已有的保留本机");
            }
        }
        if (restored.skippedWords() > 0) text.append("；").append(restored.skippedWords()).append(" 个词无法导入，已跳过");
        if (!restored.failed().isEmpty()) {
            text.append("；").append(String.join("、", restored.failed())).append("没有恢复，请重试");
        }
        return text.append("。").toString();
    }

    /**
     * 恢复设置、皮肤或常用语时有一部分写入失败、整次恢复撤销时给用户看的话。`part` 是失败的那一部分；`undone` 为假时撤销本身也没有完成，要如实说。词库和输入记录在这几部分全部成功之后才动，所以这时还没有碰过。
     */
    public static String rolledBack(String part, boolean undone) {
        return undone
            ? part + "没能恢复，这次恢复已经全部撤销，本机保持恢复前的样子。请稍后重试。"
            : part + "没能恢复，撤销时也出了错，设置、自定义皮肤或常用语可能只恢复了一部分；词库和输入记录没有改动。请重试一次恢复。";
    }
}
